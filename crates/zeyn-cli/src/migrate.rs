//! `zeyn migrate`: moves an app to a newer Zeyn version with ordered, versioned steps.
//!
//! Each Zeyn version that breaks an API has a module here with one [`Step`]. A step lists
//! [`Change`]s: renamed names, method calls that take another shape, moved `use` paths and
//! patterns a person has to look at. Files are parsed with `syn`, and every change is a text
//! edit at the span of the tokens it replaces, so formatting and comments stay as they were.
//! `zeyn check --fix` applies the renames of every step through the same engine.

mod v0_0_11;
mod v0_0_6;
mod v0_0_8;
mod v0_0_9;

use proc_macro2::{Span, TokenStream, TokenTree};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};

/// The steps, oldest first. A step runs when the app's version is below its version and the
/// target is at or above it.
pub const STEPS: &[Step] = &[v0_0_6::STEP, v0_0_8::STEP, v0_0_9::STEP, v0_0_11::STEP];

/// Zeyn crates whose dependency version `zeyn migrate` moves with the app.
const CRATES: &[&str] = &["zeyn", "zeyn-macros", "zeyn-island", "zeyn-desktop"];

/// The changes that move an app to one Zeyn version.
pub struct Step {
    pub version: &'static str,
    pub changes: &'static [Change],
}

