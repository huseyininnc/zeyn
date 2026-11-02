//! [`Markdown`]: text with Markdown marks rendered as safe HTML (feature `markdown`).
//!
//! The subset: headings, paragraphs, lists (nested), tables, fenced code blocks, block quotes,
//! rules, inline code, links, images, bold and italic. Every character of the source is escaped;
//! raw HTML is never passed through, so `<script>` shows up as text. Links may only point to
//! `http(s)`, `mailto`, an anchor or a relative path; anything else (`javascript:`, `data:`,
//! `//host`, `/\host`) keeps only its label. Images load only from this app (a relative path);
//! others show their alt text, as the page's `img-src` policy would block them anyway. User
//! content gets `rel="noopener noreferrer nofollow ugc"` on links and no heading ids (so it
//! cannot clobber ids of the page); [`Markdown::trusted`] text written by the app gets ids and
//! plain `noopener noreferrer` on external links. Input is cut at [`MAX_MARKDOWN`] bytes, nesting
//! is capped, and the renderer is linear in the input apart from that cap.

use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};

/// Longest Markdown source rendered, in bytes; the rest is dropped.
pub const MAX_MARKDOWN: usize = 64 * 1024;

super::layout::into_node!(Markdown);

/// Deepest nesting of lists and of inline marks (links, bold, italic) inside each other.
const MAX_DEPTH: usize = 8;

/// Markdown text shown as formatted content.
///
/// ```
/// use zeyn::ui::Markdown;
/// let html = Markdown::new("**Hi** <script>x</script> [a](javascript:alert`1`)").html();
/// assert_eq!(html, "<p><strong>Hi</strong> &lt;script&gt;x&lt;/script&gt; a</p>");
/// ```
#[derive(Clone, Debug)]
pub struct Markdown {
    text: Text,
    trusted: bool,
}

impl Markdown {
    /// Markdown from users or other untrusted sources: a string, or a [`Text`] key resolved in the
    /// user's language. At most [`MAX_MARKDOWN`] bytes are rendered.
    pub fn new(text: impl Into<Text>) -> Self {
        Markdown { text: text.into(), trusted: false }
    }

    /// Text the app wrote itself: headings get ids and links lose `nofollow ugc`. Escaping and
    /// the link and image rules stay the same.
    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }

    /// The HTML in English, without the wrapper the component adds.
    pub fn html(&self) -> String {
        self.html_in(&Render::english(""))
    }

    /// The HTML with texts resolved for `r`, without the wrapper.
    pub fn html_in(&self, r: &Render) -> String {
        let source = r.i18n.text(r.locale, &self.text);
        let mut end = source.len().min(MAX_MARKDOWN);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let source = &source[..end];
        let mut r = Renderer { trusted: self.trusted, html: String::with_capacity(source.len() + 64), ids: Vec::new() };
        let lines: Vec<&str> = source.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            i = r.block(&lines, i);
        }
        r.html
    }
}

impl Component for Markdown {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<div class="z-prose z-markdown" data-testid="{}.markdown">"#, r.scope));
        out.push_str(&self.html_in(r));
        out.push_str("</div>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "markdown", "trusted": self.trusted })
    }
}

struct Renderer {
    trusted: bool,
    html: String,
    ids: Vec<String>,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The marker of a list item: ordered or not, and the text after it.
fn list_item(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some((false, rest));
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && digits < 4 {
        if let Some(rest) = t[digits..].strip_prefix(". ") {
            return Some((true, rest));
        }
    }
    None
}

fn is_rule(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*'))
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
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
            '|' if !code => cells.push(std::mem::take(&mut cell).trim().to_string()),
            _ => cell.push(c),
        }
    }
    cells.push(cell.trim().to_string());
    cells
}

/// A heading id: lower case letters and digits joined by `-`.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "section".into() } else { out }
}

/// Appends one character escaped as [`escape`] does, without an allocation.
fn push_escaped(out: &mut String, c: char) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(c),
    }
}

/// Whether a link or image target may be used, and whether it leaves the app.
fn target_kind(target: &str) -> Option<Target> {
    if target.is_empty() || target.chars().any(|c| c.is_control() || c.is_whitespace() || c == '\\') {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Some(Target::External);
    }
    if lower.starts_with("mailto:") {
        return Some(Target::Mail);
    }
    if target.starts_with("//") {
        return None;
    }
    let path_end = target.find(['/', '?', '#']).unwrap_or(target.len());
    if target[..path_end].contains([':', '&']) {
        return None;
    }
    Some(Target::Local)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    External,
    Mail,
    Local,
}

