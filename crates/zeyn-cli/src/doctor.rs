//! `zeyn doctor`: reads how the project builds and says what makes it slow or large, with a one
//! line fix for each finding. Read only; `--fix` edits only `Cargo.toml` and `.cargo/config.toml`
//! of the project.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Default size above which the target folder is reported, in bytes (20 GB).
pub const DEFAULT_TARGET_LIMIT: u64 = 20 * 1024 * 1024 * 1024;

/// The profile section Cargo reads for the code of the workspace.
const DEV: &str = "[profile.dev]";

/// The profile section Cargo reads for every dependency.
const DEV_DEPS: &str = "[profile.dev.package.\"*\"]";

/// The Windows linker section of `.cargo/config.toml`.
const LINKER_SECTION: &str = "[target.x86_64-pc-windows-msvc]";

/// What the doctor looks at and how.
#[derive(Clone, Debug)]
pub struct Options {
    /// A target folder larger than this many bytes is a finding.
    pub target_limit: u64,
    /// Run the Windows checks (linker, Defender, Dev Drive).
    pub windows: bool,
    /// Run `rustc`, `cargo`, PowerShell and `fsutil` to read versions and Windows settings.
    pub probe_tools: bool,
    /// The target folder to inspect; `CARGO_TARGET_DIR`, else `target`, when `None`.
    pub target: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target_limit: DEFAULT_TARGET_LIMIT,
            windows: cfg!(windows),
            probe_tools: true,
            target: None,
        }
    }
}

/// One thing the doctor found: a stable `code`, a `status` (`ok`, `warn`, `info` or `unknown`),
/// what it saw and a one line fix when there is one.
#[derive(Clone, Debug)]
pub struct Finding {
    pub code: &'static str,
    pub status: &'static str,
    pub message: String,
    pub fix: Option<String>,
}

impl Finding {
    fn new(code: &'static str, status: &'static str, message: impl Into<String>) -> Self {
        Finding {
            code,
            status,
            message: message.into(),
            fix: None,
        }
    }

    fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// Reads the project in `root` and returns the findings, in a fixed order.
pub fn diagnose(root: &Path, options: &Options) -> Vec<Finding> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let target = options
        .target
        .clone()
        .unwrap_or_else(|| target_dir(root));
    let mut findings = vec![
        target_size(&target, options.target_limit),
        dev_debug(&manifest),
        dev_dependencies(&manifest),
        linker(root, options.windows),
        analyzer_target(root),
    ];
    let rustc = options
        .probe_tools
        .then(|| tool_version("rustc"))
        .flatten();
    let cargo = options
        .probe_tools
        .then(|| tool_version("cargo"))
        .flatten();
    findings.push(stale_artifacts(&target, rustc.clone()));
    findings.push(toolchain(root, &manifest, rustc, cargo));
    if options.windows && options.probe_tools {
        findings.push(windows_scan(&target));
    }
    findings
}

/// The findings as the JSON document of `zeyn doctor --json`.
pub fn to_json(findings: &[Finding]) -> Value {
    let list: Vec<Value> = findings
        .iter()
        .map(|f| {
            json!({
                "code": f.code,
                "status": f.status,
                "message": f.message,
                "fix": f.fix,
            })
        })
        .collect();
    json!({ "findings": list })
}

/// The findings as text, one block per finding.
pub fn to_text(findings: &[Finding]) -> String {
    let mut text = String::new();
    for f in findings {
        text.push_str(&format!("{} [{}] {}\n", f.status, f.code, f.message));
        if let Some(fix) = &f.fix {
            text.push_str(&format!("  fix: {fix}\n"));
        }
    }
    text
}

/// Where Cargo writes build output: `CARGO_TARGET_DIR`, else `target` in the project.
fn target_dir(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => root.join("target"),
    }
}

