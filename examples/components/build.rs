//! Builds the API tables of the components site from the framework's source, so they never drift
//! from the code.
//!
//! Reads every `.rs` file under `crates/zeyn/src`. For every inherent `impl` block
//! (`impl Name`, `impl<E: Model> Name<E>`, not `impl Trait for Name`) it keeps the `pub fn` items (also `const` and `async`)
//! with their signature and the first line of their `///` doc comment. The files are formatted by
//! rustfmt, so an `impl` starts at the start of a line, ends at a line that is only `}`, and its
//! methods are indented by four spaces. Writes `$OUT_DIR/api.rs` with `API`, a list of
//! `(type, &[(signature, doc)])` sorted by type name.

use std::collections::BTreeMap;
use std::path::Path;

/// The type an `impl` line is for, or `None` for a trait impl or a line that is no `impl`.
fn impl_type(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl")?;
    if !(rest.starts_with(' ') || rest.starts_with('<')) || line.contains(" for ") {
        return None;
    }
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in rest.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ' ' if depth == 0 => {
                start = i + 1;
                break;
            }
            _ => {}
        }
    }
    let name: String = rest[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The signature of a method starting at `lines[at]`, up to its body, on one line.
fn signature(lines: &[&str], at: usize) -> String {
    let mut sig = String::new();
    for line in &lines[at..] {
        let line = line.trim();
        let end = line
            .find(" {")
            .or_else(|| (line == "{").then_some(0));
        let piece = match end {
            Some(i) => &line[..i],
            None => line,
        };
        if !sig.is_empty() && !sig.ends_with('(') && !piece.starts_with(')') {
            sig.push(' ');
        }
        sig.push_str(piece);
        if end.is_some() || line.ends_with(';') {
            break;
        }
    }
    sig.replace(", )", ")")
        .replace("( ", "(")
        .trim_end_matches(',')
        .to_string()
}

/// The first line of the doc comment right above `lines[at]`, skipping attributes.
fn first_doc(lines: &[&str], at: usize) -> String {
    let mut i = at;
    let mut doc = Vec::new();
    while i > 0 {
        i -= 1;
        let line = lines[i].trim();
        if let Some(text) = line.strip_prefix("///") {
            doc.push(text.trim().to_string());
        } else if !line.starts_with("#[") {
            break;
        }
    }
    doc.reverse();
    let text = doc.join(" ");
    let first = text
        .split(". ")
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .trim()
        .to_string();
    if first.is_empty() {
        first
    } else {
        format!("{first}.")
    }
}

fn scan(path: &Path, api: &mut BTreeMap<String, Vec<(String, String)>>) {
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = source.lines().collect();
    let mut current: Option<String> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(name) = impl_type(line) {
            current = Some(name);
        } else if *line == "}" {
            current = None;
        } else if let (Some(ty), true) = (
            &current,
            ["    pub fn ", "    pub const fn ", "    pub async fn "]
                .iter()
                .any(|p| line.starts_with(p)),
        ) {
            let entry = api.entry(ty.clone()).or_default();
            entry.push((signature(&lines, i), first_doc(&lines, i)));
        }
    }
}

/// Scans every `.rs` file under `dir`, in name order.
fn walk(dir: &Path, api: &mut BTreeMap<String, Vec<(String, String)>>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk(&path, api);
        } else if path
            .extension()
            .is_some_and(|e| e == "rs")
        {
            println!("cargo:rerun-if-changed={}", path.display());
            scan(&path, api);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/zeyn/src");
    let mut api = BTreeMap::new();
    walk(&root, &mut api);
    let mut out =
        String::from("/// Public methods of each framework type: signature and first doc line.\n");
    out.push_str("pub static API: &[(&str, &[(&str, &str)])] = &[\n");
    for (ty, methods) in &api {
        out.push_str(&format!("    ({ty:?}, &["));
        for (sig, doc) in methods {
            out.push_str(&format!("({sig:?}, {doc:?}), "));
        }
        out.push_str("]),\n");
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("api.rs");
    std::fs::write(dest, out).expect("write api.rs");
}