/// Next places of the inline marks, remembered while one span is scanned from left to right so
/// a mark without its closing partner is not searched for again: the scan stays linear.
struct Marks<'a> {
    chars: &'a [char],
    seen: Vec<(&'static [char], usize, Option<usize>)>,
}

impl Marks<'_> {
    fn find(&mut self, from: usize, pat: &'static [char]) -> Option<usize> {
        if let Some((_, start, hit)) = self.seen.iter().find(|(p, ..)| *p == pat) {
            match hit {
                None if *start <= from => return None,
                Some(h) if *h >= from && *start <= from => return Some(*h),
                _ => {}
            }
        }
        let hit = (from..self.chars.len()).find(|&k| self.chars[k..].starts_with(pat));
        self.seen.retain(|(p, ..)| *p != pat);
        self.seen.push((pat, from, hit));
        hit
    }
}

impl Renderer {
    /// Renders the block that starts at line `i` and returns the line after it.
    fn block(&mut self, lines: &[&str], i: usize) -> usize {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            return i + 1;
        }
        if let Some(fence) = t.strip_prefix("```") {
            let lang: String = fence
                .trim()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+'))
                .take(32)
                .collect();
            let mut end = i + 1;
            while end < lines.len() && !lines[end].trim_start().starts_with("```") {
                end += 1;
            }
            let code = lines[i + 1..end].join("\n");
            let class = if lang.is_empty() { String::new() } else { format!(r#" class="language-{lang}""#) };
            self.html
                .push_str(&format!(r#"<pre class="z-code" tabindex="0"><code{class}>{}</code></pre>"#, escape(&code)));
            return (end + 1).min(lines.len());
        }
        let hashes = t.bytes().take_while(|b| *b == b'#').count();
        if (1..=4).contains(&hashes) && t[hashes..].starts_with(' ') {
            let text = t[hashes..].trim();
            let level = (hashes + 1).min(4);
            let body = self.inline(text, 0);
            if self.trusted {
                let mut id = slug(text);
                if self.ids.contains(&id) {
                    let mut n = 2;
                    while self.ids.contains(&format!("{id}-{n}")) {
                        n += 1;
                    }
                    id = format!("{id}-{n}");
                }
                self.html.push_str(&format!(r#"<h{level} id="{id}">{body}</h{level}>"#));
                self.ids.push(id);
            } else {
                self.html.push_str(&format!("<h{level}>{body}</h{level}>"));
            }
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
            return self.list(lines, i, indent_of(line), 0);
        }
        if t.starts_with('>') {
            let mut end = i;
            let mut text = Vec::new();
            while end < lines.len() && lines[end].trim().starts_with('>') {
                text.push(lines[end].trim().trim_start_matches('>').trim());
                end += 1;
            }
            let body = self.inline(&text.join(" "), 0);
            self.html.push_str(&format!("<blockquote><p>{body}</p></blockquote>"));
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
        let body = self.inline(&text.join(" "), 0);
        self.html.push_str(&format!("<p>{body}</p>"));
        end
    }

    fn table(&mut self, lines: &[&str], i: usize) -> usize {
        self.html.push_str("<table><thead><tr>");
        for c in cells(lines[i]) {
            let cell = self.inline(&c, 0);
            self.html.push_str(&format!(r#"<th scope="col">{cell}</th>"#));
        }
        self.html.push_str("</tr></thead><tbody>");
        let mut end = i + 2;
        while end < lines.len() && lines[end].trim().starts_with('|') {
            self.html.push_str("<tr>");
            for c in cells(lines[end]) {
                let cell = self.inline(&c, 0);
                self.html.push_str(&format!("<td>{cell}</td>"));
            }
            self.html.push_str("</tr>");
            end += 1;
        }
        self.html.push_str("</tbody></table>");
        end
    }

    /// A list whose items start at `indent`. Deeper items become nested lists, up to
    /// [`MAX_DEPTH`] (items deeper still start a new list after it); indented lines that are not
    /// items continue the item before them.
    fn list(&mut self, lines: &[&str], i: usize, indent: usize, depth: usize) -> usize {
        let ordered = list_item(lines[i]).map(|(o, _)| o).unwrap_or(false);
        let tag = if ordered { "ol" } else { "ul" };
        self.html.push_str(&format!("<{tag}>"));
        let mut at = i;
        while at < lines.len() {
            let line = lines[at];
            let Some((_, text)) = list_item(line).filter(|(o, _)| indent_of(line) == indent && *o == ordered) else {
                break;
            };
            let mut item = text.trim().to_string();
            at += 1;
            let nests = |l: &str| depth + 1 < MAX_DEPTH && list_item(l).is_some();
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
            let body = self.inline(&item, 0);
            self.html.push_str(&format!("<li>{body}"));
            if at < lines.len() && nests(lines[at]) && indent_of(lines[at]) > indent {
                at = self.list(lines, at, indent_of(lines[at]), depth + 1);
            }
            self.html.push_str("</li>");
            if at < lines.len()
                && lines[at].trim().is_empty()
                && at + 1 < lines.len()
                && list_item(lines[at + 1]).is_some_and(|(o, _)| o == ordered && indent_of(lines[at + 1]) == indent)
            {
                at += 1;
            }
        }
        self.html.push_str(&format!("</{tag}>"));
        at
    }

    /// Inline Markdown: code spans, links, images, bold and italic. Everything else is escaped.
    fn inline(&self, text: &str, depth: usize) -> String {
        let chars: Vec<char> = text.chars().collect();
        if depth >= MAX_DEPTH {
            return escape(text);
        }
        let mut out = String::with_capacity(text.len() + 16);
        let mut marks = Marks { chars: &chars, seen: Vec::new() };
        let collect = |a: usize, b: usize| -> String { chars[a..b].iter().collect() };
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\\' && i + 1 < chars.len() && "\\`*_[]()#|!".contains(chars[i + 1]) {
                push_escaped(&mut out, chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '`' {
                if let Some(end) = marks.find(i + 1, &['`']) {
                    out.push_str(&format!("<code>{}</code>", escape(&collect(i + 1, end))));
                    i = end + 1;
                    continue;
                }
            }
            let image = c == '!' && chars.get(i + 1) == Some(&'[');
            if c == '[' || image {
                let open = if image { i + 1 } else { i };
                if let Some(close) = marks.find(open + 1, &[']', '(']) {
                    if let Some(end) = marks.find(close + 2, &[')']) {
                        let label = collect(open + 1, close);
                        let target = collect(close + 2, end);
                        if image {
                            out.push_str(&self.image(&label, target.trim()));
                        } else {
                            out.push_str(&self.anchor(&self.inline(&label, depth + 1), target.trim()));
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            if c == '*' && chars.get(i + 1) == Some(&'*') {
                if let Some(end) = marks.find(i + 3, &['*', '*']) {
                    out.push_str(&format!("<strong>{}</strong>", self.inline(&collect(i + 2, end), depth + 1)));
                    i = end + 2;
                    continue;
                }
            }
            if c == '*' && chars.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*') {
                if let Some(end) = marks.find(i + 1, &['*']) {
                    out.push_str(&format!("<em>{}</em>", self.inline(&collect(i + 1, end), depth + 1)));
                    i = end + 1;
                    continue;
                }
            }
            push_escaped(&mut out, c);
            i += 1;
        }
        out
    }

    /// A link, or only its label when the target is not allowed.
    fn anchor(&self, label: &str, target: &str) -> String {
        let Some(kind) = target_kind(target) else { return label.to_string() };
        let rel = match (kind, self.trusted) {
            (_, false) => r#" rel="noopener noreferrer nofollow ugc""#,
            (Target::External, true) => r#" rel="noopener noreferrer""#,
            _ => "",
        };
        format!(r#"<a href="{}"{rel}>{label}</a>"#, escape(target))
    }

    /// An image from this app, or its alt text.
    fn image(&self, alt: &str, src: &str) -> String {
        match target_kind(src) {
            Some(Target::Local) if !src.starts_with('#') => {
                format!(r#"<img src="{}" alt="{}" loading="lazy">"#, escape(src), escape(alt))
            }
            _ => escape(alt),
        }
    }
}
