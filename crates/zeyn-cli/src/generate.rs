//! `zeyn g screen|action|component <feature> <name>`: one file per screen, action and component.
//!
//! The folder layout (`screens/mod.rs` plus `screens/<name>.rs`) is the default these generators
//! produce, not a rule: a feature that keeps a flat `screens.rs` still builds, and the generators
//! turn it into a folder the first time they add a file to it.

use super::{title_of, valid_ident};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// What a generator adds to a feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Screen,
    Action,
    Component,
}

/// The generator a word names: `feature` or `f`, `screen` or `s`, `action` or `a`, `component`
/// or `c`. Returns the full name.
pub fn kind_of(word: &str) -> Option<&'static str> {
    match word {
        "feature" | "f" => Some("feature"),
        "screen" | "s" => Some("screen"),
        "action" | "a" => Some("action"),
        "component" | "c" => Some("component"),
        _ => None,
    }
}

impl Kind {
    /// The kind for a full generator name from [`kind_of`].
    pub fn named(name: &str) -> Option<Kind> {
        match name {
            "screen" => Some(Kind::Screen),
            "action" => Some(Kind::Action),
            "component" => Some(Kind::Component),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Kind::Screen => "screen",
            Kind::Action => "action",
            Kind::Component => "component",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Kind::Screen => "screens",
            Kind::Action => "actions",
            Kind::Component => "components",
        }
    }
}

/// Rust keywords, which cannot name a module.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
    "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Adds `src/features/<feature>/<folder>/<name>.rs` and wires it up. Never overwrites a file.
/// Returns what was done and what is left to do by hand.
pub fn generate(root: &Path, kind: Kind, feature: &str, name: &str) -> Result<String, String> {
    for (what, value) in [("feature", feature), (kind.word(), name)] {
        if !valid_ident(value) || KEYWORDS.contains(&value) {
            return Err(format!(
                "`{value}` is not a valid {what} name. Use snake case: lower case letters, digits \
                 and `_`, starting with a letter, and not a Rust keyword, for example `order_lines`"
            ));
        }
    }
    let dir = root.join("src/features").join(feature);
    let feature_mod = dir.join("mod.rs");
    if !feature_mod.is_file() {
        return Err(format!(
            "feature `{feature}` does not exist: {} is missing. Create it with `zeyn g feature \
             {feature}`, or list the features with `zeyn routes`",
            shown(root, &feature_mod)
        ));
    }
    let folder = dir.join(kind.folder());
    let flat = dir.join(format!("{}.rs", kind.folder()));
    let file = folder.join(format!("{name}.rs"));
    if file.exists() {
        return Err(format!(
            "{} already exists, so nothing was changed. Pick another name or edit that file",
            shown(root, &file)
        ));
    }
    if flat.is_file() && folder.exists() {
        return Err(format!(
            "both {} and {} exist; Rust accepts only one. Move the content of the .rs file into \
             mod.rs in the folder, delete the .rs file, then run the generator again",
            shown(root, &flat),
            shown(root, &folder)
        ));
    }
    let ident = match kind {
        Kind::Component => name.to_string(),
        _ => format!("{feature}_{name}"),
    };
    if let Some(owner) = defines(&[flat.clone(), folder.clone()], &ident) {
        return Err(format!(
            "`{ident}` is already defined in {}, so nothing was changed. Pick another name",
            shown(root, &owner)
        ));
    }
    let key = format!("{feature}.{name}.title");
    let texts = match kind {
        Kind::Action => Vec::new(),
        _ => locale_updates(root, &key, &title_of(name))?,
    };
    let mut done = Vec::new();
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    if flat.is_file() {
        std::fs::rename(&flat, folder.join("mod.rs")).map_err(|e| e.to_string())?;
        done.push(format!(
            "Moved {} to {}/mod.rs, keeping its content",
            shown(root, &flat),
            shown(root, &folder)
        ));
    }
    write(&file, &body(kind, feature, name, &ident))?;
    done.push(format!("Created {}", shown(root, &file)));
    let sub_mod = folder.join("mod.rs");
    let block = match kind {
        Kind::Component => format!(
            "mod {name};\n#[expect(unused_imports, reason = \"remove once a screen uses it\")]\npub use {name}::{name};\n"
        ),
        _ => format!("mod {name};\npub use {name}::{ident};\n"),
    };
    let sub_text = std::fs::read_to_string(&sub_mod).unwrap_or_default();
    write(&sub_mod, &insert_mod(&sub_text, name, &block))?;
    let mut text = std::fs::read_to_string(&feature_mod).map_err(|e| e.to_string())?;
    if !declares(&text, kind.folder()) {
        let decl = match kind {
            Kind::Component => "mod components;\n".to_string(),
            _ => format!("pub mod {};\n", kind.folder()),
        };
        text = insert_mod(&text, kind.folder(), &decl);
    }
    let call = match kind {
        Kind::Screen => Some((format!(".screen(screens::{ident})"), &[".screen("][..])),
        Kind::Action => Some((
            format!(".api(actions::{ident})"),
            &[".api(", ".screen("][..],
        )),
        Kind::Component => None,
    };
    let mut todo = Vec::new();
    if let Some((call, anchors)) = call {
        match register(&text, &call, anchors) {
            Some(t) => {
                text = t;
                done.push(format!(
                    "Registered `{call}` in feature() in {}",
                    shown(root, &feature_mod)
                ));
            }
            None => todo.push(format!(
                "add `{call}` to feature() in {}",
                shown(root, &feature_mod)
            )),
        }
    }
    write(&feature_mod, &text)?;
    if kind == Kind::Action {
        todo.push(format!(
            "`{ident}` is also an API at /api/v1/{feature}/{ident}; remove `.api(actions::{ident})` \
             if only screens run it, and use it from a screen"
        ));
    }
    if kind == Kind::Component {
        todo.push(format!(
            "use `{name}()` in a screen of the feature (`use super::components::{name};`), then \
             remove the two `#[expect(...)]` attributes"
        ));
    }
    for (path, new, english) in texts {
        write(&path, &new)?;
        done.push(format!("Added `{key}` to {}", shown(root, &path)));
        if !english {
            todo.push(format!(
                "translate `{key}` in {}: it has the English text for now",
                shown(root, &path)
            ));
        }
    }
    let mut out = done.join("\n");
    if !todo.is_empty() {
        out.push_str("\nNext:\n");
        for t in todo {
            out.push_str(&format!("  - {t}\n"));
        }
        out.push_str("  - cargo test");
    } else {
        out.push_str("\nNext: cargo test");
    }
    Ok(out)
}

fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The source of a new screen, action or component.
fn body(kind: Kind, feature: &str, name: &str, ident: &str) -> String {
    let title = title_of(name);
    match kind {
        Kind::Screen => format!(
            "use zeyn::prelude::*;\n\n/// {title} screen.\n#[screen(route = \"/{route}\", permission = \"{feature}.{name}.view\")]\npub fn {ident}() -> Screen {{\n    Screen::new(t!(\"{feature}.{name}.title\"))\n}}\n",
            route = name.replace('_', "-")
        ),
        Kind::Action => format!(
            "use zeyn::prelude::*;\n\n/// {title}. Say what the action does: this paragraph also describes it to AI models.\n#[action(permission = \"{feature}.{name}.run\")]\npub async fn {ident}(_: ()) -> Result<()> {{\n    Ok(())\n}}\n"
        ),
        Kind::Component => format!(
            "use zeyn::prelude::*;\n\n/// {title}: a part of a screen, such as a panel or a toolbar.\n#[expect(dead_code, reason = \"remove once a screen uses it\")]\npub fn {name}() -> Node {{\n    Panel::new(t!(\"{feature}.{name}.title\")).into()\n}}\n"
        ),
    }
}

/// The first file under `paths` (files or folders) that defines `fn <ident>(`.
fn defines(paths: &[PathBuf], ident: &str) -> Option<PathBuf> {
    let needle = format!("fn {ident}(");
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            super::rust_files(p, &mut files);
        } else if p.is_file() {
            files.push(p.clone());
        }
    }
    files
        .into_iter()
        .find(|f| std::fs::read_to_string(f).is_ok_and(|t| t.contains(&needle)))
}

/// Whether `text` declares `mod <name>;`.
fn declares(text: &str, name: &str) -> bool {
    text.lines().map(str::trim).any(|l| {
        l.strip_prefix("pub ")
            .or_else(|| l.strip_prefix("pub(crate) "))
            .unwrap_or(l)
            == format!("mod {name};")
    })
}

/// The module a `mod x;` or `pub mod x;` line declares.
fn mod_name(line: &str) -> Option<&str> {
    let l = line.trim();
    l.strip_prefix("pub ")
        .unwrap_or(l)
        .strip_prefix("mod ")?
        .strip_suffix(';')
}