/// One change of a step.
pub enum Change {
    /// A name, old to new. A name with `::` (`Icon::Delete`) is renamed only where it is written
    /// with that path. Comments are rewritten too, so doc examples follow.
    Rename(&'static str, &'static str),
    /// A method call `.method(a, b)` with `args` arguments becomes `to`, a template from the dot
    /// on where `{0}`, `{1}` stand for the arguments: `.bar({1}).baz({0})`. An empty template
    /// removes the call.
    Call {
        method: &'static str,
        args: usize,
        to: &'static str,
    },
    /// A method whose last argument was a `bool` flag: `on` replaces a call with `true`, `off`
    /// one with `false` (templates as in [`Change::Call`], the flag left out). Any other value
    /// is a TODO for a person.
    Flag {
        method: &'static str,
        args: usize,
        on: &'static str,
        off: &'static str,
    },
    /// A moved item, full old path to full new path (`zeyn::rank::ReorderRequest` to
    /// `zeyn::ReorderRequest`), in `use` items and in paths written out in code.
    Use(&'static str, &'static str),
    /// A pattern that needs a person, with the reason shown in the report.
    Review(Pattern, &'static str),
}

/// What a [`Change::Review`] looks for.
pub enum Pattern {
    /// An identifier or a path such as `CallKind::Upload`, also inside macros.
    Name(&'static str),
    /// A string literal that holds this text, also inside macros.
    Text(&'static str),
    /// A struct literal of this type.
    Struct(&'static str),
}

/// A text edit: bytes `start..end` become `text`. `note` describes it in the report.
#[derive(Debug)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub note: String,
}

/// A place a person has to look at.
#[derive(Debug)]
pub struct Todo {
    pub at: usize,
    pub reason: String,
}

/// What a migration did, as written by `zeyn migrate --json`.
#[derive(Default)]
pub struct Report {
    pub from: String,
    pub to: String,
    pub steps: Vec<&'static str>,
    pub changes: Vec<Value>,
    pub todos: Vec<Value>,
    pub errors: Vec<Value>,
    pub files: Vec<String>,
    pub diff: String,
}

impl Report {
    /// The report as one JSON document with stable keys.
    pub fn to_json(&self, dry_run: bool) -> Value {
        let mut out = json!({
            "from": self.from,
            "to": self.to,
            "dry_run": dry_run,
            "steps": self.steps,
            "files": self.files,
            "changes": self.changes,
            "todos": self.todos,
            "errors": self.errors,
        });
        if dry_run {
            out["diff"] = json!(self.diff);
        }
        out
    }

    /// The report as text: what changed, then what needs a person.
    pub fn to_text(&self, dry_run: bool) -> String {
        let mut out = String::new();
        if dry_run {
            out.push_str(&self.diff);
        }
        out.push_str(&format!(
            "zeyn migrate {} -> {}, steps: {}\n",
            self.from,
            self.to,
            if self.steps.is_empty() {
                "none".to_string()
            } else {
                self.steps.join(", ")
            }
        ));
        for c in &self.changes {
            out.push_str(&format!(
                "  {}:{}  {}\n",
                c["file"].as_str().unwrap_or(""),
                c["line"],
                c["change"].as_str().unwrap_or("")
            ));
        }
        if !self.todos.is_empty() {
            out.push_str("needs a person:\n");
            for t in &self.todos {
                out.push_str(&format!(
                    "  {}:{}  {}\n",
                    t["file"].as_str().unwrap_or(""),
                    t["line"],
                    t["reason"].as_str().unwrap_or("")
                ));
            }
        }
        for e in &self.errors {
            out.push_str(&format!(
                "error: {}: {}\n",
                e["file"].as_str().unwrap_or(""),
                e["error"].as_str().unwrap_or("")
            ));
        }
        out.push_str(&format!(
            "{} files {}, {} changes, {} for a person{}\n",
            self.files.len(),
            if dry_run || !self.errors.is_empty() {
                "to change"
            } else {
                "changed"
            },
            self.changes.len(),
            self.todos.len(),
            if dry_run {
                " (dry run, nothing written)"
            } else if !self.errors.is_empty() {
                " (nothing written; fix the errors and run again)"
            } else {
                ""
            }
        ));
        out
    }
}

/// Every rename of every step, for `zeyn check --fix`.
pub fn renames() -> Vec<(&'static str, &'static str)> {
    STEPS
        .iter()
        .flat_map(|s| s.changes)
        .filter_map(|c| match c {
            Change::Rename(old, new) => Some((*old, *new)),
            _ => None,
        })
        .collect()
}

/// Migrates the project at `root` from `from` (read from Cargo.lock or Cargo.toml when `None`)
/// to `to` (the newest step or this CLI's version when `None`). With `dry_run` nothing is
/// written and the report holds a unified diff. When a file cannot be read or parsed, nothing
/// is written either.
pub fn run(
    root: &Path,
    from: Option<&str>,
    to: Option<&str>,
    dry_run: bool,
) -> Result<Report, String> {
    let from = match from {
        Some(v) => v.to_string(),
        None => current_version(root)?,
    };
    let to = to.map_or_else(latest, str::to_string);
    let (lo, hi) = (version(&from)?, version(&to)?);
    if lo > hi {
        return Err(format!("--from {from} is newer than --to {to}"));
    }
    let steps: Vec<&Step> = STEPS
        .iter()
        .filter(|s| version(s.version).is_ok_and(|v| lo < v && v <= hi))
        .collect();
    let mut report = Report {
        from,
        to: to.clone(),
        steps: steps
            .iter()
            .map(|s| s.version)
            .collect(),
        ..Report::default()
    };
    let mut files = Vec::new();
    project_files(root, &mut files);
    files.sort();
    let mut writes = Vec::new();
    for path in files {
        let name = relative(root, &path);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                report
                    .errors
                    .push(json!({ "file": name, "error": e.to_string() }));
                continue;
            }
        };
        let new = if name.ends_with(".rs") {
            migrate_file(&name, &text, &steps, &mut report)
        } else {
            bump_manifest(&text, hi, &to)
        };
        if new == text {
            continue;
        }
        if name.ends_with("Cargo.toml") {
            report.changes.push(json!({
                "file": name,
                "line": 0,
                "step": to,
                "change": format!("Zeyn dependencies -> {to}"),
            }));
        }
        if dry_run {
            report
                .diff
                .push_str(&unified_diff(&name, &text, &new));
        } else {
            writes.push((path, new));
        }
        report.files.push(name);
    }
    if report.errors.is_empty() {
        for (path, new) in writes {
            std::fs::write(&path, new).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(report)
}

/// Runs the steps on one Rust file and records changes, TODOs and parse errors.
/// Lines in the report are lines of the file as it was before the first step.
fn migrate_file(name: &str, original: &str, steps: &[&Step], report: &mut Report) -> String {
    let mut text = original.to_string();
    let mut history: Vec<Vec<(usize, usize, usize)>> = Vec::new();
    let line = |history: &[Vec<(usize, usize, usize)>], at: usize| {
        let at = history
            .iter()
            .rev()
            .fold(at, |at, set| before_edits(at, set));
        line_of(original, at)
    };
    for step in steps {
        for _ in 0..8 {
            let found = step_edits(&text, step);
            proc_macro2::extra::invalidate_current_thread_spans();
            let (edits, todos) = match found {
                Ok(found) => found,
                Err(e) => {
                    report
                        .errors
                        .push(json!({ "file": name, "error": e }));
                    return text;
                }
            };
            for t in todos {
                let entry = json!({
                    "file": name,
                    "line": line(&history, t.at),
                    "step": step.version,
                    "reason": t.reason,
                });
                if !report.todos.contains(&entry) {
                    report.todos.push(entry);
                }
            }
            if edits.is_empty() {
                break;
            }
            for e in &edits {
                let replaced = &text[e.start..e.end];
                let at = e.start + replaced.len() - replaced.trim_start().len();
                report.changes.push(json!({
                    "file": name,
                    "line": line(&history, at),
                    "step": step.version,
                    "change": e.note,
                }));
            }
            history.push(
                edits
                    .iter()
                    .map(|e| (e.start, e.end, e.text.len()))
                    .collect(),
            );
            text = apply(&text, edits);
        }
    }
    text
}

/// Where a position of the text after `edits` (start, end, new length) was before them; a
/// position inside an edit goes to its start.
fn before_edits(at: usize, edits: &[(usize, usize, usize)]) -> usize {
    let mut shift = 0isize;
    for &(start, end, len) in edits {
        let new_start = start.saturating_add_signed(shift);
        if at < new_start {
            break;
        }
        if at < new_start + len {
            return start;
        }
        shift += len as isize - (end - start) as isize;
    }
    at.saturating_add_signed(-shift)
}

/// The edits and TODOs of one step on one file. Files are parsed only when the step has more
/// than renames; a file that does not parse is an error.
pub fn step_edits(text: &str, step: &Step) -> Result<(Vec<Edit>, Vec<Todo>), String> {
    let renames: Vec<(&str, &str)> = step
        .changes
        .iter()
        .filter_map(|c| match c {
            Change::Rename(old, new) => Some((*old, *new)),
            _ => None,
        })
        .collect();
    let mut edits = rename_edits(text, &renames);
    let mut todos = Vec::new();
    if renames.len() < step.changes.len() {
        let skip = prefix_len(text);
        let file: syn::File = syn::parse_str(&text[skip..]).map_err(|e| {
            let at = e.span().start();
            format!("{}:{}: {e}", at.line, at.column + 1)
        })?;
        let mut finder = Finder {
            text,
            skip,
            changes: step.changes,
            edits: Vec::new(),
            todos: Vec::new(),
        };
        finder.visit_file(&file);
        edits.append(&mut finder.edits);
        todos.append(&mut finder.todos);
        let tokens: TokenStream = text[skip..]
            .parse()
            .map_err(|e: proc_macro2::LexError| e.to_string())?;
        review_tokens(tokens, skip, step.changes, &mut todos);
    }
    edits.sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut kept: Vec<Edit> = Vec::with_capacity(edits.len());
    for e in edits {
        if kept
            .last()
            .is_none_or(|k| e.start >= k.end)
        {
            kept.push(e);
        }
    }
    todos.sort_by_key(|t| t.at);
    Ok((kept, todos))
}

/// Applies sorted edits that do not overlap.
pub fn apply(text: &str, edits: Vec<Edit>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for e in edits {
        out.push_str(&text[at..e.start]);
        out.push_str(&e.text);
        at = e.end;
    }
    out.push_str(&text[at..]);
    out
}

/// Bytes before the code `syn` reads: a byte order mark and a `#!` line.
fn prefix_len(text: &str) -> usize {
    let mut skip = if text.starts_with('\u{feff}') { 3 } else { 0 };
    if text[skip..].starts_with("#!") && !text[skip..].starts_with("#![") {
        skip += text[skip..]
            .find('\n')
            .unwrap_or(text.len() - skip);
    }
    skip
}

/// Walks the syntax tree for calls, `use` items, paths and struct literals.
struct Finder<'a> {
    text: &'a str,
    skip: usize,
    changes: &'static [Change],
    edits: Vec<Edit>,
    todos: Vec<Todo>,
}

impl Finder<'_> {
    fn range(&self, span: Span) -> std::ops::Range<usize> {
        let r = span.byte_range();
        r.start + self.skip..r.end + self.skip
    }