/// Size of the target folder with its largest folders two levels down.
fn target_size(target: &Path, limit: u64) -> Finding {
    if !target.is_dir() {
        return Finding::new(
            "target-size",
            "ok",
            format!("{} does not exist yet", target.display()),
        );
    }
    let mut folders = Vec::new();
    let total = scan(target, 0, &mut folders);
    folders.sort_by_key(|(_, size)| std::cmp::Reverse(*size));
    let largest: Vec<String> = folders
        .iter()
        .take(5)
        .map(|(path, size)| {
            let shown = path
                .strip_prefix(target)
                .unwrap_or(path);
            format!("{} {}", shown.display(), human(*size))
        })
        .collect();
    let message = format!(
        "{} is {}; largest: {}",
        target.display(),
        human(total),
        largest.join(", ")
    );
    if total > limit {
        Finding::new("target-size", "warn", message)
            .fix("run `cargo clean`, or delete the folder; Cargo never removes old artifacts")
    } else {
        Finding::new("target-size", "ok", message)
    }
}

/// Total size of `dir`; folders up to two levels down are collected with their sizes.
fn scan(dir: &Path, depth: usize, folders: &mut Vec<(PathBuf, u64)>) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let size = scan(&entry.path(), depth + 1, folders);
            if (1..=2).contains(&(depth + 1)) {
                folders.push((entry.path(), size));
            }
            total += size;
        } else if kind.is_file() {
            total += entry.metadata().map_or(0, |m| m.len());
        }
    }
    total
}

/// `1.4 GB`, `640 MB`, `12 KB`.
fn human(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", b / 1e3)
    }
}

/// The `key = value` pairs under `header`, trimmed. `None` when the section is missing.
fn section(text: &str, header: &str) -> Option<Vec<(String, String)>> {
    let mut inside = false;
    let mut found = false;
    let mut pairs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == header;
            found |= inside;
        } else if inside && let Some((key, value)) = line.split_once('=') {
            pairs.push((
                key.trim().to_string(),
                value
                    .trim()
                    .trim_matches('"')
                    .to_string(),
            ));
        }
    }
    found.then_some(pairs)
}

/// The value of `key` under `header`.
fn setting(text: &str, header: &str, key: &str) -> Option<String> {
    section(text, header)?
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

/// Debug info of the workspace code in the dev profile.
fn dev_debug(manifest: &str) -> Finding {
    let fix =
        format!("add `debug = \"line-tables-only\"` under {DEV} (or run `zeyn doctor --fix`)");
    match setting(manifest, DEV, "debug").as_deref() {
        Some("line-tables-only" | "false" | "0" | "none" | "line-directives-only") => Finding::new(
            "dev-debuginfo",
            "ok",
            "the dev profile keeps little debug info",
        ),
        Some(other) => Finding::new(
            "dev-debuginfo",
            "warn",
            format!(
                "the dev profile sets debug = {other}, which makes every binary and test larger"
            ),
        )
        .fix(fix),
        None => Finding::new(
            "dev-debuginfo",
            "warn",
            "the dev profile has full debug info, which makes every binary and test larger",
        )
        .fix(fix),
    }
}

/// Optimization and debug info of dependencies in the dev profile.
fn dev_dependencies(manifest: &str) -> Finding {
    let fix = format!(
        "add `opt-level = 1` and `debug = false` under {DEV_DEPS} (or run `zeyn doctor --fix`)"
    );
    let opt = setting(manifest, DEV_DEPS, "opt-level");
    let debug = setting(manifest, DEV_DEPS, "debug");
    match (opt.as_deref(), debug.as_deref()) {
        (Some(o), Some("false" | "0" | "none")) if o != "0" => Finding::new(
            "dev-dependencies",
            "ok",
            format!("dependencies build with opt-level {o} and no debug info"),
        ),
        _ => Finding::new(
            "dev-dependencies",
            "warn",
            "dependencies build with opt-level 0 and debug info: slow to run and large on disk",
        )
        .fix(fix),
    }
}

/// The linker the project asks for.
fn linker(root: &Path, windows: bool) -> Finding {
    let config = std::fs::read_to_string(root.join(".cargo/config.toml"))
        .or_else(|_| std::fs::read_to_string(root.join(".cargo/config")))
        .unwrap_or_default();
    let set = setting(&config, LINKER_SECTION, "linker");
    match (windows, set.as_deref()) {
        (true, Some(l)) if l.contains("rust-lld") => {
            Finding::new("linker", "ok", format!("Windows links with {l}"))
        }
        (true, Some(l)) => Finding::new(
            "linker",
            "info",
            format!("Windows links with {l}; rust-lld is usually faster"),
        )
        .fix(format!(
            "set `linker = \"rust-lld.exe\"` under {LINKER_SECTION} in .cargo/config.toml"
        )),
        (true, None) => Finding::new(
            "linker",
            "warn",
            "Windows links with the default link.exe, which is slow on large binaries",
        )
        .fix(format!(
            "add `linker = \"rust-lld.exe\"` under {LINKER_SECTION} in .cargo/config.toml (or run `zeyn doctor --fix`)"
        )),
        (false, _) => Finding::new(
            "linker",
            "ok",
            "the Windows linker setting does not apply on this system",
        ),
    }
}

/// Whether rust-analyzer builds in its own target folder.
fn analyzer_target(root: &Path) -> Finding {
    let settings = std::fs::read_to_string(root.join(".vscode/settings.json")).unwrap_or_default();
    if settings.contains("rust-analyzer.cargo.targetDir") {
        Finding::new(
            "analyzer-target",
            "ok",
            "rust-analyzer has its own target folder",
        )
    } else {
        Finding::new(
            "analyzer-target",
            "info",
            "rust-analyzer shares the target folder with cargo, so each waits for the other's lock",
        )
        .fix("set \"rust-analyzer.cargo.targetDir\": true in .vscode/settings.json (or the same setting in your editor)")
    }
}

/// First line of `<tool> --version`.
fn tool_version(tool: &str) -> Option<String> {
    let out = Command::new(tool)
        .arg("--version")
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    })
}

