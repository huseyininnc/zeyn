//! [`CodeView`] and [`JsonView`]: read only source text with syntax highlighting done on the
//! server, line numbers and folding, and the highlighter behind them ([`highlight`]).
//!
//! The highlighter is written by hand for a few languages ([`CodeLang`]). Its output is escaped
//! text with `<span class="z-hl-*">` around keywords, strings, numbers, comments, types, macros
//! and keys; the colors come from theme tokens in `zeyn.css`, so they follow the light and dark
//! themes and cost nothing in the browser.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::{Render, escape, escape_into};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(CodeView, JsonView);

/// Most bytes of text one [`CodeView`] shows; the rest is cut at a line end with a notice.
pub const MAX_CODE_BYTES: usize = 1024 * 1024;

/// Most lines one [`CodeView`] shows; the rest is left out with a notice.
pub const MAX_CODE_LINES: usize = 10_000;

/// A language the highlighter knows. Anything else is shown as [`CodeLang::Plain`] text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeLang {
    /// Escaped text without colors.
    #[default]
    Plain,
    Rust,
    Json,
    Toml,
    Sql,
    JavaScript,
    Shell,
    /// Report and calculated field expressions: field references in brackets
    /// (`[Orders.Total]`), numbers with an optional `m` (decimal) suffix, strings in double or
    /// single quotes, operators, function names before `(` and the words `and`, `or`, `not`,
    /// `true`, `false`, `null`, `in`, `is`, `like` and `between`, matched ignoring case.
    Expression,
}

impl CodeLang {
    /// The language of a code fence or file extension name: `rust`, `rs`, `json`, `toml`, `sql`,
    /// `js`, `sh`, ...; [`CodeLang::Plain`] for any other.
    ///
    /// ```
    /// use zeyn::ui::CodeLang;
    /// assert_eq!(CodeLang::named("RS"), CodeLang::Rust);
    /// assert_eq!(CodeLang::named("html"), CodeLang::Plain);
    /// ```
    pub fn named(name: &str) -> CodeLang {
        match name.to_ascii_lowercase().as_str() {
            "rust" | "rs" => CodeLang::Rust,
            "json" => CodeLang::Json,
            "toml" => CodeLang::Toml,
            "sql" | "psql" | "plsql" => CodeLang::Sql,
            "js" | "javascript" | "mjs" => CodeLang::JavaScript,
            "sh" | "shell" | "bash" | "console" => CodeLang::Shell,
            "expression" | "expr" => CodeLang::Expression,
            _ => CodeLang::Plain,
        }
    }

    /// The name the browser and the schema use: `plain`, `rust`, `sql`, `expression`, ...
    pub(crate) fn name(self) -> &'static str {
        match self {
            CodeLang::Plain => "plain",
            CodeLang::Rust => "rust",
            CodeLang::Json => "json",
            CodeLang::Toml => "toml",
            CodeLang::Sql => "sql",
            CodeLang::JavaScript => "javascript",
            CodeLang::Shell => "shell",
            CodeLang::Expression => "expression",
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            CodeLang::Rust => RUST,
            CodeLang::JavaScript => JS,
            CodeLang::Json => &["true", "false", "null"],
            CodeLang::Toml => &["true", "false"],
            CodeLang::Shell => SHELL,
            CodeLang::Sql => SQL,
            CodeLang::Expression => EXPRESSION,
            CodeLang::Plain => &[],
        }
    }

    fn line_comment(self) -> Option<&'static str> {
        match self {
            CodeLang::Rust | CodeLang::JavaScript => Some("//"),
            CodeLang::Toml | CodeLang::Shell => Some("#"),
            CodeLang::Sql => Some("--"),
            CodeLang::Json | CodeLang::Plain | CodeLang::Expression => None,
        }
    }
}

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const JS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

const SHELL: &[&str] = &[
    "if", "then", "else", "fi", "for", "do", "done", "export", "cd", "echo",
];

