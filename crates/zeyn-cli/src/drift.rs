//! Drift tests: the texts agents read in a new project (the AGENTS template, the Zeyn skill, the
//! docs' `llms.txt`) name only what exists. Each test reads the repository's files and is skipped
//! where they are missing, as in a published crate.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The AGENTS template of an app, the one `zeyn_docs` serves as `rules`.
const AGENTS: &str = include_str!("../templates/AGENTS.md.tmpl");

/// The AGENTS template of a library.
const LIB_AGENTS: &str = include_str!("../templates/lib/AGENTS.md.tmpl");

/// The skill copy every project gets.
const SKILL: &str = include_str!("../embedded/SKILL.md");

/// Layout and generator names from before the feature folders; none of them exists any more.
const STALE: &[&str] = &[
    "store.rs",
    "g action",
    "g component",
    "tests/structure.rs",
];

fn repo(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn read(path: &str) -> Option<String> {
    std::fs::read_to_string(repo(path)).ok()
}

/// The agent facing texts with their names, the repository ones only where present.
fn texts() -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("crates/zeyn-cli/templates/AGENTS.md.tmpl", AGENTS.to_string()),
        (
            "crates/zeyn-cli/templates/lib/AGENTS.md.tmpl",
            LIB_AGENTS.to_string(),
        ),
        ("crates/zeyn-cli/embedded/SKILL.md", SKILL.to_string()),
    ];
    for path in [
        "examples/docs/llms.txt",
        "examples/docs/pages/mcp.md",
        "examples/docs/pages/cli.md",
        "deploy/landing/site/llms.txt",
    ] {
        if let Some(text) = read(path) {
            out.push((path, text));
        }
    }
    out
}

/// Words in backticks, such as `rules` in "topic `rules`".
fn backticked(text: &str) -> impl Iterator<Item = &str> {
    text.split('`')
        .skip(1)
        .step_by(2)
}

/// The topics a text names for `zeyn_docs`: lower case words in backticks after each mention,
/// up to the end of that clause.
fn named_topics(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("`zeyn_docs`") {
        let rest = &text[at + "`zeyn_docs`".len()..];
        let end = rest
            .find([';', ')', '.'])
            .unwrap_or(rest.len());
        out.extend(
            backticked(&rest[..end])
                .filter(|w| {
                    !w.is_empty()
                        && w.chars()
                            .all(|c| c.is_ascii_lowercase())
                })
                .map(str::to_string),
        );
    }
    out
}

#[test]
fn texts_name_only_the_topics_the_mcp_server_has() {
    let topics: HashSet<&str> = crate::DOCS
        .iter()
        .map(|(t, _)| *t)
        .collect();
    let tools = crate::tools();
    let docs = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "zeyn_docs")
        .unwrap();
    let schema: HashSet<&str> = docs["inputSchema"]["properties"]["topic"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(schema, topics, "the zeyn_docs schema lists the topics of DOCS");
    let description = docs["description"]
        .as_str()
        .unwrap();
    for word in ["architecture", "components", "examples"] {
        assert!(
            !description.contains(word) || topics.contains(word),
            "the zeyn_docs description names `{word}`, which is not a topic"
        );
    }
    for (path, text) in texts() {
        for topic in named_topics(&text) {
            assert!(
                topics.contains(topic.as_str()),
                "{path} names the zeyn_docs topic `{topic}`; the MCP server has only {topics:?}"
            );
        }
    }
}

#[test]
fn texts_name_only_the_tools_the_mcp_server_has() {
    let tools = crate::tools();
    let names: HashSet<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for (path, text) in texts() {
        for word in backticked(&text).filter(|w| {
            w.starts_with("zeyn_")
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_')
        }) {
            assert!(
                names.contains(word),
                "{path} names the MCP tool `{word}`; the server has {names:?}"
            );
        }
    }
    if let Some(mcp) = read("examples/docs/pages/mcp.md") {
        for name in &names {
            assert!(
                mcp.contains(&format!("| `{name}` |")),
                "examples/docs/pages/mcp.md has no row for the MCP tool `{name}`"
            );
        }
    }
}

#[test]
fn texts_do_not_name_the_old_layout_or_removed_generators() {
    for (path, text) in texts() {
        for stale in STALE {
            let found = text
                .match_indices(stale)
                .any(|(at, _)| {
                    text[..at]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric())
                });
            assert!(
                !found,
                "{path} mentions `{stale}`; features have models/, services/, actions/ and screens/ folders and the generators are `zeyn g feature` and `zeyn g screen`"
            );
        }
    }
}