    fn todo(&mut self, span: Span, reason: String) {
        let at = self.range(span).start;
        self.todos.push(Todo { at, reason });
    }

    /// Replaces the call from its dot to its closing parenthesis with `template`, its
    /// arguments filled in. An empty template also takes the white space before the dot.
    fn reshape(&mut self, call: &syn::ExprMethodCall, args: &[String], template: &str) {
        let close = self
            .range(call.paren_token.span.close())
            .end;
        let mut start = self
            .range(call.dot_token.spans[0])
            .start;
        let mut text = template.to_string();
        for (i, a) in args.iter().enumerate() {
            text = text.replace(&format!("{{{i}}}"), a);
        }
        if text.is_empty() {
            start = self.text[..start].trim_end().len();
        }
        let old = &self.text[self
            .range(call.dot_token.spans[0])
            .start..close];
        let note = format!("`{}` -> `{}`", one_line(old), one_line(&text));
        self.edits.push(Edit {
            start,
            end: close,
            text,
            note,
        });
    }

    /// The source text of each argument of a call. `None` when a comment sits between the
    /// arguments, which a new shape could not keep in place, or when commas of generic types
    /// make the arguments ambiguous.
    fn args(&self, call: &syn::ExprMethodCall) -> Option<Vec<String>> {
        let open = self
            .range(call.paren_token.span.open())
            .end;
        let close = self
            .range(call.paren_token.span.close())
            .start;
        let inner = &self.text[open..close];
        let tokens: TokenStream = inner.parse().ok()?;
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut next = true;
        for token in tokens {
            let r = token.span().byte_range();
            match &token {
                TokenTree::Punct(p) if p.as_char() == ',' => next = true,
                _ if next => {
                    spans.push((r.start, r.end));
                    next = false;
                }
                _ => {
                    if let Some(last) = spans.last_mut() {
                        last.1 = r.end;
                    }
                }
            }
        }
        if spans.len() != call.args.len() {
            return None;
        }
        let mut at = 0;
        let mut out = Vec::new();
        for (start, end) in spans {
            if !inner[at..start]
                .trim_matches(|c: char| c == ',' || c.is_whitespace())
                .is_empty()
            {
                return None;
            }
            out.push(inner[start..end].to_string());
            at = end;
        }
        inner[at..]
            .trim_matches(|c: char| c == ',' || c.is_whitespace())
            .is_empty()
            .then_some(out)
    }

