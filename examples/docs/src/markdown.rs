//! A small Markdown renderer for the subset the documentation uses: headings, paragraphs, lists,
//! tables, fenced code blocks, inline code, links, bold, italic, block quotes and rules.
//!
//! Every character of the source is escaped. There is no raw HTML passthrough: `<b>` in a page
//! shows up as text. Links may only point to `http(s)`, `mailto`, an app path or an anchor;
//! anything else (such as `javascript:`) is rendered as plain text. A line `@live name` places
//! the live example `name` there; the page keeps it as a separate part.

use crate::highlight::highlight;
use zeyn::render::escape;

/// A heading of a page, for "On this page" and search.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// A piece of a rendered page: HTML, or the place of a live example.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    Html(String),
    Live(String),
}

/// The plain text of a page section, for search: the heading it is under and its words.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub id: String,
    pub heading: String,
    pub text: String,
}

/// A rendered page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rendered {
    /// The text of the first `# ` heading, which is not part of the HTML.
    pub title: String,
    pub parts: Vec<Part>,
    pub headings: Vec<Heading>,
    pub sections: Vec<Section>,
}

/// Renders Markdown. `link` turns a link target into an app path (such as `theming.md` into
/// `/guides/theming`) or returns it unchanged.
pub fn render(source: &str, link: &dyn Fn(&str) -> String) -> Rendered {
    render_with_ids(source, link, &[])
}

/// [`render`] with fixed heading anchors: the heading at position `n` gets `ids[n]`, so a
/// translated page keeps the anchors of the English page and links such as
/// `security.md#csrf-csp-and-headers` work in every language. Headings past the end of `ids`
/// get an anchor from their text.
pub fn render_with_ids(source: &str, link: &dyn Fn(&str) -> String, ids: &[String]) -> Rendered {
    let mut r = Renderer {
        link,
        out: Rendered::default(),
        html: String::new(),
        ids: Vec::new(),
        fixed: ids,
    };
    r.out.sections.push(Section {
        id: String::new(),
        heading: String::new(),
        text: String::new(),
    });
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        i = r.block(&lines, i);
    }
    r.flush();
    r.out
}

struct Renderer<'a> {
    link: &'a dyn Fn(&str) -> String,
    out: Rendered,
    html: String,
    ids: Vec<String>,
    fixed: &'a [String],
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The marker of a list item: ordered or not, and the text after it.
fn list_item(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    if let Some(rest) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
    {
        return Some((false, rest));
    }
    let digits = t
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    if digits > 0
        && digits < 4
        && let Some(rest) = t[digits..].strip_prefix(". ")
    {
        return Some((true, rest));
    }
    None
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*'))
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|')
        && t.contains('-')
        && t.chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Cells of a table row. `|` inside inline code and `\|` do not split.
fn cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|');
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut code = false;
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '`' => {
                code = !code;
                cell.push(c);
            }
            '|' if !code => cells.push(
                std::mem::take(&mut cell)
                    .trim()
                    .to_string(),
            ),
            _ => cell.push(c),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

/// A heading id: lower case letters and digits joined by `-`.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "section".into()
    } else {
        out
    }
}