/// Inserts `block`, which starts with `mod <name>;`, among the module lines at the top of `text`:
/// before the first module that sorts after `name`, else after the last one, else after the
/// leading `//!` comment. Keeps a blank line between the block and the code after it.
fn insert_mod(text: &str, name: &str, block: &str) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut lead = 0;
    let mut last_mod = None;
    let mut before = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        let modish = t.starts_with("mod ")
            || t.starts_with("pub mod ")
            || t.starts_with("pub use ")
            || t.starts_with("#[expect(");
        if !(t.is_empty() || t.starts_with("//!") || modish) {
            break;
        }
        lead = i + 1;
        if modish {
            last_mod = Some(i + 1);
        }
        if before.is_none() && mod_name(t).is_some_and(|m| m > name) {
            before = Some(i);
        }
    }
    let at = before.or(last_mod).unwrap_or(lead);
    let mut out: String = lines[..at].concat();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if at > 0 && last_mod.is_none() && !lines[at - 1].trim().is_empty() {
        out.push('\n');
    }
    out.push_str(block);
    if lines
        .get(at)
        .is_some_and(|l| !l.trim().is_empty() && mod_name(l).is_none())
    {
        out.push('\n');
    }
    out.push_str(&lines[at..].concat());
    out
}

/// Adds `call` as a line of the `Feature` builder: after the last line that starts with one of
/// `anchors`, else after `Feature::new(...)`. `None` when no such complete line is found.
fn register(text: &str, call: &str, anchors: &[&str]) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let complete =
        |l: &str| l.matches('(').count() == l.matches(')').count() && !l.trim_end().ends_with(';');
    let found = anchors
        .iter()
        .find_map(|a| {
            lines
                .iter()
                .rposition(|l| l.trim_start().starts_with(a) && complete(l))
                .map(|i| (i, 0))
        })
        .or_else(|| {
            lines
                .iter()
                .rposition(|l| l.contains("Feature::new(") && complete(l))
                .map(|i| (i, 4))
        });
    let (i, extra) = found?;
    let indent = lines[i].len() - lines[i].trim_start().len() + extra;
    let mut out: String = lines[..=i].concat();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{}{call}\n", " ".repeat(indent)));
    out.push_str(&lines[i + 1..].concat());
    Some(out)
}

/// The new text of every `locales/*.json` that lacks `key`, with `english` as its text, and
/// whether the file is `en.json`. Fails before anything is written when a file is invalid or a
/// part of the key is already a text.
fn locale_updates(
    root: &Path,
    key: &str,
    english: &str,
) -> Result<Vec<(PathBuf, String, bool)>, String> {
    let dir = root.join("locales");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|_| "there is no locales/ folder; create locales/en.json first".to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x == "json")
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("locales/ has no .json files; create locales/en.json first".into());
    }
    let parts: Vec<&str> = key.split('.').collect();
    let mut out = Vec::new();
    for path in files {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = shown(root, &path);
        let json: Value = serde_json::from_str(&text).map_err(|e| {
            format!("{file} is not valid JSON ({e}); fix it, then run the generator again")
        })?;
        let mut node = &json;
        let mut depth = 0;
        while let Some(next) = parts
            .get(depth)
            .and_then(|p| node.get(p))
        {
            node = next;
            depth += 1;
        }
        if depth == parts.len() {
            continue;
        }
        if !node.is_object() {
            return Err(format!(
                "`{}` in {file} is a text, so `{key}` cannot be added. Pick another name",
                parts[..depth].join(".")
            ));
        }
        let mut value = serde_json::to_string(english).unwrap_or_default();
        for p in parts[depth + 1..].iter().rev() {
            value = format!("{{ \"{p}\": {value} }}");
        }
        let member = format!("\"{}\": {value}", parts[depth]);
        let (open, close) = object_at(&text, &parts[..depth]);
        let new = add_member(&text, open, close, &member);
        let ok = serde_json::from_str::<Value>(&new).is_ok_and(|v| {
            parts
                .iter()
                .try_fold(&v, |v, p| v.get(p))
                .is_some_and(Value::is_string)
        });
        if !ok {
            return Err(format!("could not add `{key}` to {file}; add it by hand"));
        }
        let english_file = path
            .file_stem()
            .is_some_and(|s| s == "en");
        out.push((path, new, english_file));
    }
    Ok(out)
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while b
        .get(i)
        .is_some_and(u8::is_ascii_whitespace)
    {
        i += 1;
    }
    i
}