/// Words of [`CodeLang::Expression`], matched ignoring case. `codeedit.js` holds the same list.
const EXPRESSION: &[&str] = &[
    "and", "between", "false", "in", "is", "like", "not", "null", "or", "true",
];

/// Operator characters of [`CodeLang::Expression`]. `codeedit.js` holds the same set.
const OPERATORS: &str = "+-*/%<>=!&|^";

/// SQL keywords, matched ignoring case. `codeedit.js` holds the same list for the editor.
const SQL: &[&str] = &[
    "add",
    "all",
    "alter",
    "and",
    "as",
    "asc",
    "begin",
    "between",
    "by",
    "case",
    "check",
    "commit",
    "constraint",
    "create",
    "cross",
    "default",
    "delete",
    "desc",
    "distinct",
    "drop",
    "else",
    "end",
    "exists",
    "false",
    "fetch",
    "first",
    "foreign",
    "from",
    "full",
    "group",
    "having",
    "if",
    "in",
    "index",
    "inner",
    "insert",
    "into",
    "is",
    "join",
    "key",
    "left",
    "like",
    "limit",
    "not",
    "null",
    "offset",
    "on",
    "or",
    "order",
    "outer",
    "over",
    "partition",
    "primary",
    "references",
    "returning",
    "right",
    "rollback",
    "rows",
    "select",
    "set",
    "table",
    "then",
    "true",
    "union",
    "unique",
    "update",
    "using",
    "values",
    "view",
    "when",
    "where",
    "with",
];

/// Escaped, highlighted HTML for `code` in `lang`.
///
/// ```
/// use zeyn::ui::{CodeLang, highlight};
/// let h = highlight(CodeLang::Sql, "SELECT name FROM t -- all");
/// assert!(h.starts_with(r#"<span class="z-hl-kw">SELECT</span> name"#));
/// assert!(h.ends_with(r#"<span class="z-hl-com">-- all</span>"#));
/// assert_eq!(highlight(CodeLang::Plain, "<b>"), "&lt;b&gt;");
/// ```
pub fn highlight(lang: CodeLang, code: &str) -> String {
    if lang == CodeLang::Plain {
        return escape(code);
    }
    Tokens {
        lang,
        chars: code.chars().collect(),
        out: String::with_capacity(code.len() * 2),
    }
    .run()
}

