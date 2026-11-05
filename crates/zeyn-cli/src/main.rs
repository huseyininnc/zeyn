//! `zeyn`: new projects, generators, checks and an MCP server for AI agents.

mod desktop;
mod dev;
mod generate;
mod islands;

use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod license;

const TEMPLATES: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../templates/Cargo.toml.tmpl")),
    ("AGENTS.md", include_str!("../templates/AGENTS.md.tmpl")),
    ("CLAUDE.md", include_str!("../templates/CLAUDE.md.tmpl")),
    (".mcp.json", include_str!("../templates/mcp.json.tmpl")),
    (".gitignore", include_str!("../templates/gitignore.tmpl")),
    (
        ".gitlab-ci.yml",
        include_str!("../templates/gitlab-ci.yml.tmpl"),
    ),
    (
        ".github/workflows/ci.yml",
        include_str!("../templates/github/workflows/ci.yml"),
    ),
    (
        ".github/dependabot.yml",
        include_str!("../templates/github/dependabot.yml"),
    ),
    (
        ".github/pull_request_template.md",
        include_str!("../templates/github/pull_request_template.md"),
    ),
    ("Jenkinsfile", include_str!("../templates/Jenkinsfile")),
    (
        "sonar-project.properties",
        include_str!("../templates/sonar-project.properties.tmpl"),
    ),
    ("SECURITY.md", include_str!("../templates/SECURITY.md.tmpl")),
    (
        "CHANGELOG.md",
        include_str!("../templates/CHANGELOG.md.tmpl"),
    ),
    ("rustfmt.toml", include_str!("../templates/rustfmt.toml")),
    (
        ".editorconfig",
        include_str!("../templates/editorconfig.tmpl"),
    ),
    (
        ".claude/skills/zeyn/SKILL.md",
        include_str!("../embedded/SKILL.md"),
    ),
    (
        ".gitattributes",
        include_str!("../templates/gitattributes.tmpl"),
    ),
    (
        "rust-toolchain.toml",
        include_str!("../templates/rust-toolchain.toml"),
    ),
    ("clippy.toml", include_str!("../templates/clippy.toml")),
    ("deny.toml", include_str!("../templates/deny.toml")),
    ("Dockerfile", include_str!("../templates/Dockerfile.tmpl")),
    (
        ".dockerignore",
        include_str!("../templates/dockerignore.tmpl"),
    ),
    (
        "config/app.json",
        include_str!("../templates/config/app.json"),
    ),
    (
        "locales/en.json",
        include_str!("../templates/locales/en.json"),
    ),
    (
        "locales/tr.json",
        include_str!("../templates/locales/tr.json"),
    ),
    ("src/main.rs", include_str!("../templates/src/main.rs")),
    ("src/app.rs", include_str!("../templates/src/app.rs")),
    (
        "src/features/mod.rs",
        include_str!("../templates/src/features/mod.rs"),
    ),
    (
        "src/features/home/mod.rs",
        include_str!("../templates/src/features/home/mod.rs"),
    ),
    (
        "src/features/notes/mod.rs",
        include_str!("../templates/src/features/notes/mod.rs"),
    ),
    (
        "src/features/notes/models.rs",
        include_str!("../templates/src/features/notes/models.rs"),
    ),
    (
        "src/features/notes/store.rs",
        include_str!("../templates/src/features/notes/store.rs"),
    ),
    (
        "src/features/notes/actions.rs",
        include_str!("../templates/src/features/notes/actions.rs"),
    ),
    (
        "src/features/notes/screens.rs",
        include_str!("../templates/src/features/notes/screens.rs"),
    ),
    ("tests/app.rs", include_str!("../templates/tests/app.rs")),
    (
        "tests/structure.rs",
        include_str!("../templates/common/structure.rs"),
    ),
];