/// `1.98` or `1.98.2` from `rustc 1.98.2 (abc 2026-01-01)`, as (major, minor).
fn major_minor(text: &str) -> Option<(u32, u32)> {
    let version = text
        .split_whitespace()
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()))?;
    let mut parts = version
        .split(['.', '-'])
        .map(|p| p.parse::<u32>());
    Some((parts.next()?.ok()?, parts.next()?.ok()?))
}

/// The `rust-version` of the project or of the workspace above it.
fn rust_version(root: &Path, manifest: &str) -> Option<String> {
    let own = |text: &str| {
        text.lines()
            .map(str::trim)
            .find_map(|l| {
                l.strip_prefix("rust-version")
                    .and_then(|r| r.trim_start().strip_prefix('='))
            })
            .map(|v| v.trim().trim_matches('"').to_string())
            .filter(|v| v.starts_with(|c: char| c.is_ascii_digit()))
    };
    own(manifest).or_else(|| {
        let start = root
            .canonicalize()
            .unwrap_or_else(|_| root.to_path_buf());
        start
            .ancestors()
            .skip(1)
            .find_map(|dir| {
                let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
                text.contains("[workspace.package]")
                    .then(|| own(&text))
                    .flatten()
            })
    })
}

/// Installed rustc and cargo against the `rust-version` of the project.
fn toolchain(root: &Path, manifest: &str, rustc: Option<String>, cargo: Option<String>) -> Finding {
    let (Some(rustc), Some(cargo)) = (rustc, cargo) else {
        return Finding::new(
            "toolchain",
            "unknown",
            "could not run rustc and cargo to read their versions",
        )
        .fix("install the Rust toolchain from https://rustup.rs and open a new terminal");
    };
    let wanted = rust_version(root, manifest);
    let wanted_pair = wanted.as_deref().and_then(major_minor);
    match (wanted_pair, major_minor(&rustc)) {
        (Some(w), Some(have)) if have < w => Finding::new(
            "toolchain",
            "warn",
            format!(
                "{rustc} is older than rust-version {}",
                wanted.unwrap_or_default()
            ),
        )
        .fix("run `rustup update stable`"),
        (Some(_), _) => Finding::new(
            "toolchain",
            "ok",
            format!(
                "{rustc}, {cargo}; rust-version {}",
                wanted.unwrap_or_default()
            ),
        ),
        (None, _) => Finding::new(
            "toolchain",
            "ok",
            format!("{rustc}, {cargo}; the project sets no rust-version"),
        ),
    }
}