struct Tokens {
    lang: CodeLang,
    chars: Vec<char>,
    out: String,
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl Tokens {
    fn span(&mut self, class: &str, from: usize, to: usize) {
        self.out
            .push_str(r#"<span class="z-hl-"#);
        self.out.push_str(class);
        self.out.push_str(r#"">"#);
        let mut buf = [0u8; 4];
        for c in &self.chars[from..to] {
            escape_into(&mut self.out, c.encode_utf8(&mut buf));
        }
        self.out.push_str("</span>");
    }

    fn plain(&mut self, c: char) {
        let mut buf = [0u8; 4];
        escape_into(&mut self.out, c.encode_utf8(&mut buf));
    }

    fn at(&self, i: usize, pat: &str) -> bool {
        pat.chars()
            .enumerate()
            .all(|(k, p)| self.chars.get(i + k) == Some(&p))
    }

    fn line_start(&self, i: usize) -> bool {
        self.chars[..i]
            .iter()
            .rev()
            .take_while(|c| **c != '\n')
            .all(|c| c.is_whitespace())
    }

    /// End of a string that starts at `i` with `quote`, after the closing quote.
    fn string_end(&self, i: usize, quote: char) -> usize {
        let mut k = i + 1;
        while k < self.chars.len() {
            match self.chars[k] {
                '\'' if self.lang == CodeLang::Sql && quote == '\'' => {
                    if self.chars.get(k + 1) == Some(&'\'') {
                        k += 2;
                    } else {
                        return k + 1;
                    }
                }
                '\\' if self.lang != CodeLang::Sql
                    && (self.lang != CodeLang::Toml || quote == '"') =>
                {
                    k += 2
                }
                c if c == quote => return k + 1,
                '\n' if !matches!(self.lang, CodeLang::Rust | CodeLang::Sql) => return k,
                _ => k += 1,
            }
        }
        self.chars.len()
    }

    fn run(mut self) -> String {
        let mut i = 0;
        let n = self.chars.len();
        while i < n {
            let c = self.chars[i];
            if let Some(lc) = self.lang.line_comment()
                && self.at(i, lc)
                && (lc != "#" || i == 0 || self.chars[i - 1].is_whitespace())
            {
                let end = (i..n)
                    .find(|&k| self.chars[k] == '\n')
                    .unwrap_or(n);
                self.span("com", i, end);
                i = end;
                continue;
            }
            if matches!(
                self.lang,
                CodeLang::Rust | CodeLang::JavaScript | CodeLang::Sql
            ) && self.at(i, "/*")
            {
                let end = (i + 2..n)
                    .find(|&k| self.at(k, "*/"))
                    .map(|k| k + 2)
                    .unwrap_or(n);
                self.span("com", i, end);
                i = end;
                continue;
            }
            if self.lang == CodeLang::Rust
                && c == '#'
                && matches!(self.chars.get(i + 1), Some('[') | Some('!'))
            {
                let mut depth = 0;
                let mut end = i + 1;
                while end < n {
                    match self.chars[end] {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                end += 1;
                                break;
                            }
                        }
                        '\n' => break,
                        _ => {}
                    }
                    end += 1;
                }
                self.span("mac", i, end);
                i = end;
                continue;
            }
            if self.lang == CodeLang::Rust
                && c == 'r'
                && (self.at(i, "r\"") || self.at(i, "r#\""))
                && (i == 0 || !ident_char(self.chars[i - 1]))
            {
                let hashes = self.chars[i + 1..]
                    .iter()
                    .take_while(|c| **c == '#')
                    .count();
                let close: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let start = i + 2 + hashes;
                let end = (start..n)
                    .find(|&k| self.at(k, &close))
                    .map(|k| k + close.len())
                    .unwrap_or(n);
                self.span("str", i, end);
                i = end;
                continue;
            }
            if c == '"'
                || (c == '\''
                    && matches!(
                        self.lang,
                        CodeLang::JavaScript
                            | CodeLang::Shell
                            | CodeLang::Toml
                            | CodeLang::Sql
                            | CodeLang::Expression
                    ))
                || (c == '`' && self.lang == CodeLang::JavaScript)
            {
                let end = self.string_end(i, c);
                let next = self.chars[end..]
                    .iter()
                    .find(|c| !c.is_whitespace());
                let key = (self.lang == CodeLang::Json && next == Some(&':'))
                    || (self.lang == CodeLang::Toml && self.line_start(i) && next == Some(&'='))
                    || (self.lang == CodeLang::Sql && c == '"');
                self.span(if key { "key" } else { "str" }, i, end);
                i = end;
                continue;
            }
            if c == '\'' && self.lang == CodeLang::Rust {
                let close = if self.chars.get(i + 1) == Some(&'\\') {
                    (i + 2..(i + 12).min(n)).find(|&k| self.chars[k] == '\'')
                } else {
                    (self.chars.get(i + 2) == Some(&'\'')).then_some(i + 2)
                };
                if let Some(end) = close {
                    self.span("str", i, end + 1);
                    i = end + 1;
                    continue;
                }
            }
            if self.lang == CodeLang::Expression && c == '[' {
                let end = (i + 1..n)
                    .find(|&k| matches!(self.chars[k], ']' | '\n'))
                    .map_or(n, |k| k + usize::from(self.chars[k] == ']'));
                self.span("key", i, end);
                i = end;
                continue;
            }
            if self.lang == CodeLang::Expression && OPERATORS.contains(c) {
                let end = (i..n)
                    .find(|&k| !OPERATORS.contains(self.chars[k]))
                    .unwrap_or(n);
                self.span("op", i, end);
                i = end;
                continue;
            }
            if c.is_ascii_digit() && (i == 0 || !ident_char(self.chars[i - 1])) {
                let end = (i..n)
                    .find(|&k| {
                        !(ident_char(self.chars[k])
                            || (self.chars[k] == '.'
                                && self
                                    .chars
                                    .get(k + 1)
                                    .is_some_and(char::is_ascii_digit)))
                    })
                    .unwrap_or(n);
                self.span("num", i, end);
                i = end;
                continue;
            }
            if self.lang == CodeLang::Toml && c == '[' && self.line_start(i) {
                let end = (i..n)
                    .find(|&k| self.chars[k] == '\n')
                    .unwrap_or(n);
                self.span("kw", i, end);
                i = end;
                continue;
            }
            if self.lang == CodeLang::Shell
                && c == '-'
                && (i == 0 || self.chars[i - 1].is_whitespace())
            {
                let end = (i..n)
                    .find(|&k| self.chars[k].is_whitespace() || self.chars[k] == '=')
                    .unwrap_or(n);
                self.span("ty", i, end);
                i = end;
                continue;
            }
            if ident_char(c) {
                let end = (i..n)
                    .find(|&k| {
                        !(ident_char(self.chars[k])
                            || (self.lang == CodeLang::Shell && matches!(self.chars[k], '-' | '.')))
                    })
                    .unwrap_or(n);
                let word: String = self.chars[i..end].iter().collect();
                let next = self.chars[end..]
                    .iter()
                    .find(|c| !c.is_whitespace() || **c == '\n')
                    .copied();
                let keyword = match self.lang {
                    CodeLang::Sql | CodeLang::Expression => self
                        .lang
                        .keywords()
                        .contains(&word.to_ascii_lowercase().as_str()),
                    _ => self
                        .lang
                        .keywords()
                        .contains(&word.as_str()),
                };
                let class = if keyword {
                    Some("kw")
                } else if self.lang == CodeLang::Rust && self.chars.get(end) == Some(&'!') {
                    self.span("mac", i, end + 1);
                    i = end + 1;
                    continue;
                } else if self.lang == CodeLang::Toml && self.line_start(i) && next == Some('=') {
                    Some("key")
                } else if self.lang == CodeLang::Shell && self.line_start(i) {
                    Some("kw")
                } else if (matches!(self.lang, CodeLang::Rust | CodeLang::JavaScript)
                    && c.is_uppercase())
                    || (self.lang == CodeLang::Expression && self.chars.get(end) == Some(&'('))
                {
                    Some("ty")
                } else {
                    None
                };
                match class {
                    Some(k) => self.span(k, i, end),
                    None => escape_into(&mut self.out, &word),
                }
                i = end;
                continue;
            }
            self.plain(c);
            i += 1;
        }
        self.out
    }
}

/// Highlighted HTML cut into lines: a span that runs over a line end is closed at the end of
/// the line and opened again on the next, so every line stands alone.
fn lines_of(html: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut open: Option<&str> = None;
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(tail) = rest.strip_prefix("</span>") {
            line.push_str("</span>");
            open = None;
            rest = tail;
        } else if rest.starts_with("<span ") {
            let end = rest
                .find('>')
                .map_or(rest.len(), |e| e + 1);
            open = Some(&rest[..end]);
            line.push_str(&rest[..end]);
            rest = &rest[end..];
        } else if let Some(tail) = rest.strip_prefix('\n') {
            if open.is_some() {
                line.push_str("</span>");
            }
            lines.push(std::mem::take(&mut line));
            if let Some(tag) = open {
                line.push_str(tag);
            }
            rest = tail;
        } else {
            let end = rest
                .find(['<', '\n'])
                .unwrap_or(rest.len())
                .max(1);
            line.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    lines.push(line);
    lines
}

/// Brackets a line opens and closes outside strings and comments, in order: `true` opens.
fn brackets(line: &str) -> Vec<bool> {
    let b = line.as_bytes();
    let mut found = Vec::new();
    let mut in_span = false;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' => {
                in_span = b.get(i + 1) != Some(&b'/');
                i += b[i..]
                    .iter()
                    .position(|&c| c == b'>')
                    .unwrap_or(b.len() - i);
            }
            b'{' | b'[' if !in_span => found.push(true),
            b'}' | b']' if !in_span => found.push(false),
            _ => {}
        }
        i += 1;
    }
    found
}

