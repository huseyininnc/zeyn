//! `zeyn dev`: build, run, watch, rebuild, restart. The browser follows each new build without a
//! page load (see `assets/dev.js` in the zeyn crate).
//!
//! The running server keeps serving while a new build compiles; it is only replaced once the
//! build succeeds, so a compile error never takes the app down. On Windows a running program
//! cannot be overwritten, so the server always runs from a copy of the built binary.

use serde_json::Value;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

/// How often files are checked, and how long changes must settle before a rebuild.
const POLL: Duration = Duration::from_millis(300);
const SETTLE: Duration = Duration::from_millis(150);

/// Runs the dev loop until the process is stopped (Ctrl+C stops the server too).
pub fn run(root: &Path) -> Result<(), String> {
    let watch = watch_roots(root)?;
    println!("zeyn dev: watching {} folder(s); Ctrl+C stops", watch.len());
    let mut stamp = snapshot(&watch);
    let mut generation = 0u32;
    let mut server = match build(root)? {
        Some(exe) => Some(start(root, &exe, &mut generation)?),
        None => None,
    };
    loop {
        std::thread::sleep(POLL);
        let now = snapshot(&watch);
        if now == stamp {
            continue;
        }
        std::thread::sleep(SETTLE);
        stamp = snapshot(&watch);
        println!("zeyn dev: change detected, rebuilding");
        match build(root)? {
            Some(exe) => {
                if let Some(mut old) = server.take() {
                    let _ = old.kill();
                    let _ = old.wait();
                }
                server = Some(start(root, &exe, &mut generation)?);
            }
            None => println!("zeyn dev: build failed; the running server keeps the last good build"),
        }
    }
}

/// Folders of every local package the app is built from: the app, its workspace members and
/// path dependencies such as shared libraries or a local zeyn checkout.
fn watch_roots(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo could not be started: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut roots: Vec<PathBuf> = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["source"].is_null())
        .filter_map(|p| p["manifest_path"].as_str().map(|m| Path::new(m).parent().map(Path::to_path_buf)))
        .flatten()
        .collect();
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// Modification times of the files under the watched folders, skipping build output and VCS data.
fn snapshot(roots: &[PathBuf]) -> HashMap<PathBuf, SystemTime> {
    let mut out = HashMap::new();
    for root in roots {
        walk(root, &mut out);
    }
    out
}

fn walk(dir: &Path, out: &mut HashMap<PathBuf, SystemTime>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            walk(&path, out);
        } else if let Ok(modified) = meta.modified() {
            out.insert(path, modified);
        }
    }
}

/// Builds the app; returns its binary, or `None` when the build failed (errors are already shown).
fn build(root: &Path) -> Result<Option<PathBuf>, String> {
    let manifest = std::fs::canonicalize(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let mut child = Command::new("cargo")
        .args(["build", "--message-format=json-render-diagnostics"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cargo could not be started: {e}"))?;
    let mut exe = None;
    if let Some(stdout) = child.stdout.take() {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
            if msg["reason"] != "compiler-artifact" {
                continue;
            }
            let Some(path) = msg["executable"].as_str() else { continue };
            let ours = msg["manifest_path"].as_str().and_then(|m| std::fs::canonicalize(m).ok()).is_some_and(|m| m == manifest);
            if ours {
                exe = Some(PathBuf::from(path));
            }
        }
    }
    let ok = child.wait().map_err(|e| e.to_string())?.success();
    if ok && exe.is_none() {
        return Err("the package has no binary to run; `zeyn dev` runs apps, not libraries".into());
    }
    Ok(if ok { exe } else { None })
}

/// Starts the server from a fresh copy of the binary with `ZEYN_DEV=1`.
fn start(root: &Path, exe: &Path, generation: &mut u32) -> Result<Child, String> {
    let dir = exe.parent().unwrap_or(root).join("zeyn-dev");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if let Ok(old) = std::fs::read_dir(&dir) {
        for f in old.flatten() {
            let _ = std::fs::remove_file(f.path());
        }
    }
    *generation += 1;
    let name = format!("run-{generation}{}", std::env::consts::EXE_SUFFIX);
    let copy = dir.join(name);
    std::fs::copy(exe, &copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    Command::new(&copy)
        .current_dir(root)
        .env("ZEYN_DEV", "1")
        .spawn()
        .map_err(|e| format!("{}: {e}", copy.display()))
}