    /// Checks one `use` tree against the moved paths. `prefix` holds the segments above it and
    /// `start` where the item's path begins, or `None` inside a `{..}` group.
    fn use_tree(&mut self, tree: &syn::UseTree, prefix: &mut Vec<String>, start: Option<usize>) {
        match tree {
            syn::UseTree::Path(p) => {
                let start = start.or(Some(self.range(p.ident.span()).start));
                prefix.push(p.ident.to_string());
                self.use_tree(&p.tree, prefix, start);
                prefix.pop();
            }
            syn::UseTree::Group(g) => {
                for t in &g.items {
                    self.use_tree(t, prefix, None);
                }
            }
            syn::UseTree::Name(syn::UseName { ident })
            | syn::UseTree::Rename(syn::UseRename { ident, .. }) => {
                let full = format!(
                    "{}{}{ident}",
                    prefix.join("::"),
                    if prefix.is_empty() { "" } else { "::" }
                );
                for change in self.changes {
                    let Change::Use(from, to) = change else {
                        continue;
                    };
                    if full != *from {
                        continue;
                    }
                    let end = self.range(ident.span()).end;
                    match start {
                        Some(start) => self.edits.push(Edit {
                            start,
                            end,
                            text: to.to_string(),
                            note: format!("`use {from}` -> `use {to}`"),
                        }),
                        None => self.todo(
                            ident.span(),
                            format!("`{from}` moved to `{to}`: change this `use` by hand"),
                        ),
                    }
                }
            }
            syn::UseTree::Glob(_) => {}
        }
    }
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        let count = call.args.len();
        for change in self.changes {
            match change {
                Change::Call { method, args, to } if *method == name && *args == count => {
                    match self.args(call) {
                        Some(args) => self.reshape(call, &args, to),
                        None => self.todo(
                            call.method.span(),
                            format!("comments or generic types between the arguments: rewrite `.{name}(..)` as `{to}` by hand"),
                        ),
                    }
                    break;
                }
                Change::Flag {
                    method,
                    args,
                    on,
                    off,
                } if *method == name && *args == count => {
                    let Some(mut args) = self.args(call) else {
                        self.todo(
                            call.method.span(),
                            format!("comments or generic types between the arguments: `.{name}` no longer takes a bool, rewrite it by hand"),
                        );
                        break;
                    };
                    let flag = args.pop().unwrap_or_default();
                    match flag.as_str() {
                        "true" => self.reshape(call, &args, on),
                        "false" => self.reshape(call, &args, off),
                        _ => {
                            let fill = |t: &str| {
                                let mut t = t.to_string();
                                for (i, a) in args.iter().enumerate() {
                                    t = t.replace(&format!("{{{i}}}"), a);
                                }
                                if t.is_empty() {
                                    "nothing".to_string()
                                } else {
                                    format!("`{t}`")
                                }
                            };
                            let reason = format!(
                                "`.{name}` no longer takes a bool: write {} for true and {} for false",
                                fill(on),
                                fill(off)
                            );
                            self.todo(call.method.span(), reason);
                        }
                    }
                    break;
                }
                _ => {}
            }
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.use_tree(&item.tree, &mut Vec::new(), None);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        for change in self.changes {
            let Change::Use(from, to) = change else {
                continue;
            };
            let parts: Vec<&str> = from.split("::").collect();
            if path.segments.len() < parts.len()
                || path
                    .segments
                    .iter()
                    .zip(&parts)
                    .any(|(s, p)| s.ident != p)
            {
                continue;
            }
            let start = self
                .range(path.segments[0].ident.span())
                .start;
            let end = self
                .range(
                    path.segments[parts.len() - 1]
                        .ident
                        .span(),
                )
                .end;
            self.edits.push(Edit {
                start,
                end,
                text: to.to_string(),
                note: format!("`{from}` -> `{to}`"),
            });
        }
        visit::visit_path(self, path);
    }

    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        for change in self.changes {
            if let Change::Review(Pattern::Struct(name), reason) = change
                && expr
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == name)
            {
                self.todo(expr.path.segments[0].ident.span(), reason.to_string());
            }
        }
        visit::visit_expr_struct(self, expr);
    }
}

