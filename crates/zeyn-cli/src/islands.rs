//! `zeyn build-islands`: compiles island crates to WebAssembly and writes content hashed files
//! plus `manifest.json`, which `App::islands` reads to serve them.
//!
//! Each crate is built with `cargo build --target wasm32-unknown-unknown --release` and size
//! settings (`opt-level = "z"`, fat LTO, one codegen unit, `panic = "abort"`, no debug info), then
//! `wasm-bindgen --target web`, then `wasm-opt -Oz` when it is on the `PATH`.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Folder holding island crates by default, one crate per subfolder.
pub const CRATES_DIR: &str = "islands";
/// Output folder by default.
pub const OUT_DIR: &str = "dist/islands";

const SIZE_PROFILE: [&str; 5] = [
    "profile.release.opt-level=\"z\"",
    "profile.release.lto=true",
    "profile.release.codegen-units=1",
    "profile.release.panic=\"abort\"",
    "profile.release.debug=false",
];

/// Island crates under `root/islands`, sorted.
pub fn default_crates(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root.join(CRATES_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file())
        .collect();
    found.sort();
    found
}

/// Builds every crate into `out` and returns a report with the file sizes.
pub fn build(crates: &[PathBuf], out: &Path) -> Result<String, String> {
    if crates.is_empty() {
        return Err(format!("no island crates found; put each one in {CRATES_DIR}/<name>/ or name the folders"));
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let manifest_path = out.join("manifest.json");
    let mut islands: Map<String, Value> = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["islands"].as_object().cloned())
        .unwrap_or_default();
    let optimizer = Command::new("wasm-opt").arg("--version").output().is_ok_and(|o| o.status.success());
    let mut report = String::new();
    for dir in crates {
        let (name, wasm) = compile(dir)?;
        let staging = out.join(format!(".staging-{name}"));
        let _ = std::fs::remove_dir_all(&staging);
        run(Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--remove-name-section", "--remove-producers-section", "--out-name", &name, "--out-dir"])
            .arg(&staging)
            .arg(&wasm))?;
        let bg = staging.join(format!("{name}_bg.wasm"));
        if optimizer {
            run(Command::new("wasm-opt")
                .args(["-Oz", "--enable-bulk-memory", "--enable-nontrapping-float-to-int", "--enable-sign-ext", "--enable-reference-types", "--enable-mutable-globals", "--enable-multivalue", "-o"])
                .arg(&bg)
                .arg(&bg))?;
        }
        let js_bytes = std::fs::read(staging.join(format!("{name}.js"))).map_err(|e| e.to_string())?;
        let wasm_bytes = std::fs::read(&bg).map_err(|e| e.to_string())?;
        remove_old(out, &name);
        let js_file = format!("{name}.{}.js", hash(&js_bytes));
        let wasm_file = format!("{name}_bg.{}.wasm", hash(&wasm_bytes));
        std::fs::write(out.join(&js_file), &js_bytes).map_err(|e| e.to_string())?;
        std::fs::write(out.join(&wasm_file), &wasm_bytes).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&staging);
        report.push_str(&format!("{name}: {wasm_file} {} bytes, {js_file} {} bytes\n", wasm_bytes.len(), js_bytes.len()));
        islands.insert(name, json!({ "js": js_file, "wasm": wasm_file }));
    }
    let manifest = json!({ "islands": islands });
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
    if !optimizer {
        report.push_str("wasm-opt was not found on the PATH; install binaryen for smaller files\n");
    }
    report.push_str(&format!("wrote {}", manifest_path.display()));
    Ok(report)
}

/// Builds one crate for the browser; returns the island name (the library name) and its `.wasm`.
fn compile(dir: &Path) -> Result<(String, PathBuf), String> {
    let manifest = std::fs::canonicalize(dir.join("Cargo.toml")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--manifest-path"])
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cargo could not be started: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let package = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["manifest_path"].as_str().and_then(|m| std::fs::canonicalize(m).ok()).is_some_and(|m| m == manifest))
        .ok_or_else(|| format!("{}: package not found", manifest.display()))?;
    let lib = package["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["kind"].as_array().is_some_and(|k| k.iter().any(|k| k == "cdylib")))
        .and_then(|t| t["name"].as_str())
        .ok_or_else(|| format!("{}: an island needs `[lib] crate-type = [\"cdylib\"]`", manifest.display()))?
        .replace('-', "_");
    let target = meta["target_directory"].as_str().map(PathBuf::from).ok_or("cargo metadata has no target directory")?;
    let mut cargo = Command::new("cargo");
    cargo.args(["build", "--lib", "--release", "--target", "wasm32-unknown-unknown", "--manifest-path"]).arg(&manifest);
    for setting in SIZE_PROFILE {
        cargo.args(["--config", setting]);
    }
    run(&mut cargo)?;
    Ok((lib.clone(), target.join("wasm32-unknown-unknown").join("release").join(format!("{lib}.wasm"))))
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let status = cmd.status().map_err(|e| format!("{program} could not be started: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{program} failed ({status})")) }
}

/// Deletes the earlier hashed files of an island.
fn remove_old(out: &Path, name: &str) {
    let Ok(entries) = std::fs::read_dir(out) else { return };
    for e in entries.flatten() {
        let file = e.file_name().to_string_lossy().into_owned();
        let ours = (file.starts_with(&format!("{name}.")) && file.ends_with(".js")) || (file.starts_with(&format!("{name}_bg.")) && file.ends_with(".wasm"));
        if ours {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// 64 bit FNV-1a of the content as 12 hex digits: a cache key, not a security check.
pub fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_short() {
        assert_eq!(hash(b""), "cbf29ce48422");
        assert_ne!(hash(b"a"), hash(b"b"));
        assert_eq!(hash(b"island").len(), 12);
    }

    #[test]
    fn remove_old_keeps_other_islands() {
        let dir = std::env::temp_dir().join(format!("zeyn-islands-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["a.111.js", "a_bg.111.wasm", "ab.222.js", "manifest.json"] {
            std::fs::write(dir.join(f), "x").unwrap();
        }
        remove_old(&dir, "a");
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, ["ab.222.js", "manifest.json"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