/// The copies in `embedded/` ship with the crate; in the repository they must match the originals.
#[test]
fn embedded_copies_match_the_repository() {
    for (name, original) in [
        ("SKILL.md", "skills/zeyn/SKILL.md"),
        ("runtime-protocol.md", "docs/runtime-protocol.md"),
    ] {
        let Some(original) = read(original) else {
            continue;
        };
        let copy = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("embedded")
                .join(name),
        )
        .unwrap();
        assert!(
            copy == original,
            "crates/zeyn-cli/embedded/{name} is out of date: copy the file from the repository root"
        );
    }
}

/// What the zeyn crate declares: public items, functions and enum variants by name.
#[derive(Default)]
struct Declared {
    items: HashSet<String>,
    fns: HashSet<String>,
    variants: HashSet<String>,
}

fn ident(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    &s[..end]
}

fn scan(dir: &Path, out: &mut Declared) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan(&path, out);
            continue;
        }
        if path
            .extension()
            .is_none_or(|e| e != "rs")
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim_start();
            if line.starts_with("//") {
                continue;
            }
            for kw in [
                "pub struct ",
                "pub enum ",
                "pub trait ",
                "pub type ",
                "pub fn ",
                "pub const fn ",
                "pub async fn ",
            ] {
                if let Some(rest) = line.strip_prefix(kw) {
                    out.items
                        .insert(ident(rest).to_string());
                }
            }
            if let Some(at) = line.find("fn ") {
                out.fns
                    .insert(ident(&line[at + 3..]).to_string());
            }
            let name = ident(line);
            if name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
                && line[name.len()..]
                    .chars()
                    .next()
                    .is_none_or(|c| matches!(c, ',' | '(' | ' ' | '{'))
            {
                out.variants
                    .insert(name.to_string());
            }
        }
    }
}

/// The names the component index of the AGENTS template uses: types (`DataGrid`), associated
/// functions and variants after `::`, and methods after a dot.
fn index_names() -> (Vec<String>, Vec<String>, Vec<String>) {
    let start = AGENTS
        .find("### Components by area")
        .expect("the AGENTS template has a component index");
    let rest = &AGENTS[start + 4..];
    let section = &rest[..rest
        .find("\n### ")
        .unwrap_or(rest.len())];
    let (mut types, mut paths, mut methods) = (Vec::new(), Vec::new(), Vec::new());
    for code in backticked(section) {
        let mut i = 0;
        while i < code.len() {
            let c = code[i..]
                .chars()
                .next()
                .unwrap();
            if !(c.is_alphabetic() || c == '_') {
                i += c.len_utf8();
                continue;
            }
            let name = ident(&code[i..]);
            let before = &code[..i];
            let after = &code[i + name.len()..];
            let short = name.len() < 2
                || name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_');
            if !short {
                if before.ends_with("::") {
                    paths.push(name.to_string());
                } else if before.ends_with('.') && after.starts_with('(') {
                    methods.push(name.to_string());
                } else if c.is_ascii_uppercase() {
                    types.push(name.to_string());
                }
            }
            i += name.len();
        }
    }
    (types, paths, methods)
}

#[test]
fn the_component_index_names_only_what_the_zeyn_crate_has() {
    let src = repo("crates/zeyn/src");
    if !src.is_dir() {
        return;
    }
    let mut declared = Declared::default();
    scan(&src, &mut declared);
    let (types, paths, methods) = index_names();
    assert!(
        types.len() > 40,
        "the component index lists the main types: {types:?}"
    );
    let missing: Vec<&String> = types
        .iter()
        .filter(|t| !declared.items.contains(*t) && !declared.variants.contains(*t))
        .chain(
            paths
                .iter()
                .filter(|p| !declared.fns.contains(*p) && !declared.variants.contains(*p)),
        )
        .chain(
            methods
                .iter()
                .filter(|m| !declared.fns.contains(*m)),
        )
        .collect();
    assert!(
        missing.is_empty(),
        "the component index in crates/zeyn-cli/templates/AGENTS.md.tmpl names {missing:?}, which crates/zeyn/src does not declare; fix the name or the index"
    );
}

#[test]
fn the_agents_template_stays_within_its_size_budget() {
    assert!(
        AGENTS.len() <= 18 * 1024,
        "crates/zeyn-cli/templates/AGENTS.md.tmpl is {} bytes; keep it at most 18 KB, it is read by every agent in every project",
        AGENTS.len()
    );
}