const LIB_TEMPLATES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        include_str!("../templates/lib/Cargo.toml.tmpl"),
    ),
    ("AGENTS.md", include_str!("../templates/lib/AGENTS.md.tmpl")),
    ("CLAUDE.md", include_str!("../templates/CLAUDE.md.tmpl")),
    (
        ".gitignore",
        include_str!("../templates/lib/gitignore.tmpl"),
    ),
    (
        ".gitlab-ci.yml",
        include_str!("../templates/gitlab-ci.yml.tmpl"),
    ),
    (
        ".github/workflows/ci.yml",
        include_str!("../templates/github/workflows/ci.yml"),
    ),
    (
        ".github/dependabot.yml",
        include_str!("../templates/github/dependabot.yml"),
    ),
    (
        ".github/pull_request_template.md",
        include_str!("../templates/github/pull_request_template.md"),
    ),
    ("Jenkinsfile", include_str!("../templates/Jenkinsfile")),
    (
        "sonar-project.properties",
        include_str!("../templates/sonar-project.properties.tmpl"),
    ),
    ("SECURITY.md", include_str!("../templates/SECURITY.md.tmpl")),
    (
        "CHANGELOG.md",
        include_str!("../templates/CHANGELOG.md.tmpl"),
    ),
    ("rustfmt.toml", include_str!("../templates/rustfmt.toml")),
    (
        ".editorconfig",
        include_str!("../templates/editorconfig.tmpl"),
    ),
    (
        ".claude/skills/zeyn/SKILL.md",
        include_str!("../embedded/SKILL.md"),
    ),
    (
        ".gitattributes",
        include_str!("../templates/gitattributes.tmpl"),
    ),
    (
        "rust-toolchain.toml",
        include_str!("../templates/rust-toolchain.toml"),
    ),
    ("clippy.toml", include_str!("../templates/clippy.toml")),
    ("deny.toml", include_str!("../templates/deny.toml")),
    (
        "locales/en.json",
        include_str!("../templates/lib/locales/en.json"),
    ),
    (
        "locales/tr.json",
        include_str!("../templates/lib/locales/tr.json"),
    ),
    ("src/lib.rs", include_str!("../templates/lib/src/lib.rs")),
    (
        "src/customers/mod.rs",
        include_str!("../templates/lib/src/customers/mod.rs"),
    ),
    (
        "src/customers/models.rs",
        include_str!("../templates/lib/src/customers/models.rs"),
    ),
    (
        "src/customers/store.rs",
        include_str!("../templates/lib/src/customers/store.rs"),
    ),
    (
        "src/customers/actions.rs",
        include_str!("../templates/lib/src/customers/actions.rs"),
    ),
    (
        "src/customers/screens.rs",
        include_str!("../templates/lib/src/customers/screens.rs"),
    ),
    (
        "tests/lib.rs",
        include_str!("../templates/lib/tests/lib.rs"),
    ),
    (
        "tests/structure.rs",
        include_str!("../templates/common/structure.rs"),
    ),
];

const DOCS: &[(&str, &str)] = &[
    ("rules", include_str!("../templates/AGENTS.md.tmpl")),
    ("language", include_str!("../embedded/SKILL.md")),
    ("protocol", include_str!("../embedded/runtime-protocol.md")),
];

const HELP: &str = "zeyn: command line tool for Zeyn Framework

Usage:
  zeyn new <name> [--zeyn-path <path>] [--no-git]
                                          create an app with the standard feature structure, CI for
                                          GitHub Actions, GitLab and Jenkins, SonarQube settings, and a
                                          local git repository on branch main when git is installed
  zeyn new <name> --lib [--zeyn-path <path>] [--no-git]
                                          the same for a shared library (models, features, texts)
  zeyn g feature <name>                   add src/features/<name> with models, actions and screens,
                                          one file per screen and action (screens/list.rs, ...)
  zeyn g screen <feature> <name>          add src/features/<feature>/screens/<name>.rs, register it in
                                          the feature and add its title to every file in locales/
  zeyn g action <feature> <name>          add src/features/<feature>/actions/<name>.rs and register it
  zeyn g component <feature> <name>       add src/features/<feature>/components/<name>.rs, a function
                                          that returns a Node (a toolbar, a panel)
                                          Short forms: g f, g s, g a, g c. A flat screens.rs or
                                          actions.rs is first moved to screens/mod.rs (actions/mod.rs)
  zeyn routes                             list features, screens, routes, APIs and access
  zeyn check [--fix]                      compile and report structural problems and missing translations;
                                          warns about unregistered screens and actions and crowded flat
                                          screens.rs or actions.rs files;
                                          --fix first rewrites renamed Zeyn APIs in src, tests and examples
  zeyn dev                                run the app, rebuild and restart on every change; the browser
                                          follows without a page reload
  zeyn build-desktop [--package <name>] [--features <list>]
                                          release desktop exe (feature `desktop` by default, see
                                          docs/desktop.md), small profile, size reported
  zeyn build-islands [--out <dir>] [<crate dir>...]
                                          compile WASM islands (default: every crate in islands/)
                                          to hashed files and a manifest in dist/islands
  zeyn mcp                                MCP server on stdio for AI agents
  zeyn license keygen [--out <file>]      new license signing key pair; the private key goes to
                                          zeyn-license.key, the public key to embed is printed
  zeyn license sign <json> [--key <file>] [--out <file>]
                                          sign a license (a JSON file or inline JSON with holder,
                                          valid_until, modules, max_users); key from --key,
                                          ZEYN_LICENSE_KEY or zeyn-license.key
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .collect();
    let result = match words.as_slice() {
        ["new", name, rest @ ..] => new_project(
            name,
            flag(rest, "--zeyn-path"),
            rest.contains(&"--lib"),
            !rest.contains(&"--no-git"),
        ),
        ["g" | "generate", kind, name] if generate::kind_of(kind) == Some("feature") => {
            new_feature(Path::new("."), name).map(|m| println!("{m}"))
        }
        ["g" | "generate", kind, feature, name] => {
            match generate::kind_of(kind).and_then(generate::Kind::named) {
                Some(k) => {
                    generate::generate(Path::new("."), k, feature, name).map(|m| println!("{m}"))
                }
                None => Err(format!(
                    "unknown generator `{kind}`; use feature (f), screen (s), action (a) or component (c)"
                )),
            }
        }
        ["routes"] => describe(Path::new(".")).map(|d| print!("{}", routes_table(&d))),
        ["check", rest @ ..] => fix_if(Path::new("."), rest.contains(&"--fix"))
            .and_then(|fixed| check(Path::new(".")).map(|r| fixed + &r))
            .and_then(|report| {
                print!("{report}");
                if report
                    .lines()
                    .any(|l| l.starts_with("error") || l.contains(": error"))
                {
                    Err("the app has problems".into())
                } else {
                    Ok(())
                }
            }),
        ["dev"] => dev::run(Path::new(".")),
        ["build-desktop", rest @ ..] => desktop::build(
            Path::new("."),
            flag(rest, "--package"),
            flag(rest, "--features").unwrap_or("desktop"),
        )
        .map(|r| print!("{r}")),
        ["build-islands", rest @ ..] => build_islands(rest).map(|r| println!("{r}")),
        ["mcp"] => mcp(),
        ["license", "keygen", rest @ ..] => license::keygen(Path::new(
            flag(rest, "--out").unwrap_or(license::DEFAULT_KEY),
        ))
        .map(|m| print!("{m}")),
        ["license", "sign", json, rest @ ..] => {
            license::sign(json, flag(rest, "--key").map(Path::new)).and_then(|signed| {
                match flag(rest, "--out") {
                    Some(out) => {
                        std::fs::write(out, signed + "\n").map_err(|e| format!("{out}: {e}"))
                    }
                    None => {
                        println!("{signed}");
                        Ok(())
                    }
                }
            })
        }
        _ => {
            print!("{HELP}");
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `zeyn build-islands [--out <dir>] [<crate dir>...]`.
fn build_islands(rest: &[&str]) -> Result<String, String> {
    let out = PathBuf::from(flag(rest, "--out").unwrap_or(islands::OUT_DIR));
    let mut named = Vec::new();
    let mut skip = false;
    for a in rest {
        if std::mem::take(&mut skip) {
            continue;
        }
        if *a == "--out" {
            skip = true;
        } else {
            named.push(PathBuf::from(a));
        }
    }
    let crates = if named.is_empty() {
        islands::default_crates(Path::new("."))
    } else {
        named
    };
    islands::build(&crates, &out)
}

fn flag<'a>(rest: &[&'a str], name: &str) -> Option<&'a str> {
    rest.iter()
        .position(|a| *a == name)
        .and_then(|i| rest.get(i + 1).copied())
}

