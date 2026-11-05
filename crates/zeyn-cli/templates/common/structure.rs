//! Keeps source files small, so code is split into one file per component or screen.
//!
//! The limit is `max_file_lines` under `[package.metadata.zeyn]` in `Cargo.toml`. Raise it there
//! only when a file really cannot be split. Long functions are caught by clippy's
//! `too_many_lines` (threshold in `clippy.toml`).

use std::fs;
use std::path::Path;

/// Default when `Cargo.toml` sets no `max_file_lines`.
const DEFAULT_MAX_FILE_LINES: usize = 300;

/// Reads `max_file_lines` from `[package.metadata.zeyn]`.
fn max_file_lines() -> usize {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .unwrap_or_default();
    manifest
        .lines()
        .skip_while(|l| l.trim() != "[package.metadata.zeyn]")
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "max_file_lines")
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(DEFAULT_MAX_FILE_LINES)
}

/// Collects every `.rs` file under `dir` with its count of non blank lines.
fn sizes(dir: &Path, out: &mut Vec<(String, usize)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sizes(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).unwrap_or_default();
            let lines = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();
            out.push((path.display().to_string(), lines));
        }
    }
}

#[test]
fn source_files_stay_small() {
    let max = max_file_lines();
    let mut files = Vec::new();
    sizes(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let big: Vec<String> = files
        .into_iter()
        .filter(|(_, n)| *n > max)
        .map(|(f, n)| format!("  {f}: {n} lines"))
        .collect();
    assert!(
        big.is_empty(),
        "these files have more than {max} non blank lines:\n{}\n\
         Split them: move each component, screen or group of actions into its own file in \
         the same feature folder and `mod` it from there. Raise `max_file_lines` under \
         `[package.metadata.zeyn]` in Cargo.toml only if the owner agrees.",
        big.join("\n")
    );
}