/// Finds [`Pattern::Name`] and [`Pattern::Text`] in the tokens, inside macros too.
fn review_tokens(tokens: TokenStream, skip: usize, changes: &[Change], todos: &mut Vec<Todo>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (i, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(g) => review_tokens(g.stream(), skip, changes, todos),
            TokenTree::Ident(_) => {
                for change in changes {
                    let Change::Review(Pattern::Name(name), reason) = change else {
                        continue;
                    };
                    if path_at(&tokens, i, name) {
                        todos.push(Todo {
                            at: token.span().byte_range().start + skip,
                            reason: reason.to_string(),
                        });
                    }
                }
            }
            TokenTree::Literal(lit) => {
                let lit = lit.to_string();
                for change in changes {
                    if let Change::Review(Pattern::Text(text), reason) = change
                        && lit.contains(text)
                    {
                        todos.push(Todo {
                            at: token.span().byte_range().start + skip,
                            reason: reason.to_string(),
                        });
                    }
                }
            }
            TokenTree::Punct(_) => {}
        }
    }
}

/// Whether the tokens from `i` spell `name`, a single name or a path such as `A::B`, and the
/// token before is not `::` (so `x::A::B` does not match `A::B` halfway).
fn path_at(tokens: &[TokenTree], i: usize, name: &str) -> bool {
    let mut at = i;
    for (n, part) in name.split("::").enumerate() {
        if n > 0 {
            let colons = matches!(
                (tokens.get(at), tokens.get(at + 1)),
                (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b))) if a.as_char() == ':' && b.as_char() == ':'
            );
            if !colons {
                return false;
            }
            at += 2;
        }
        match tokens.get(at) {
            Some(TokenTree::Ident(id)) if *id == part => at += 1,
            _ => return false,
        }
    }
    !name.contains("::")
        || i < 2
        || !matches!(&tokens[i - 1], TokenTree::Punct(p) if p.as_char() == ':')
}