/// `notes`, `sales_orders`: lower case letters, digits and `_`, starting with a letter.
fn valid_ident(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `acme-portal` becomes `Acme Portal`.
fn title_of(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn new_project(name: &str, zeyn_path: Option<&str>, lib: bool, git: bool) -> Result<(), String> {
    new_project_at(&PathBuf::from(name), name, zeyn_path, lib, git)
}

/// Writes a new project named `name` into `root`.
fn new_project_at(
    root: &Path,
    name: &str,
    zeyn_path: Option<&str>,
    lib: bool,
    git: bool,
) -> Result<(), String> {
    let ok = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        return Err(format!(
            "`{name}` is not a valid project name; use lower case letters, digits, `-` and `_`"
        ));
    }
    if root.exists() {
        return Err(format!("`{name}` already exists"));
    }
    let dep = match zeyn_path {
        Some(p) => {
            let abs = std::fs::canonicalize(p).map_err(|e| format!("--zeyn-path {p}: {e}"))?;
            let shown = abs
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            format!("zeyn = {{ path = \"{shown}\" }}")
        }
        None => format!("zeyn = \"{}\"", env!("CARGO_PKG_VERSION")),
    };
    let title = title_of(name);
    let crate_name = name.replace('-', "_");
    let templates = if lib { LIB_TEMPLATES } else { TEMPLATES };
    for (path, body) in templates {
        let target = root.join(path);
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = body
            .replace("{{name}}", name)
            .replace("{{crate}}", &crate_name)
            .replace("{{title}}", &title)
            .replace("{{zeyn_dep}}", &dep);
        std::fs::write(&target, text).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    if git {
        println!("{}", git_init(root));
    }
    if lib {
        println!(
            "Created library {name}/\n\n  cd {name}\n  cargo test\n\nUse it from an app: {crate_name} = {{ git = \"<repo url>\", tag = \"v0.1.0\" }}"
        );
    } else {
        println!(
            "Created {name}/\n\n  cd {name}\n  cargo run\n\nAgents: read {name}/AGENTS.md; the zeyn MCP server is set up in .mcp.json."
        );
    }
    Ok(())
}

/// Starts a local git repository on branch `main` in a new project, when git is installed and the
/// project is not inside another repository. Never commits: the author's name and email are the
/// user's to set. Returns the line to print.
fn git_init(root: &Path) -> String {
    let run = |dir: &Path, args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    let here = root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !run(here, &["--version"]) {
        return "git is not installed, so no repository was created; run `git init -b main` later"
            .into();
    }
    if run(here, &["rev-parse", "--is-inside-work-tree"]) {
        return "Inside an existing git repository, so no new one was created".into();
    }
    let started = run(root, &["init", "-b", "main"])
        || (run(root, &["init"]) && run(root, &["symbolic-ref", "HEAD", "refs/heads/main"]));
    if started {
        "Started a git repository on branch main; nothing is committed yet".into()
    } else {
        "git init failed; run `git init -b main` in the project".into()
    }
}

/// Adds `src/features/<name>` with one file per screen and action (`screens/list.rs`,
/// `actions/search.rs`) and registers the module. Returns what is left to do by hand.
fn new_feature(root: &Path, name: &str) -> Result<String, String> {
    if !valid_ident(name) {
        return Err(format!(
            "`{name}` is not a valid feature name; use lower case letters, digits and `_`"
        ));
    }
    let dir = root.join("src/features").join(name);
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    let model = title_of(name).replace(' ', "");
    let files = [
        (
            "mod.rs",
            format!(
                "//! {model}.\n\npub mod actions;\npub mod models;\npub mod screens;\n\nuse zeyn::prelude::*;\n\npub fn feature() -> Feature {{\n    Feature::new(\"{name}\")\n        .route_prefix(\"/{name}\")\n        .guard(SignedIn)\n        .screen(screens::{name}_list)\n}}\n"
            ),
        ),
        (
            "models.rs",
            format!(
                "use zeyn::prelude::*;\n\n/// A {name} record.\n#[entity(table = \"{name}\")]\npub struct {model} {{\n    #[key]\n    pub id: {model}Id,\n    #[rule(required, max_len = 120)]\n    pub name: String,\n    #[version]\n    pub version: i64,\n}}\n"
            ),
        ),
        (
            "actions/mod.rs",
            format!("mod search;\npub use search::search_{name};\n"),
        ),
        (
            "actions/search.rs",
            format!(
                "use super::super::models::{model};\nuse zeyn::prelude::*;\n\n/// Records for the list. Replace the empty page with your store or database.\n#[action(permission = \"{name}.record.read\")]\npub async fn search_{name}(q: DataQuery) -> Result<Page<{model}>> {{\n    Ok(q.apply(Vec::new()))\n}}\n"
            ),
        ),
        (
            "screens/mod.rs",
            format!("mod list;\npub use list::{name}_list;\n"),
        ),
        (
            "screens/list.rs",
            format!(
                "use super::super::actions::search_{name};\nuse zeyn::prelude::*;\n\n/// List screen.\n#[screen(route = \"/\", permission = \"{name}.record.read\")]\npub fn {name}_list() -> Screen {{\n    Screen::new(\"{model}\").body(DataGrid::auto(search_{name}).mode(DataMode::Server))\n}}\n"
            ),
        ),
    ];
    for (file, body) in files {
        let path = dir.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, body).map_err(|e| e.to_string())?;
    }
    let mod_rs = root.join("src/features/mod.rs");
    let mut text = std::fs::read_to_string(&mod_rs).unwrap_or_default();
    if !text.ends_with('\n') && !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&format!("pub mod {name};\n"));
    std::fs::write(&mod_rs, text).map_err(|e| e.to_string())?;
    Ok(format!(
        "Created src/features/{name}/ and registered it in src/features/mod.rs.\nNext: add `.feature(features::{name}::feature())` in src/app.rs, add texts to locales/, and `cargo test`."
    ))
}