/// Artifacts a different rustc than the installed one left in the target folder.
fn stale_artifacts(target: &Path, rustc: Option<String>) -> Finding {
    let info = std::fs::read_to_string(target.join(".rustc_info.json")).unwrap_or_default();
    let Some(current) = rustc
        .as_deref()
        .and_then(major_minor_patch)
    else {
        return Finding::new(
            "stale-artifacts",
            "unknown",
            "could not tell which rustc built the target folder",
        );
    };
    let built: Vec<String> = info
        .split("rustc ")
        .skip(1)
        .filter_map(|rest| {
            rest.split_whitespace()
                .next()
                .map(str::to_string)
        })
        .collect();
    if built.is_empty() || built.contains(&current) {
        Finding::new(
            "stale-artifacts",
            "ok",
            "the target folder was built by the installed rustc",
        )
    } else {
        Finding::new(
            "stale-artifacts",
            "warn",
            format!(
                "the target folder was built by rustc {} and the installed one is {current}; the old artifacts are never used again",
                built.join(", ")
            ),
        )
        .fix("run `cargo clean` once")
    }
}

/// `1.98.2` from `rustc 1.98.2 (abc 2026-01-01)`.
fn major_minor_patch(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
}

/// Runs a command and returns its output when it succeeds.
fn run(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// Lower case path with `\` turned into `/` and no `\\?\` prefix, for comparing folders.
fn normal(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

/// Whether the target folder and `~/.cargo` are outside Defender's real time scan: excluded, or
/// on a Dev Drive. Says it could not check when Windows does not allow reading the exclusions.
fn windows_scan(target: &Path) -> Finding {
    let target_abs = target
        .canonicalize()
        .unwrap_or_else(|_| {
            std::env::current_dir()
                .unwrap_or_default()
                .join(target)
        });
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".cargo")))
        .unwrap_or_default();
    let fix = format!(
        "in an administrator PowerShell: Add-MpPreference -ExclusionPath '{}', '{}'; or build on a Dev Drive",
        target_abs
            .to_string_lossy()
            .trim_start_matches(r"\\?\"),
        cargo_home.display()
    );
    let drive = normal(&target_abs)
        .chars()
        .take(2)
        .collect::<String>();
    let dev_drive = run("fsutil", &["devdrv", "query", &drive])
        .is_some_and(|o| o.contains("trusted developer volume"));
    if dev_drive {
        return Finding::new(
            "defender",
            "ok",
            format!("{drive} is a Dev Drive, which Defender scans in performance mode"),
        );
    }
    let listed = run(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-MpPreference).ExclusionPath -join [char]10",
        ],
    );
    let Some(listed) = listed.filter(|l| !l.contains("N/A")) else {
        return Finding::new(
            "defender",
            "unknown",
            "could not check Defender exclusions: Windows lets only an administrator read them",
        )
        .fix(fix);
    };
    let excluded: Vec<String> = listed
        .lines()
        .map(|l| normal(Path::new(l.trim())))
        .filter(|l| !l.is_empty())
        .collect();
    let covered = |path: &Path| {
        let p = normal(path);
        excluded
            .iter()
            .any(|e| p == *e || p.starts_with(&format!("{e}/")))
    };
    let missing: Vec<String> = [&target_abs, &cargo_home]
        .into_iter()
        .filter(|p| !covered(p))
        .map(|p| {
            p.to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string()
        })
        .collect();
    if missing.is_empty() {
        Finding::new(
            "defender",
            "ok",
            "the target folder and ~/.cargo are excluded from Defender scans",
        )
    } else {
        Finding::new(
            "defender",
            "warn",
            format!(
                "Defender scans every file the compiler writes in {}",
                missing.join(" and ")
            ),
        )
        .fix(fix)
    }
}