/// Fold ranges `(first, last)` of the lines: a bracket opened on `first` and closed on `last`,
/// the outermost one for each first line, nested properly.
fn folds(lines: &[String]) -> Vec<(usize, usize)> {
    let mut stack: Vec<usize> = Vec::new();
    let mut found: Vec<(usize, usize)> = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        for open in brackets(line) {
            if open {
                stack.push(n);
            } else if let Some(first) = stack.pop()
                && n > first
            {
                match found
                    .iter_mut()
                    .find(|(f, _)| *f == first)
                {
                    Some(r) => r.1 = r.1.max(n),
                    None => found.push((first, n)),
                }
            }
        }
    }
    found.sort_unstable();
    let mut nested: Vec<(usize, usize)> = Vec::with_capacity(found.len());
    let mut open: Vec<usize> = Vec::new();
    for (first, last) in found {
        while open
            .last()
            .is_some_and(|&end| end < first)
        {
            open.pop();
        }
        if open
            .last()
            .is_some_and(|&end| last > end)
        {
            continue;
        }
        open.push(last);
        nested.push((first, last));
    }
    nested
}

/// Source text shown read only: highlighted on the server, with line numbers, folding of
/// bracketed blocks (`{}` and `[]`) through `<details>` (no script) and a copy button. At most
/// [`MAX_CODE_LINES`] lines and [`MAX_CODE_BYTES`] bytes are shown, with a notice that says how
/// much was left out; the copy button copies what is shown.
///
/// The text is a region with its own scroll bar and one tab stop, named by the title or the
/// language; every fold is a native disclosure button (Enter or Space opens and closes it).
///
/// ```
/// use zeyn::ui::{CodeLang, CodeView};
/// use zeyn::{Component, I18n, Render};
///
/// let view = CodeView::new(CodeLang::Rust, "fn main() {\n    run();\n}").title("main.rs");
/// let mut html = String::new();
/// view.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains(r#"<span class="z-hl-kw">fn</span>"#));
/// assert!(html.contains("<details class=\"z-codeview-fold\" open>"));
/// ```
pub struct CodeView {
    lang: CodeLang,
    text: String,
    lines: usize,
    shown: usize,
    title: Option<Text>,
}