/// Runs the app with `ZEYN_DESCRIBE=1`, which prints its description instead of serving.
fn describe(root: &Path) -> Result<Value, String> {
    let out = Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(root)
        .env("ZEYN_DESCRIBE", "1")
        .output()
        .map_err(|e| format!("cargo could not be started: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr)
            .trim()
            .to_string());
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("the app did not print a description: {e}"))
}

fn routes_table(d: &Value) -> String {
    let mut out = String::new();
    for f in d["features"]
        .as_array()
        .into_iter()
        .flatten()
    {
        out.push_str(&format!(
            "feature {}  guards: {}\n",
            f["name"].as_str().unwrap_or(""),
            f["guards"]
        ));
        for s in f["screens"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let access = s["access"]["permission"]
                .as_str()
                .unwrap_or("public");
            out.push_str(&format!(
                "  screen {:<28} {:<32} {access}\n",
                s["name"].as_str().unwrap_or(""),
                s["path"].as_str().unwrap_or("(window)")
            ));
        }
        for a in f["apis"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let access = a["access"]["permission"]
                .as_str()
                .unwrap_or("public");
            out.push_str(&format!(
                "  api    {:<28} POST {:<27} {access}\n",
                a["action"].as_str().unwrap_or(""),
                a["path"].as_str().unwrap_or("")
            ));
        }
    }
    out
}

/// Zeyn names that changed, old to new. `zeyn check --fix` rewrites them.
const RENAMES: &[(&str, &str)] = &[
    ("GridQuery", "DataQuery"),
    ("GridMode", "DataMode"),
    ("GridColumn", "DataGridColumn"),
    ("Grid", "DataGrid"),
    ("MAX_EMBED_CONSENT", "MAX_PREF_LIFETIME"),
];

/// Runs [`fix_renames`] when asked and describes what changed.
fn fix_if(root: &Path, fix: bool) -> Result<String, String> {
    if !fix {
        return Ok(String::new());
    }
    let changed = fix_renames(root)?;
    Ok(changed
        .iter()
        .map(|(path, n)| format!("fixed: {n} renamed names in {}\n", path.display()))
        .collect())
}