/// `--fix`: adds the fast dev profile to `Cargo.toml` and the Windows linker to
/// `.cargo/config.toml`. Returns a line per change; running it again changes nothing.
pub fn fix(root: &Path) -> Result<String, String> {
    let mut report = String::new();
    let manifest_path = root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let mut text = manifest.clone();
    for (header, keys) in [
        (DEV, vec![("debug", "\"line-tables-only\"")]),
        (DEV_DEPS, vec![("debug", "false"), ("opt-level", "1")]),
    ] {
        for (key, value) in keys {
            if setting(&text, header, key).is_none() {
                text = set_key(&text, header, key, value);
            }
        }
    }
    if text != manifest {
        std::fs::write(&manifest_path, &text)
            .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
        report.push_str("Cargo.toml: added the fast dev profile\n");
    }
    let config_path = root.join(".cargo/config.toml");
    let config = std::fs::read_to_string(&config_path).unwrap_or_default();
    if setting(&config, LINKER_SECTION, "linker").is_none() {
        let updated = set_key(&config, LINKER_SECTION, "linker", "\"rust-lld.exe\"");
        if let Some(dir) = config_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&config_path, updated)
            .map_err(|e| format!("{}: {e}", config_path.display()))?;
        report.push_str(".cargo/config.toml: set the rust-lld linker for Windows\n");
    }
    if report.is_empty() {
        report.push_str("nothing to fix\n");
    }
    Ok(report)
}