impl CodeView {
    /// `text` in `lang`, cut to the limits.
    pub fn new(lang: CodeLang, text: impl Into<String>) -> Self {
        let mut text = text.into();
        let lines = text.lines().count().max(1);
        if text.len() > MAX_CODE_BYTES {
            let mut cut = MAX_CODE_BYTES;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            cut = text[..cut].rfind('\n').unwrap_or(cut);
            text.truncate(cut);
        }
        if let Some((at, _)) = text
            .match_indices('\n')
            .nth(MAX_CODE_LINES - 1)
        {
            text.truncate(at);
        }
        let trimmed = text.trim_end_matches('\n').len();
        text.truncate(trimmed);
        let shown = text.lines().count().max(1);
        CodeView {
            lang,
            text,
            lines,
            shown,
            title: None,
        }
    }

    /// A heading over the text, such as a file name, which also names the region.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Whether lines were left out.
    pub fn truncated(&self) -> bool {
        self.shown < self.lines
    }
}

impl Component for CodeView {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.code.{key}")));
        let id = super::dom_id(r, "code");
        let name = match &self.title {
            Some(title) => r.text(title),
            None => t(self.lang.name()),
        };
        let lines = lines_of(&highlight(self.lang, &self.text));
        let count = r.text(&Text::key("zeyn.ui.code.lines").arg("n", self.shown.to_string()));
        out.reserve(self.text.len() * 3 + lines.len() * 96);
        out.push_str(&format!(
            r#"<section class="z-codeview" data-testid="{}.code"><div class="z-codeview-head"><h2 class="z-codeview-title" id="{id}-title">{name}</h2><span class="z-codeview-meta">{count}</span><button type="button" class="z-button z-codeview-copy" data-zeyn-copy="{id}-body" data-copied="{}" data-failed="{}" hidden>{}</button></div>"#,
            r.scope,
            t("copied"),
            t("failed"),
            t("copy")
        ));
        out.push_str(&format!(
            r#"<div class="z-codeview-body" id="{id}-body" role="region" tabindex="0" aria-labelledby="{id}-title" style="--z-codeview-digits:{}">"#,
            self.shown.to_string().len()
        ));
        let folds = folds(&lines);
        let mut next = folds.iter().peekable();
        let mut ends: Vec<usize> = Vec::new();
        let fold = t("fold");
        for (n, line) in lines.iter().enumerate() {
            let starts = next.peek().is_some_and(|f| f.0 == n);
            if starts {
                let (_, last) = next.next().copied().unwrap_or_default();
                ends.push(last);
                out.push_str(r#"<details class="z-codeview-fold" open><summary title=""#);
                out.push_str(&fold);
                out.push_str(r#""><code class="z-codeview-l">"#);
            } else {
                out.push_str(r#"<code class="z-codeview-l">"#);
            }
            out.push_str(line);
            out.push_str(if starts {
                "</code></summary>"
            } else {
                "</code>"
            });
            while ends.last() == Some(&n) {
                ends.pop();
                out.push_str("</details>");
            }
        }
        out.push_str("</div>");
        if self.truncated() {
            out.push_str(&format!(
                r#"<p class="z-codeview-truncated" role="note">{}</p>"#,
                r.text(
                    &Text::key("zeyn.ui.code.truncated")
                        .arg("shown", self.shown.to_string())
                        .arg("total", self.lines.to_string())
                )
            ));
        }
        out.push_str("</section>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "code_view",
            "lang": self.lang.name(),
            "title": self.title,
            "lines": self.lines,
            "shown": self.shown,
        })
    }
}

/// A JSON value shown as indented, highlighted text in a [`CodeView`], with folding of every
/// object and array. [`JsonView::of`] takes any value and masks its masked fields for the
/// user first, so a model with `#[mask]` fields can be shown as it is.
///
/// ```
/// use zeyn::ui::JsonView;
/// use zeyn::{Component, I18n, Render};
///
/// let view = JsonView::new(&serde_json::json!({ "id": 7, "tags": ["a", "b"] }));
/// let mut html = String::new();
/// view.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert!(html.contains(r#"<span class="z-hl-key">&quot;id&quot;</span>"#));
/// ```
pub struct JsonView(CodeView);

impl JsonView {
    /// A JSON value, indented with two spaces.
    pub fn new(value: &Json) -> Self {
        let text = serde_json::to_string_pretty(value).unwrap_or_default();
        JsonView(CodeView::new(CodeLang::Json, text))
    }

    /// Any serializable value, with its masked fields masked for the user of `ctx`, as the
    /// endpoints send it to the browser.
    pub fn of<T: serde::Serialize>(value: &T, ctx: &crate::ctx::Ctx) -> Self {
        let json = crate::privacy::client_json(value, ctx).unwrap_or(Json::Null);
        JsonView::new(&json)
    }

    /// A heading over the text, which also names the region.
    pub fn title(self, title: impl Into<Text>) -> Self {
        JsonView(self.0.title(title))
    }
}

impl Component for JsonView {
    fn render(&self, out: &mut String, r: &Render) {
        self.0.render(out, r);
    }

    fn schema(&self) -> Json {
        let mut schema = self.0.schema();
        schema["type"] = json!("json_view");
        schema
    }
}