/// Index just past the JSON value at `i`. The text is known to be valid JSON.
fn value_end(b: &[u8], i: usize) -> usize {
    let mut j = i;
    let mut depth = 0usize;
    let mut in_str = false;
    while j < b.len() {
        let c = b[j];
        if in_str {
            match c {
                b'\\' => j += 1,
                b'"' => {
                    in_str = false;
                    if depth == 0 {
                        return j + 1;
                    }
                }
                _ => {}
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' | b'[' => depth += 1,
                b'}' | b']' if depth == 0 => return j,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return j + 1;
                    }
                }
                b',' if depth == 0 => return j,
                c if depth == 0 && c.is_ascii_whitespace() => return j,
                _ => {}
            }
        }
        j += 1;
    }
    j
}

/// The braces of the object at `path` (each part an existing object member).
fn object_at(text: &str, path: &[&str]) -> (usize, usize) {
    let b = text.as_bytes();
    let mut open = skip_ws(b, 0);
    let mut parts = path.iter();
    loop {
        let want = parts.next();
        let mut i = open + 1;
        let mut found = None;
        loop {
            i = skip_ws(b, i);
            if b.get(i).is_none_or(|c| *c == b'}') {
                break;
            }
            let key_end = value_end(b, i);
            let key = &text[i + 1..key_end - 1];
            let start = skip_ws(b, skip_ws(b, key_end) + 1);
            if want.is_some_and(|w| *w == key) {
                found = Some(start);
                break;
            }
            i = skip_ws(b, value_end(b, start));
            if b.get(i) == Some(&b',') {
                i += 1;
            }
        }
        match found {
            Some(start) => open = start,
            None => return (open, i),
        }
    }
}

/// Adds `member` as the last member of the object between `open` and `close`, on its own line
/// when the object spans several lines.
fn add_member(text: &str, open: usize, close: usize, member: &str) -> String {
    let inner = &text[open + 1..close];
    if inner.trim().is_empty() {
        return format!("{} {member} {}", &text[..=open], &text[close..]);
    }
    let last = open + 1 + inner.trim_end().len();
    let sep = if inner.contains('\n') {
        let first = skip_ws(text.as_bytes(), open + 1);
        let line_start = text[..first]
            .rfind('\n')
            .map_or(0, |n| n + 1);
        format!(",\n{}", &text[line_start..first])
    } else {
        ", ".to_string()
    };
    format!("{}{sep}{member}{}", &text[..last], &text[last..])
}

/// Warnings about the default layout for `zeyn check`, from the source files of an app and the
/// screens and APIs its features register. Never errors: the layout is a default, not a rule.
pub fn layout_warnings(files: &[(PathBuf, String)], registered: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let words = |t: &str| -> Vec<String> {
        t.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let all: Vec<String> = files
        .iter()
        .flat_map(|(_, t)| words(t))
        .collect();
    for (path, text) in files {
        let shown = path
            .display()
            .to_string()
            .replace('\\', "/");
        let mut screens = 0;
        let mut actions = 0;
        for (kind, name) in attributed_fns(text) {
            if kind == "screen" {
                screens += 1;
            } else {
                actions += 1;
            }
            let used = registered.contains(&name)
                || (kind == "action"
                    && all
                        .iter()
                        .filter(|w| **w == name)
                        .count()
                        > 1);
            if !used {
                let how = if kind == "screen" {
                    format!(
                        "add `.screen(screens::{name})` to its feature() in mod.rs, and the feature to src/app.rs if it is not there"
                    )
                } else {
                    format!("use it from a screen or add `.api(actions::{name})` to its feature()")
                };
                out.push(format!(
                    "#[{kind}] `{name}` in {shown} is not registered in any feature; {how}"
                ));
            }
        }
        let lines = text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        let feature = path
            .parent()
            .and_then(Path::file_name)
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        for (file, kind, n) in [
            ("screens.rs", "screen", screens),
            ("actions.rs", "action", actions),
        ] {
            if path
                .file_name()
                .is_some_and(|f| f == file)
                && n > 1
                && lines > 150
            {
                out.push(format!(
                    "{shown} has {n} #[{kind}] functions in {lines} lines; run `zeyn g {kind} {feature} <name>` \
                     for the next one, or move each {kind} into {}/<name>.rs",
                    file.trim_end_matches(".rs")
                ));
            }
        }
    }
    out
}

/// The `#[screen]` and `#[action]` functions of a source file, as (kind, name).
fn attributed_fns(text: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let mut pending: Option<&'static str> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("#[screen") {
            pending = Some("screen");
        } else if t.starts_with("#[action") {
            pending = Some("action");
        } else if let Some(kind) = pending {
            if let Some(i) = t.find("fn ") {
                let name: String = t[i + 3..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                out.push((kind, name));
                pending = None;
            } else if !(t.starts_with("#[") || t.starts_with("///") || t.is_empty()) {
                pending = None;
            }
        }
    }
    out
}