/// Text without Markdown marks, for headings in search and "On this page".
fn plain(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' | '*' => {}
            '[' => {}
            ']' if chars.peek() == Some(&'(') => {
                for d in chars.by_ref() {
                    if d == ')' {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

impl Renderer<'_> {
    /// Moves the HTML written so far into a part.
    fn flush(&mut self) {
        if !self.html.is_empty() {
            self.out
                .parts
                .push(Part::Html(std::mem::take(&mut self.html)));
        }
    }

    fn words(&mut self, text: &str) {
        let section = self
            .out
            .sections
            .last_mut()
            .expect("there is always a section");
        if !section.text.is_empty() {
            section.text.push(' ');
        }
        section.text.push_str(&plain(text));
    }

    /// Renders the block that starts at line `i` and returns the line after it.
    fn block(&mut self, lines: &[&str], i: usize) -> usize {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            return i + 1;
        }
        if let Some(fence) = t.strip_prefix("```") {
            let lang = fence.trim();
            let mut end = i + 1;
            while end < lines.len()
                && !lines[end]
                    .trim_start()
                    .starts_with("```")
            {
                end += 1;
            }
            let code = lines[i + 1..end].join("\n");
            let class = if lang.is_empty() {
                String::new()
            } else {
                format!(r#" class="language-{}""#, escape(lang))
            };
            self.html.push_str(&format!(
                r#"<pre class="z-code" tabindex="0"><code{class}>{}</code></pre>"#,
                highlight(lang, &code)
            ));
            self.words(&code);
            return (end + 1).min(lines.len());
        }
        if let Some(name) = t.strip_prefix("@live ") {
            self.flush();
            self.out
                .parts
                .push(Part::Live(name.trim().to_string()));
            return i + 1;
        }
        let hashes = t
            .bytes()
            .take_while(|b| *b == b'#')
            .count();
        if (1..=4).contains(&hashes) && t[hashes..].starts_with(' ') {
            let text = t[hashes..].trim();
            if hashes == 1 && self.out.title.is_empty() {
                self.out.title = plain(text);
                return i + 1;
            }
            let level = hashes.max(2) as u8;
            let mut id = self
                .fixed
                .get(self.out.headings.len())
                .cloned()
                .unwrap_or_else(|| slug(&plain(text)));
            if self.ids.contains(&id) {
                let mut n = 2;
                while self.ids.contains(&format!("{id}-{n}")) {
                    n += 1;
                }
                id = format!("{id}-{n}");
            }
            self.ids.push(id.clone());
            self.html.push_str(&format!(
                r#"<h{level} id="{id}">{}</h{level}>"#,
                self.inline(text)
            ));
            self.out.headings.push(Heading {
                level,
                id: id.clone(),
                text: plain(text),
            });
            self.out.sections.push(Section {
                id,
                heading: plain(text),
                text: String::new(),
            });
            return i + 1;
        }
        if is_rule(t) {
            self.html.push_str("<hr>");
            return i + 1;
        }
        if t.starts_with('|') && i + 1 < lines.len() && is_table_separator(lines[i + 1]) {
            return self.table(lines, i);
        }
        if list_item(line).is_some() {
            return self.list(lines, i, indent_of(line));
        }
        if t.starts_with('>') {
            let mut end = i;
            let mut text = Vec::new();
            while end < lines.len() && lines[end].trim().starts_with('>') {
                text.push(
                    lines[end]
                        .trim()
                        .trim_start_matches('>')
                        .trim(),
                );
                end += 1;
            }
            let joined = text.join(" ");
            self.words(&joined);
            self.html.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>",
                self.inline(&joined)
            ));
            return end;
        }
        let mut end = i;
        let mut text = Vec::new();
        while end < lines.len() {
            let l = lines[end].trim();
            if l.is_empty()
                || l.starts_with("```")
                || l.starts_with('#')
                || l.starts_with('|')
                || l.starts_with('>')
                || l.starts_with("@live ")
                || list_item(lines[end]).is_some()
                || (end > i && is_rule(l))
            {
                break;
            }
            text.push(l);
            end += 1;
        }
        if text.is_empty() {
            text.push(t);
            end = i + 1;
        }
        let joined = text.join(" ");
        self.words(&joined);
        self.html
            .push_str(&format!("<p>{}</p>", self.inline(&joined)));
        end
    }

    fn table(&mut self, lines: &[&str], i: usize) -> usize {
        let head = cells(lines[i]);
        self.html.push_str("<table><thead><tr>");
        for c in &head {
            self.words(c);
            self.html
                .push_str(&format!(r#"<th scope="col">{}</th>"#, self.inline(c)));
        }
        self.html
            .push_str("</tr></thead><tbody>");
        let mut end = i + 2;
        while end < lines.len() && lines[end].trim().starts_with('|') {
            self.html.push_str("<tr>");
            for c in cells(lines[end]) {
                self.words(&c);
                self.html
                    .push_str(&format!("<td>{}</td>", self.inline(&c)));
            }
            self.html.push_str("</tr>");
            end += 1;
        }
        self.html.push_str("</tbody></table>");
        end
    }

    /// A list whose items start at `indent`. Deeper items become nested lists; indented lines
    /// that are not items continue the item before them.
    fn list(&mut self, lines: &[&str], i: usize, indent: usize) -> usize {
        let ordered = list_item(lines[i])
            .map(|(o, _)| o)
            .unwrap_or(false);
        let tag = if ordered { "ol" } else { "ul" };
        self.html.push_str(&format!("<{tag}>"));
        let mut at = i;
        while at < lines.len() {
            let line = lines[at];
            let Some((_, text)) =
                list_item(line).filter(|(o, _)| indent_of(line) == indent && *o == ordered)
            else {
                break;
            };
            let mut item = text.trim().to_string();
            at += 1;
            while at < lines.len() {
                let next = lines[at];
                if next.trim().is_empty()
                    || list_item(next).is_some()
                    || indent_of(next) <= indent
                    || next.trim_start().starts_with("```")
                {
                    break;
                }
                item.push(' ');
                item.push_str(next.trim());
                at += 1;
            }
            self.words(&item);
            self.html
                .push_str(&format!("<li>{}", self.inline(&item)));
            if at < lines.len() && list_item(lines[at]).is_some() && indent_of(lines[at]) > indent {
                at = self.list(lines, at, indent_of(lines[at]));
            }
            self.html.push_str("</li>");
            if at < lines.len()
                && lines[at].trim().is_empty()
                && at + 1 < lines.len()
                && list_item(lines[at + 1])
                    .is_some_and(|(o, _)| o == ordered && indent_of(lines[at + 1]) == indent)
            {
                at += 1;
            }
        }
        self.html.push_str(&format!("</{tag}>"));
        at
    }

    /// Inline Markdown: code spans, links, bold and italic. Everything else is escaped.
    fn inline(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() + 16);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        let find = |from: usize, pat: &[char]| -> Option<usize> {
            (from..chars.len()).find(|&k| chars[k..].starts_with(pat))
        };
        let collect = |a: usize, b: usize| -> String { chars[a..b].iter().collect() };
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && i + 1 < chars.len() && "\\`*_[]()#|".contains(chars[i + 1]) {
                out.push_str(&escape(&chars[i + 1].to_string()));
                i += 2;
                continue;
            }
            if c == '`'
                && let Some(end) = find(i + 1, &['`'])
            {
                let code = collect(i + 1, end);
                let page = code
                    .split('#')
                    .next()
                    .is_some_and(|p| p.ends_with(".md"))
                    .then(|| (self.link)(&code));
                match page.filter(|p| p.starts_with('/')) {
                    Some(route) => out.push_str(&format!(
                        r#"<a href="{}"><code>{}</code></a>"#,
                        escape(&route),
                        escape(&code)
                    )),
                    None => out.push_str(&format!("<code>{}</code>", escape(&code))),
                }
                i = end + 1;
                continue;
            }
            if c == '!' && chars.get(i + 1) == Some(&'[') {
                i += 1;
                continue;
            }
            if c == '['
                && let Some(close) = find(i + 1, &[']', '('])
                && let Some(end) = find(close + 2, &[')'])
            {
                let label = self.inline(&collect(i + 1, close));
                let target = collect(close + 2, end);
                out.push_str(&self.anchor(&label, target.trim()));
                i = end + 1;
                continue;
            }
            if c == '*'
                && chars.get(i + 1) == Some(&'*')
                && let Some(end) = find(i + 2, &['*', '*']).filter(|e| *e > i + 2)
            {
                out.push_str(&format!(
                    "<strong>{}</strong>",
                    self.inline(&collect(i + 2, end))
                ));
                i = end + 2;
                continue;
            }
            if c == '*'
                && chars
                    .get(i + 1)
                    .is_some_and(|n| !n.is_whitespace() && *n != '*')
                && let Some(end) = find(i + 1, &['*'])
            {
                out.push_str(&format!("<em>{}</em>", self.inline(&collect(i + 1, end))));
                i = end + 1;
                continue;
            }
            out.push_str(&escape(&c.to_string()));
            i += 1;
        }
        out
    }

    /// A link, or only its label when the target is not allowed.
    fn anchor(&self, label: &str, target: &str) -> String {
        let target = (self.link)(target);
        let lower = target.to_ascii_lowercase();
        let external = lower.starts_with("https://") || lower.starts_with("http://");
        let allowed = external
            || lower.starts_with("mailto:")
            || target.starts_with('#')
            || (target.starts_with('/') && !target.starts_with("//"));
        if !allowed {
            return label.to_string();
        }
        let rel = if external {
            r#" rel="noopener noreferrer""#
        } else {
            ""
        };
        format!(r#"<a href="{}"{rel}>{label}</a>"#, escape(&target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(md: &str) -> String {
        render(md, &|t| t.to_string())
            .parts
            .into_iter()
            .map(|p| match p {
                Part::Html(h) => h,
                Part::Live(n) => format!("[live {n}]"),
            })
            .collect()
    }

    #[test]
    fn blocks_and_inline_marks() {
        let out = render(
            "# Title\n\nSome **bold**, *it* and `a<b>`.\n\n## A b C\n\n- one\n  more\n- two\n  - deep\n\n1. first\n2. second\n",
            &|t| t.to_string(),
        );
        assert_eq!(out.title, "Title");
        let h: String = out
            .parts
            .iter()
            .map(|p| {
                if let Part::Html(h) = p {
                    h.clone()
                } else {
                    String::new()
                }
            })
            .collect();
        assert!(h.starts_with(
            "<p>Some <strong>bold</strong>, <em>it</em> and <code>a&lt;b&gt;</code>.</p>"
        ));
        assert!(h.contains(r#"<h2 id="a-b-c">A b C</h2>"#));
        assert!(
            h.contains("<ul><li>one more</li><li>two<ul><li>deep</li></ul></li></ul>"),
            "{h}"
        );
        assert!(h.contains("<ol><li>first</li><li>second</li></ol>"));
        assert_eq!(
            out.headings,
            vec![Heading {
                level: 2,
                id: "a-b-c".into(),
                text: "A b C".into()
            }]
        );
    }

    #[test]
    fn tables_split_cells_outside_code() {
        let out = html("| a | b |\n|---|---|\n| `x|y` | 2 |\n");
        assert_eq!(
            out,
            r#"<table><thead><tr><th scope="col">a</th><th scope="col">b</th></tr></thead><tbody><tr><td><code>x|y</code></td><td>2</td></tr></tbody></table>"#
        );
    }

    #[test]
    fn raw_html_and_unsafe_links_are_text() {
        let out = html(
            "<script>alert(1)</script> [x](javascript:alert(1)) [y](https://e.org) [z](/a#b)\n",
        );
        assert!(!out.contains("<script"));
        assert!(out.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!out.contains("javascript:"));
        assert!(out.contains(r#"<a href="https://e.org" rel="noopener noreferrer">y</a>"#));
        assert!(out.contains(r#"<a href="/a#b">z</a>"#));
    }

    #[test]
    fn code_blocks_are_escaped_and_live_examples_split_the_page() {
        let out = html("```rust\nlet s = \"<script>\";\n```\n\n@live grid\n\nafter\n");
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
        assert!(out.contains("[live grid]<p>after</p>"));
    }

    #[test]
    fn heading_ids_are_unique() {
        let out = render("## Same\n\n## Same\n", &|t| t.to_string());
        assert_eq!(
            out.headings
                .iter()
                .map(|h| h.id.as_str())
                .collect::<Vec<_>>(),
            ["same", "same-2"]
        );
    }
}
