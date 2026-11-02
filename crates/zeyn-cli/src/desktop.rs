//! `zeyn build-desktop`: the release desktop exe of an app, built small, with its size reported.

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

/// Release settings for a small exe: whole program optimization and no debug symbols.
/// Speed stays at `opt-level = 3`.
const SMALL: &[&str] = &["profile.release.lto=true", "profile.release.codegen-units=1", "profile.release.strip=true"];

/// Runs `cargo build --release --features <features>` in `dir` and reports every executable it
/// produced with its size. Icon, version info and manifest come from the app's `build.rs`
/// (see `docs/desktop.md`).
pub fn build(dir: &Path, package: Option<&str>, features: &str) -> Result<String, String> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(dir).args(["build", "--release", "--message-format=json-render-diagnostics", "--features", features]);
    for c in SMALL {
        cmd.args(["--config", c]);
    }
    if let Some(p) = package {
        cmd.args(["-p", p]);
    }
    let out = cmd.stderr(Stdio::inherit()).output().map_err(|e| format!("cargo: {e}"))?;
    if !out.status.success() {
        return Err("the desktop build failed; see the errors above".into());
    }
    let exes = executables(&String::from_utf8_lossy(&out.stdout));
    if exes.is_empty() {
        return Err("the build produced no executable; run it in the app folder or pass --package".into());
    }
    let mut report = String::new();
    for exe in exes {
        let size = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
        report += &format!("{exe}\n  {:.2} MB ({size} bytes)\n", size as f64 / 1_048_576.0);
    }
    Ok(report)
}

/// Paths of the binaries in cargo's JSON messages.
fn executables(messages: &str) -> Vec<String> {
    messages
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|m| m["reason"] == "compiler-artifact" && m["target"]["kind"].as_array().is_some_and(|k| k.iter().any(|k| k == "bin")))
        .filter_map(|m| m["executable"].as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_binary_artifacts() {
        let messages = [
            r#"{"reason":"compiler-artifact","target":{"kind":["lib"]},"executable":null}"#,
            r#"{"reason":"compiler-artifact","target":{"kind":["bin"]},"executable":"C:/t/release/pos.exe"}"#,
            r#"{"reason":"build-script-executed"}"#,
            "not json",
            r#"{"reason":"build-finished","success":true}"#,
        ]
        .join("\n");
        assert_eq!(executables(&messages), ["C:/t/release/pos.exe"]);
    }
}