/// Rewrites renamed Zeyn names in the `.rs` files under `src`, `tests` and `examples`.
fn fix_renames(root: &Path) -> Result<Vec<(PathBuf, usize)>, String> {
    let mut files = Vec::new();
    for dir in ["src", "tests", "examples"] {
        rust_files(&root.join(dir), &mut files);
    }
    let mut changed = Vec::new();
    for path in files {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (fixed, n) = rename_idents(&text, RENAMES);
        if n > 0 {
            std::fs::write(&path, fixed).map_err(|e| format!("{}: {e}", path.display()))?;
            changed.push((path, n));
        }
    }
    Ok(changed)
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != "target")
            {
                rust_files(&path, out);
            }
        } else if path
            .extension()
            .is_some_and(|e| e == "rs")
        {
            out.push(path);
        }
    }
}

/// Replaces whole identifiers outside string and char literals. Comments are rewritten too, so
/// doc examples follow. Returns the text and how many identifiers changed.
fn rename_idents(text: &str, renames: &[(&str, &str)]) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"'
            || (c == 'r' && matches!(chars.get(i + 1), Some('"' | '#')) && !ident_before(&chars, i))
        {
            let end = string_end(&chars, i);
            out.extend(&chars[i..end]);
            i = end;
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            out.extend(&chars[i..i + 3]);
            i += 3;
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match renames
                .iter()
                .find(|(old, _)| *old == word)
            {
                Some((_, new)) => {
                    out.push_str(new);
                    count += 1;
                }
                None => out.push_str(&word),
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    (out, count)
}

fn ident_before(chars: &[char], i: usize) -> bool {
    i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
}

/// End (exclusive) of the string literal starting at `i`: `"..."` with escapes, or `r#"..."#`.
fn string_end(chars: &[char], i: usize) -> usize {
    let mut j = i;
    let mut hashes = 0;
    let raw = chars[j] == 'r';
    if raw {
        j += 1;
        while chars.get(j) == Some(&'#') {
            hashes += 1;
            j += 1;
        }
        if chars.get(j) != Some(&'"') {
            return i + 1;
        }
    }
    j += 1;
    while j < chars.len() {
        match chars[j] {
            '\\' if !raw => j += 2,
            '"' if (1..=hashes).all(|h| chars.get(j + h) == Some(&'#')) => return j + 1 + hashes,
            _ => j += 1,
        }
    }
    chars.len()
}

/// Compiles, then reports structural problems and missing translations.
fn check(root: &Path) -> Result<String, String> {
    let out = Command::new("cargo")
        .args(["check", "--quiet", "--message-format=short"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cargo could not be started: {e}"))?;
    let mut report = String::new();
    if !out.status.success() {
        report.push_str(&String::from_utf8_lossy(&out.stderr));
        return Ok(report);
    }
    let d = describe(root)?;
    for p in d["problems"]
        .as_array()
        .into_iter()
        .flatten()
    {
        report.push_str(&format!("error: {}\n", p.as_str().unwrap_or("")));
    }
    for m in d["missing_translations"]
        .as_array()
        .into_iter()
        .flatten()
    {
        report.push_str(&format!(
            "warning: translation `{}` is missing in `{}`\n",
            m[1].as_str().unwrap_or(""),
            m[0].as_str().unwrap_or("")
        ));
    }
    let registered: Vec<String> = d["features"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|f| {
            let screens = f["screens"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| &s["name"]);
            let apis = f["apis"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| &a["action"]);
            screens.chain(apis)
        })
        .filter_map(|n| n.as_str().map(str::to_string))
        .collect();
    let mut paths = Vec::new();
    rust_files(&root.join("src"), &mut paths);
    let files: Vec<(PathBuf, String)> = paths
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            Some((
                p.strip_prefix(root)
                    .unwrap_or(&p)
                    .to_path_buf(),
                text,
            ))
        })
        .collect();
    for w in generate::layout_warnings(&files, &registered) {
        report.push_str(&format!("warning: {w}\n"));
    }
    if report.is_empty() {
        report.push_str("ok: no problems found\n");
    }
    Ok(report)
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({ "name": name, "description": description, "inputSchema": { "type": "object", "properties": properties, "required": required } })
}

fn tools() -> Value {
    json!([
        tool(
            "zeyn_docs",
            "Zeyn framework reference. Topics: rules (project rules for agents), language (how to write models, actions, screens, forms, grids), protocol (browser runtime protocol), architecture.",
            json!({ "topic": { "type": "string", "enum": ["rules", "language", "protocol"] } }),
            &["topic"]
        ),
        tool(
            "zeyn_describe",
            "Describe the app in the current directory: features, screens, routes, access, APIs, problems and missing translations.",
            json!({}),
            &[]
        ),
        tool(
            "zeyn_check",
            "Compile the app and report compiler errors, structural problems and missing translations. With fix: true, first rewrite renamed Zeyn APIs (Grid to DataGrid and others).",
            json!({ "fix": { "type": "boolean" } }),
            &[]
        ),
        tool(
            "zeyn_test",
            "Run cargo test and return the end of the output.",
            json!({}),
            &[]
        ),
        tool(
            "zeyn_new_feature",
            "Create src/features/<name> with models, actions and screens (one file per screen and action) and register the module.",
            json!({ "name": { "type": "string", "description": "lower case, digits and _" } }),
            &["name"]
        ),
        tool(
            "zeyn_new_screen",
            "Create src/features/<feature>/screens/<name>.rs with a #[screen] skeleton, register it in the feature and add its title to every locale file. A flat screens.rs is first moved to screens/mod.rs.",
            generator_args(),
            &["feature", "name"]
        ),
        tool(
            "zeyn_new_action",
            "Create src/features/<feature>/actions/<name>.rs with an #[action] skeleton and register it in the feature. A flat actions.rs is first moved to actions/mod.rs.",
            generator_args(),
            &["feature", "name"]
        ),
        tool(
            "zeyn_new_component",
            "Create src/features/<feature>/components/<name>.rs with a function that returns a Node (a part of a screen such as a toolbar or a panel) and wire components/mod.rs.",
            generator_args(),
            &["feature", "name"]
        ),
    ])
}

fn generator_args() -> Value {
    json!({
        "feature": { "type": "string", "description": "an existing feature in src/features" },
        "name": { "type": "string", "description": "snake case: lower case, digits and _" }
    })
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    let root = Path::new(".");
    match name {
        "zeyn_docs" => {
            let topic = args["topic"]
                .as_str()
                .unwrap_or("rules");
            DOCS.iter()
                .find(|(t, _)| *t == topic)
                .map(|(_, d)| d.to_string())
                .ok_or_else(|| format!("unknown topic `{topic}`"))
        }
        "zeyn_describe" => {
            describe(root).map(|d| serde_json::to_string_pretty(&d).unwrap_or_default())
        }
        "zeyn_check" => fix_if(root, args["fix"].as_bool().unwrap_or(false))
            .and_then(|fixed| check(root).map(|r| fixed + &r)),
        "zeyn_test" => {
            let out = Command::new("cargo")
                .args(["test", "--quiet"])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            let tail: String = text
                .lines()
                .rev()
                .take(80)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            Ok(format!(
                "{}\n{tail}",
                if out.status.success() {
                    "tests passed"
                } else {
                    "tests failed"
                }
            ))
        }
        "zeyn_new_feature" => new_feature(root, args["name"].as_str().unwrap_or("")),
        "zeyn_new_screen" | "zeyn_new_action" | "zeyn_new_component" => generate::generate(
            root,
            generate::Kind::named(name.trim_start_matches("zeyn_new_"))
                .unwrap_or(generate::Kind::Screen),
            args["feature"].as_str().unwrap_or(""),
            args["name"].as_str().unwrap_or(""),
        ),
        _ => Err(format!("unknown tool `{name}`")),
    }
}

/// MCP over stdio: one JSON-RPC message per line.
fn mcp() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = req.get("id").cloned() else {
            continue;
        };
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "zeyn", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Zeyn framework tools. Read zeyn_docs topic rules before changing code. Run zeyn_check and zeyn_test before finishing."
            }),
            "tools/list" => json!({ "tools": tools() }),
            "tools/call" => {
                match call_tool(
                    req["params"]["name"]
                        .as_str()
                        .unwrap_or(""),
                    &req["params"]["arguments"],
                ) {
                    Ok(text) => {
                        json!({ "content": [{ "type": "text", "text": text }], "isError": false })
                    }
                    Err(e) => {
                        json!({ "content": [{ "type": "text", "text": e }], "isError": true })
                    }
                }
            }
            "ping" => json!({}),
            other => {
                let reply = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("method `{other}` not found") } });
                writeln!(stdout, "{reply}").map_err(|e| e.to_string())?;
                stdout
                    .flush()
                    .map_err(|e| e.to_string())?;
                continue;
            }
        };
        writeln!(
            stdout,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "result": result })
        )
        .map_err(|e| e.to_string())?;
        stdout
            .flush()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    /// The copies in `embedded/` ship with the crate; in the repository they must match the originals.
    #[test]
    fn embedded_copies_match_the_repository() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for (name, original) in [
            ("SKILL.md", "../../skills/zeyn/SKILL.md"),
            ("runtime-protocol.md", "../../docs/runtime-protocol.md"),
        ] {
            let Ok(original) = std::fs::read_to_string(root.join(original)) else {
                continue;
            };
            let copy = std::fs::read_to_string(root.join("embedded").join(name)).unwrap();
            assert!(
                copy == original,
                "crates/zeyn-cli/embedded/{name} is out of date: copy the file from the repository root"
            );
        }
    }

    #[test]
    fn renames_identifiers_but_not_strings_or_longer_names() {
        let src = r##"use zeyn::{Grid, GridQuery};
/// `Grid::of(find)` shows rows.
fn list(q: GridQuery) -> Grid<P> { Grid::of(find).mode(GridMode::Server).title("Grid") }
struct MyGrid; impl GridSource for MyGrid {}
const RAW: &str = r#"Grid "quoted" Grid"#; const C: char = 'G';"##;
        let (out, n) = super::rename_idents(src, super::RENAMES);
        assert_eq!(n, 7, "{out}");
        assert!(out.contains("use zeyn::{DataGrid, DataQuery};"));
        assert!(out.contains("/// `DataGrid::of(find)` shows rows."));
        assert!(out.contains(
            "fn list(q: DataQuery) -> DataGrid<P> { DataGrid::of(find).mode(DataMode::Server).title(\"Grid\") }"
        ));
        assert!(out.contains("struct MyGrid; impl GridSource for MyGrid {}"));
        assert!(out.contains(r##"r#"Grid "quoted" Grid"#"##));
        assert_eq!(
            super::rename_idents(&out, super::RENAMES).1,
            0,
            "a second run changes nothing"
        );
    }

    use super::*;

    #[test]
    fn new_projects_get_ci_for_every_platform_and_their_own_git_repository() {
        let base = std::env::temp_dir().join(format!("zeyn-new-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let root = base.join("ci-app");
        let created = new_project_at(&root, "ci-app", None, false, true);
        assert!(created.is_ok(), "{created:?}");
        for file in [
            ".github/workflows/ci.yml",
            ".github/dependabot.yml",
            ".github/pull_request_template.md",
            ".gitlab-ci.yml",
            "Jenkinsfile",
            "sonar-project.properties",
            "SECURITY.md",
            "CHANGELOG.md",
        ] {
            assert!(root.join(file).is_file(), "{file}");
        }
        let sonar = std::fs::read_to_string(root.join("sonar-project.properties")).unwrap();
        assert!(
            sonar.contains("sonar.projectKey=ci_app") && sonar.contains("sonar.projectName=Ci App"),
            "{sonar}"
        );
        let has_git = Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success());
        let inside = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&base)
            .output()
            .is_ok_and(|o| o.status.success());
        if has_git && !inside {
            let head = std::fs::read_to_string(root.join(".git/HEAD")).unwrap();
            assert_eq!(head.trim(), "ref: refs/heads/main");
        }
        let lib = base.join("ci-lib");
        new_project_at(&lib, "ci-lib", None, true, false).unwrap();
        assert!(lib.join("Jenkinsfile").is_file() && !lib.join(".git").exists());
        std::fs::remove_dir_all(&base).ok();
    }

    /// A project from the app template in a fresh temp folder.
    fn app_at(tag: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("zeyn-gen-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let root = base.join("app");
        new_project_at(&root, "app", None, false, false).unwrap();
        (base, root)
    }

    fn read(root: &Path, file: &str) -> String {
        std::fs::read_to_string(root.join(file)).unwrap()
    }

    #[test]
    fn generator_aliases_name_the_full_generators() {
        for (short, full) in [
            ("f", "feature"),
            ("s", "screen"),
            ("a", "action"),
            ("c", "component"),
        ] {
            assert_eq!(generate::kind_of(short), Some(full));
            assert_eq!(generate::kind_of(full), Some(full));
        }
        assert_eq!(generate::kind_of("x"), None);
        assert_eq!(
            generate::Kind::named("component"),
            Some(generate::Kind::Component)
        );
        assert_eq!(generate::Kind::named("feature"), None);
    }

    #[test]
    fn generators_add_one_file_each_and_move_a_flat_screens_file() {
        use generate::{Kind, generate};
        let (base, root) = app_at("files");
        let flat = read(&root, "src/features/notes/screens.rs");
        let out = generate(&root, Kind::Screen, "notes", "detail").unwrap();
        assert!(out.contains("Moved src/features/notes/screens.rs"), "{out}");
        assert!(
            out.contains("translate `notes.detail.title` in locales/tr.json"),
            "{out}"
        );
        assert!(
            !root
                .join("src/features/notes/screens.rs")
                .exists()
        );
        let moved = read(&root, "src/features/notes/screens/mod.rs");
        assert!(
            moved.starts_with("mod detail;\npub use detail::notes_detail;\n\n"),
            "{moved}"
        );
        assert!(moved.ends_with(&flat));
        let screen = read(&root, "src/features/notes/screens/detail.rs");
        assert!(
            screen.contains("#[screen(route = \"/detail\", permission = \"notes.detail.view\")]")
        );
        assert!(screen.contains("pub fn notes_detail() -> Screen"));
        let feature = read(&root, "src/features/notes/mod.rs");
        assert!(feature.contains(
            "        .screen(screens::note_editor)\n        .screen(screens::notes_detail)\n"
        ));
        for locale in ["en", "tr"] {
            let json: Value =
                serde_json::from_str(&read(&root, &format!("locales/{locale}.json"))).unwrap();
            assert_eq!(json["notes"]["detail"]["title"], "Detail");
            assert!(json["notes"]["saved"].is_string());
        }
        assert!(read(&root, "locales/en.json").contains(
            "    \"saved\": \"Note saved.\",\n    \"detail\": { \"title\": \"Detail\" }\n  },"
        ));

        generate(&root, Kind::Action, "notes", "archive").unwrap();
        assert!(
            read(&root, "src/features/notes/actions/mod.rs").starts_with(
                "mod archive;\npub use archive::notes_archive;\n\nuse super::models::Note;"
            )
        );
        assert!(read(&root, "src/features/notes/actions/archive.rs").contains("#[action(permission = \"notes.archive.run\")]\npub async fn notes_archive(_: ()) -> Result<()>"));
        assert!(
            read(&root, "src/features/notes/mod.rs")
                .contains(".api(actions::save_note)\n        .api(actions::notes_archive)\n")
        );

        generate(&root, Kind::Component, "notes", "summary").unwrap();
        let feature = read(&root, "src/features/notes/mod.rs");
        assert!(
            feature.contains("pub mod actions;\nmod components;\npub mod models;"),
            "{feature}"
        );
        assert!(
            read(&root, "src/features/notes/components/mod.rs")
                .starts_with("mod summary;\n#[expect(unused_imports")
        );
        assert!(
            read(&root, "src/features/notes/components/summary.rs")
                .contains("pub fn summary() -> Node")
        );
        generate(&root, Kind::Component, "notes", "header").unwrap();
        assert!(read(&root, "src/features/notes/components/mod.rs").starts_with("mod header;\n#[expect(unused_imports, reason = \"remove once a screen uses it\")]\npub use header::header;\nmod summary;"));

        generate(&root, Kind::Screen, "home", "about").unwrap();
        let home = read(&root, "src/features/home/mod.rs");
        assert!(home.starts_with("//! Home: the first screen after sign in.\n\npub mod screens;\n\nuse zeyn::prelude::*;"), "{home}");
        assert!(home.contains("        .screen(home)\n        .screen(screens::home_about)\n"));
        assert_eq!(
            read(&root, "src/features/home/screens/mod.rs"),
            "mod about;\npub use about::home_about;\n"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn generators_refuse_bad_names_missing_features_and_existing_files() {
        use generate::{Kind, generate};
        let (base, root) = app_at("refuse");
        let before = read(&root, "locales/en.json");
        for (feature, name, says) in [
            ("notes", "Detail", "not a valid screen name"),
            ("notes", "type", "not a valid screen name"),
            ("bad-name", "x", "not a valid feature name"),
            ("orders", "list", "feature `orders` does not exist"),
            (
                "notes",
                "editor",
                "`notes.editor` in locales/en.json is a text",
            ),
        ] {
            let err = generate(&root, Kind::Screen, feature, name).unwrap_err();
            assert!(err.contains(says), "{err}");
        }
        assert_eq!(read(&root, "locales/en.json"), before);
        assert!(
            root.join("src/features/notes/screens.rs")
                .is_file(),
            "a refusal moves nothing"
        );
        new_feature(&root, "orders").unwrap();
        assert!(
            root.join("src/features/orders/screens/list.rs")
                .is_file()
        );
        assert!(
            root.join("src/features/orders/actions/search.rs")
                .is_file()
        );
        let err = generate(&root, Kind::Screen, "orders", "list").unwrap_err();
        assert!(
            err.contains("src/features/orders/screens/list.rs already exists"),
            "{err}"
        );
        std::fs::write(
            root.join("src/features/notes/actions.rs"),
            "pub fn notes_save() {}\n",
        )
        .unwrap();
        let err = generate(&root, Kind::Action, "notes", "save").unwrap_err();
        assert!(err.contains("`notes_save` is already defined"), "{err}");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn check_warns_about_unregistered_items_and_crowded_flat_files() {
        let mut crowded = String::from(
            "#[screen(permission = \"a.b.c\")]\npub fn one() -> Screen {}\n/// Doc.\n#[screen(permission = \"a.b.c\")]\npub fn two() -> Screen {}\n",
        );
        crowded.push_str(&"let x = 1;\n".repeat(150));
        let files = vec![
            (PathBuf::from("src/features/sales/screens.rs"), crowded),
            (PathBuf::from("src/features/sales/actions.rs"), "#[action(public)]\npub async fn lost(_: ()) -> Result<()> {}\n#[action(public)]\npub async fn used(_: ()) -> Result<()> {}\n".into()),
            (PathBuf::from("src/features/sales/other.rs"), "fn f() { used; }\n".into()),
        ];
        let w = generate::layout_warnings(&files, &["one".into()]);
        assert_eq!(w.len(), 3, "{w:#?}");
        assert!(
            w[0].contains("#[screen] `two` in src/features/sales/screens.rs is not registered")
        );
        assert!(w[1].contains("src/features/sales/screens.rs has 2 #[screen] functions in 155 lines; run `zeyn g screen sales <name>`"));
        assert!(
            w[2].contains("#[action] `lost` in src/features/sales/actions.rs is not registered")
        );
        assert!(w.iter().all(|w| !w.contains("error")));
        let small = generate::layout_warnings(&files[1..], &["lost".into()]);
        assert!(small.is_empty(), "{small:#?}");
    }

    #[test]
    fn names_and_titles() {
        assert!(valid_ident("sales_orders"));
        assert!(
            !valid_ident("Sales") && !valid_ident("1x") && !valid_ident("a-b") && !valid_ident("")
        );
        assert_eq!(title_of("acme-portal"), "Acme Portal");
    }

    #[test]
    fn every_doc_topic_is_listed_in_the_tool_schema() {
        let topics = tools()[0]["inputSchema"]["properties"]["topic"]["enum"].clone();
        for (t, _) in DOCS {
            assert!(
                topics
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == t),
                "{t}"
            );
        }
    }
}