/// Edits that replace whole identifiers outside string and char literals. Comments are
/// rewritten too, so doc examples follow.
pub fn rename_edits(text: &str, renames: &[(&str, &str)]) -> Vec<Edit> {
    let mut edits = Vec::new();
    if renames.is_empty() {
        return edits;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut bytes: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .collect();
    bytes.push(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"'
            || (c == 'r' && matches!(chars.get(i + 1), Some('"' | '#')) && !ident_before(&chars, i))
        {
            i = string_end(&chars, i);
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            i += 3;
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if chars.get(i) == Some(&':') && chars.get(i + 1) == Some(&':') {
                let mut j = i + 2;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let path: String = chars[start..j].iter().collect();
                if let Some((old, new)) = renames
                    .iter()
                    .find(|(old, _)| *old == path)
                {
                    edits.push(rename(bytes[start], bytes[j], old, new));
                    i = j;
                    continue;
                }
            }
            if let Some((old, new)) = renames
                .iter()
                .find(|(old, _)| *old == word)
            {
                edits.push(rename(bytes[start], bytes[i], old, new));
            }
        } else {
            i += 1;
        }
    }
    edits
}

fn rename(start: usize, end: usize, old: &str, new: &str) -> Edit {
    Edit {
        start,
        end,
        text: new.to_string(),
        note: format!("`{old}` -> `{new}`"),
    }
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

/// Moves the version of each Zeyn dependency below `to` in a manifest to `to`, keeping its
/// operator (`^`, `=`, `~`). Path only and workspace dependencies stay as they are.
fn bump_manifest(text: &str, to: (u64, u64, u64), to_text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        match dependency_version(line) {
            Some(range) if version(&line[range.clone()]).is_ok_and(|v| v < to) => {
                out.push_str(&line[..range.start]);
                out.push_str(to_text);
                out.push_str(&line[range.end..]);
            }
            _ => out.push_str(line),
        }
    }
    out
}

/// The byte range of the version number in a dependency line of a Zeyn crate, such as
/// `zeyn = "0.0.9"` or `zeyn = { version = "^0.0.9", features = [..] }`.
fn dependency_version(line: &str) -> Option<std::ops::Range<usize>> {
    let (key, value) = line.split_once('=')?;
    if !CRATES.contains(&key.trim().trim_matches('"')) {
        return None;
    }
    let base = key.len() + 1;
    let quoted = match value.trim_start().starts_with('{') {
        true => {
            let at = value.find("version")?;
            let rest = &value[at + "version".len()..];
            let open = rest.find('"')?;
            base + at + "version".len() + open
        }
        false => base + value.find('"')?,
    };
    let start = quoted + 1;
    let len = line[start..].find('"')?;
    let skip = line[start..start + len].len()
        - line[start..start + len]
            .trim_start_matches(['^', '=', '~', ' '])
            .len();
    Some(start + skip..start + len)
}

/// The Zeyn version the project uses: Cargo.lock first, then the dependency in Cargo.toml.
fn current_version(root: &Path) -> Result<String, String> {
    if let Ok(lock) = std::fs::read_to_string(root.join("Cargo.lock")) {
        let mut lines = lock.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "name = \"zeyn\""
                && let Some(v) = lines
                    .next()
                    .and_then(|l| l.trim().strip_prefix("version = \""))
            {
                return Ok(v.trim_end_matches('"').to_string());
            }
        }
    }
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    manifest
        .lines()
        .find_map(|l| {
            dependency_version(l)
                .filter(|_| l.trim_start().starts_with("zeyn "))
                .map(|r| l[r].to_string())
        })
        .ok_or_else(|| {
            "cannot tell the Zeyn version of this project from Cargo.lock or Cargo.toml; pass --from <version>"
                .to_string()
        })
}