/// Sets `key = value` under `header`: right after the header when the section exists, else in
/// a new section at the end of the text.
fn set_key(text: &str, header: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = String::new();
    let mut placed = false;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        if !placed && line.trim() == header {
            if !line.ends_with('\n') {
                out.push_str(newline);
            }
            out.push_str(&format!("{key} = {value}{newline}"));
            placed = true;
        }
    }
    if placed {
        return out;
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(newline);
    }
    if !out.is_empty() {
        out.push_str(newline);
    }
    out.push_str(&format!("{header}{newline}{key} = {value}{newline}"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(tag: &str, manifest: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("zeyn-doctor-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Cargo.toml"), manifest).unwrap();
        root
    }

    fn offline() -> Options {
        Options {
            target_limit: 1000,
            windows: true,
            probe_tools: false,
            target: Some(PathBuf::from("zeyn-doctor-no-such-target")),
        }
    }

    fn status(findings: &[Finding], code: &str) -> &'static str {
        findings
            .iter()
            .find(|f| f.code == code)
            .map_or("missing", |f| f.status)
    }

    const FAST: &str = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[profile.dev]\ndebug = \"line-tables-only\"\n\n[profile.dev.package.\"*\"]\ndebug = false\nopt-level = 1\n";

    #[test]
    fn a_project_without_the_dev_profile_and_linker_gets_warnings_with_fixes() {
        let root = project("plain", "[package]\nname = \"a\"\nversion = \"0.1.0\"\n");
        let found = diagnose(&root, &offline());
        assert_eq!(status(&found, "dev-debuginfo"), "warn");
        assert_eq!(status(&found, "dev-dependencies"), "warn");
        assert_eq!(status(&found, "linker"), "warn");
        assert_eq!(status(&found, "analyzer-target"), "info");
        for f in found
            .iter()
            .filter(|f| f.status == "warn")
        {
            assert!(f.fix.is_some(), "{}", f.code);
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_tuned_project_has_no_warnings() {
        let root = project("tuned", FAST);
        std::fs::create_dir_all(root.join(".cargo")).unwrap();
        std::fs::write(
            root.join(".cargo/config.toml"),
            "[target.x86_64-pc-windows-msvc]\nlinker = \"rust-lld.exe\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".vscode")).unwrap();
        std::fs::write(
            root.join(".vscode/settings.json"),
            "{ \"rust-analyzer.cargo.targetDir\": true }",
        )
        .unwrap();
        let found = diagnose(&root, &offline());
        let bad: Vec<_> = found
            .iter()
            .filter(|f| f.status == "warn" || f.status == "info")
            .map(|f| f.code)
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_wrong_linker_is_reported_on_windows_only() {
        let root = project("linker", FAST);
        std::fs::create_dir_all(root.join(".cargo")).unwrap();
        std::fs::write(
            root.join(".cargo/config.toml"),
            "[target.x86_64-pc-windows-msvc]\nlinker = \"link.exe\"\n",
        )
        .unwrap();
        let found = diagnose(&root, &offline());
        let linker = found
            .iter()
            .find(|f| f.code == "linker")
            .unwrap();
        assert_eq!(linker.status, "info");
        assert!(linker.message.contains("link.exe"));
        let elsewhere = diagnose(
            &root,
            &Options {
                windows: false,
                ..offline()
            },
        );
        assert_eq!(status(&elsewhere, "linker"), "ok");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_target_folder_over_the_limit_is_reported_with_its_largest_folders() {
        let root = project("big", FAST);
        let deps = root.join("target/debug/deps");
        std::fs::create_dir_all(&deps).unwrap();
        std::fs::write(deps.join("blob"), vec![0u8; 5000]).unwrap();
        let target = root.join("target");
        let found = target_size(&target, 1000);
        assert_eq!(found.status, "warn");
        assert!(found.message.contains("debug"), "{}", found.message);
        assert!(found.fix.is_some());
        assert_eq!(target_size(&target, 1_000_000).status, "ok");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn artifacts_of_another_rustc_are_stale() {
        let root = project("stale", FAST);
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join(".rustc_info.json"),
            "{\"outputs\":{\"1\":{\"stdout\":\"rustc 1.50.0 (abc 2021-01-01)\\n\"}}}",
        )
        .unwrap();
        let old = stale_artifacts(&target, Some("rustc 1.98.0 (def 2026-01-01)".into()));
        assert_eq!(old.status, "warn");
        let same = stale_artifacts(&target, Some("rustc 1.50.0 (abc 2021-01-01)".into()));
        assert_eq!(same.status, "ok");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_older_rustc_than_rust_version_is_reported() {
        let root = project("rv", "[package]\nname = \"a\"\nrust-version = \"1.98\"\n");
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let old = toolchain(
            &root,
            &manifest,
            Some("rustc 1.90.1 (x)".into()),
            Some("cargo 1.90.1 (x)".into()),
        );
        assert_eq!(old.status, "warn");
        let new = toolchain(
            &root,
            &manifest,
            Some("rustc 1.98.0 (x)".into()),
            Some("cargo 1.98.0 (x)".into()),
        );
        assert_eq!(new.status, "ok");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn fix_adds_the_profile_and_linker_once() {
        let root = project(
            "fix",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n[profile.dev]\nopt-level = 0\n",
        );
        let first = fix(&root).unwrap();
        assert!(first.contains("Cargo.toml") && first.contains(".cargo/config.toml"));
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("opt-level = 0"), "{manifest}");
        assert_eq!(
            setting(&manifest, DEV, "debug").as_deref(),
            Some("line-tables-only")
        );
        assert_eq!(
            setting(&manifest, DEV_DEPS, "opt-level").as_deref(),
            Some("1")
        );
        assert_eq!(
            setting(&manifest, DEV_DEPS, "debug").as_deref(),
            Some("false")
        );
        let config = std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap();
        assert_eq!(
            setting(&config, LINKER_SECTION, "linker").as_deref(),
            Some("rust-lld.exe")
        );
        let again = fix(&root).unwrap();
        assert_eq!(again, "nothing to fix\n");
        assert_eq!(
            std::fs::read_to_string(root.join("Cargo.toml")).unwrap(),
            manifest
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".cargo/config.toml")).unwrap(),
            config
        );
        let found = diagnose(&root, &offline());
        assert_eq!(status(&found, "dev-debuginfo"), "ok");
        assert_eq!(status(&found, "dev-dependencies"), "ok");
        assert_eq!(status(&found, "linker"), "ok");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn json_output_carries_stable_codes() {
        let root = project("json", FAST);
        let json = to_json(&diagnose(&root, &offline()));
        let codes: Vec<&str> = json["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["code"].as_str().unwrap())
            .collect();
        for code in [
            "target-size",
            "dev-debuginfo",
            "dev-dependencies",
            "linker",
            "analyzer-target",
            "toolchain",
            "stale-artifacts",
        ] {
            assert!(codes.contains(&code), "{code} in {codes:?}");
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