/// The newest step's version or this CLI's version, whichever is newer.
fn latest() -> String {
    let cli = env!("CARGO_PKG_VERSION");
    STEPS
        .last()
        .map(|s| s.version)
        .filter(|v| version(v).ok() > version(cli).ok())
        .unwrap_or(cli)
        .to_string()
}

/// `major.minor.patch`, without a pre-release or build suffix.
pub fn version(text: &str) -> Result<(u64, u64, u64), String> {
    let core = text
        .trim()
        .split(['-', '+'])
        .next()
        .unwrap_or("");
    let parts: Vec<u64> = core
        .split('.')
        .map(|p| p.parse::<u64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("`{text}` is not a version such as 0.0.9"))?;
    match parts[..] {
        [a, b, c] => Ok((a, b, c)),
        [a, b] => Ok((a, b, 0)),
        _ => Err(format!("`{text}` is not a version such as 0.0.9")),
    }
}

/// The Rust files and manifests of a project, without `target` and hidden folders.
fn project_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .into_owned();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                project_files(&path, out);
            }
        } else if name.ends_with(".rs") || name == "Cargo.toml" {
            out.push(path);
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn line_of(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

fn one_line(text: &str) -> String {
    let joined = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match joined.char_indices().nth(80) {
        Some((at, _)) => format!("{}...", &joined[..at]),
        None => joined,
    }
}

/// A unified diff with three lines of context. Lines are matched again within a window of 64
/// lines after a difference, which covers the local edits of a migration with constant memory.
pub fn unified_diff(name: &str, old: &str, new: &str) -> String {
    let a: Vec<&str> = old.split_inclusive('\n').collect();
    let b: Vec<&str> = new.split_inclusive('\n').collect();
    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
            continue;
        }
        let (da, db) = resync(&a[i..], &b[j..]);
        ops.extend(a[i..i + da].iter().map(|l| ('-', *l)));
        ops.extend(b[j..j + db].iter().map(|l| ('+', *l)));
        i += da;
        j += db;
    }
    let changed: Vec<usize> = (0..ops.len())
        .filter(|&k| ops[k].0 != ' ')
        .collect();
    if changed.is_empty() {
        return String::new();
    }
    let mut out = format!("--- a/{name}\n+++ b/{name}\n");
    let mut k = 0;
    while k < changed.len() {
        let first = changed[k].saturating_sub(3);
        let mut last = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= last + 7 {
            k += 1;
            last = changed[k];
        }
        k += 1;
        let last = (last + 3).min(ops.len() - 1);
        let old_start = 1 + ops[..first]
            .iter()
            .filter(|o| o.0 != '+')
            .count();
        let new_start = 1 + ops[..first]
            .iter()
            .filter(|o| o.0 != '-')
            .count();
        let hunk = &ops[first..=last];
        out.push_str(&format!(
            "@@ -{old_start},{} +{new_start},{} @@\n",
            hunk.iter()
                .filter(|o| o.0 != '+')
                .count(),
            hunk.iter()
                .filter(|o| o.0 != '-')
                .count()
        ));
        for (op, line) in hunk {
            out.push(*op);
            out.push_str(line.trim_end_matches(['\n', '\r']));
            out.push('\n');
        }
    }
    out
}

/// How many lines of `a` and `b` to take as changed before they match again.
fn resync(a: &[&str], b: &[&str]) -> (usize, usize) {
    for k in 1..=64 {
        for da in 0..=k {
            let db = k - da;
            if da < a.len() && db < b.len() && a[da] == b[db] {
                return (da, db);
            }
        }
    }
    (a.len(), b.len())
}

#[cfg(test)]
mod tests;
