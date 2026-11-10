//! Allow-list sanitizer for SVG and HTML fragments, such as the pages of a
//! [`DocumentViewer`](super::DocumentViewer).
//!
//! The fragment is read by a small tokenizer and written again from scratch: only elements and
//! attributes on the lists below are written, under their canonical names, with every text and
//! attribute value decoded and escaped again. Nothing of the input reaches the output unescaped,
//! so markup the tokenizer reads differently from a browser can drop content but never adds
//! markup. Comments, processing instructions, doctypes and CDATA markers are removed (CDATA text
//! is kept as text).
//!
//! Removed with everything inside them: `script`, `style`, `foreignObject`, `iframe`, `object`,
//! `template`, `noscript`, `math`, form controls, media elements and SVG animation. Elements on
//! no list are unwrapped: their tags go, their text stays. Attributes: `on*` handlers, `data-*`
//! and every name off the list are removed; `class` keeps no `z-` names, so a page cannot borrow
//! the framework's components; `href` keeps only `#fragment` references on `use` and links to
//! paths of the same origin on `a`; images keep only `data:image/png`, `jpeg`, `gif` and `webp`
//! sources, so a page never makes the browser send a request; paint attributes keep only
//! `url(#id)`; `style` keeps a list of layout and paint properties without `url()`, escapes or
//! comments. Ids and every reference to them get a prefix, so pages shown together keep
//! their ids apart.

use crate::render::escape_into;

/// SVG elements a page may hold, by canonical name.
const SVG: &[&str] = &[
    "svg",
    "g",
    "defs",
    "symbol",
    "use",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "textPath",
    "title",
    "desc",
    "image",
    "clipPath",
    "mask",
    "pattern",
    "linearGradient",
    "radialGradient",
    "stop",
    "marker",
    "filter",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "a",
];

/// HTML elements a page may hold.
const HTML: &[&str] = &[
    "div",
    "span",
    "p",
    "br",
    "hr",
    "b",
    "strong",
    "i",
    "em",
    "u",
    "s",
    "small",
    "sub",
    "sup",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "caption",
    "colgroup",
    "col",
    "img",
    "a",
    "figure",
    "figcaption",
    "blockquote",
    "pre",
    "code",
    "section",
    "article",
    "header",
    "footer",
    "mark",
    "abbr",
    "time",
    "title",
];

/// HTML elements without an end tag.
const VOID: &[&str] = &["br", "hr", "img", "col"];

/// HTML elements without an end tag that may stand inside removed content.
const SKIPPED_VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "keygen", "link", "meta", "param",
    "source", "track", "wbr",
];

/// Elements whose text runs to their end tag without markup; removed with it.
const RAW: &[&str] = &[
    "script",
    "style",
    "textarea",
    "xmp",
    "iframe",
    "noembed",
    "noframes",
    "noscript",
    "plaintext",
];

/// Elements removed with everything inside them.
const DROP: &[&str] = &[
    "foreignObject",
    "object",
    "applet",
    "template",
    "math",
    "form",
    "select",
    "button",
    "audio",
    "video",
    "canvas",
    "picture",
    "svg:script",
    "animate",
    "animateMotion",
    "animateTransform",
    "set",
    "feImage",
    "frameset",
    "head",
];

/// How an attribute's value is checked.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Plain,
    Id,
    IdRefs,
    Paint,
    Class,
    Style,
}

/// Attributes any allowed element may carry, by canonical name.
const ATTRS: &[(&str, Kind)] = &[
    ("id", Kind::Id),
    ("class", Kind::Class),
    ("style", Kind::Style),
    ("lang", Kind::Plain),
    ("dir", Kind::Plain),
    ("role", Kind::Plain),
    ("title", Kind::Plain),
    ("alt", Kind::Plain),
    ("width", Kind::Plain),
    ("height", Kind::Plain),
    ("x", Kind::Plain),
    ("y", Kind::Plain),
    ("x1", Kind::Plain),
    ("y1", Kind::Plain),
    ("x2", Kind::Plain),
    ("y2", Kind::Plain),
    ("cx", Kind::Plain),
    ("cy", Kind::Plain),
    ("r", Kind::Plain),
    ("rx", Kind::Plain),
    ("ry", Kind::Plain),
    ("fx", Kind::Plain),
    ("fy", Kind::Plain),
    ("fr", Kind::Plain),
    ("dx", Kind::Plain),
    ("dy", Kind::Plain),
    ("d", Kind::Plain),
    ("points", Kind::Plain),
    ("transform", Kind::Plain),
    ("viewBox", Kind::Plain),
    ("preserveAspectRatio", Kind::Plain),
    ("fill", Kind::Paint),
    ("fill-opacity", Kind::Plain),
    ("fill-rule", Kind::Plain),
    ("stroke", Kind::Paint),
    ("stroke-width", Kind::Plain),
    ("stroke-opacity", Kind::Plain),
    ("stroke-linecap", Kind::Plain),
    ("stroke-linejoin", Kind::Plain),
    ("stroke-dasharray", Kind::Plain),
    ("stroke-dashoffset", Kind::Plain),
    ("stroke-miterlimit", Kind::Plain),
    ("opacity", Kind::Plain),
    ("color", Kind::Plain),
    ("visibility", Kind::Plain),
    ("display", Kind::Plain),
    ("overflow", Kind::Plain),
    ("font-family", Kind::Plain),
    ("font-size", Kind::Plain),
    ("font-weight", Kind::Plain),
    ("font-style", Kind::Plain),
    ("font-variant", Kind::Plain),
    ("text-anchor", Kind::Plain),
    ("text-decoration", Kind::Plain),
    ("dominant-baseline", Kind::Plain),
    ("alignment-baseline", Kind::Plain),
    ("baseline-shift", Kind::Plain),
    ("letter-spacing", Kind::Plain),
    ("word-spacing", Kind::Plain),
    ("writing-mode", Kind::Plain),
    ("rotate", Kind::Plain),
    ("textLength", Kind::Plain),
    ("lengthAdjust", Kind::Plain),
    ("startOffset", Kind::Plain),
    ("xml:space", Kind::Plain),
    ("clip-path", Kind::Paint),
    ("clip-rule", Kind::Plain),
    ("mask", Kind::Paint),
    ("filter", Kind::Paint),
    ("marker-start", Kind::Paint),
    ("marker-mid", Kind::Paint),
    ("marker-end", Kind::Paint),
    ("markerWidth", Kind::Plain),
    ("markerHeight", Kind::Plain),
    ("markerUnits", Kind::Plain),
    ("refX", Kind::Plain),
    ("refY", Kind::Plain),
    ("orient", Kind::Plain),
    ("offset", Kind::Plain),
    ("stop-color", Kind::Plain),
    ("stop-opacity", Kind::Plain),
    ("gradientUnits", Kind::Plain),
    ("gradientTransform", Kind::Plain),
    ("spreadMethod", Kind::Plain),
    ("patternUnits", Kind::Plain),
    ("patternContentUnits", Kind::Plain),
    ("patternTransform", Kind::Plain),
    ("clipPathUnits", Kind::Plain),
    ("maskUnits", Kind::Plain),
    ("maskContentUnits", Kind::Plain),
    ("filterUnits", Kind::Plain),
    ("primitiveUnits", Kind::Plain),
    ("stdDeviation", Kind::Plain),
    ("in", Kind::Plain),
    ("in2", Kind::Plain),
    ("result", Kind::Plain),
    ("mode", Kind::Plain),
    ("operator", Kind::Plain),
    ("k1", Kind::Plain),
    ("k2", Kind::Plain),
    ("k3", Kind::Plain),
    ("k4", Kind::Plain),
    ("values", Kind::Plain),
    ("type", Kind::Plain),
    ("tableValues", Kind::Plain),
    ("slope", Kind::Plain),
    ("intercept", Kind::Plain),
    ("amplitude", Kind::Plain),
    ("exponent", Kind::Plain),
    ("radius", Kind::Plain),
    ("flood-color", Kind::Plain),
    ("flood-opacity", Kind::Plain),
    ("vector-effect", Kind::Plain),
    ("shape-rendering", Kind::Plain),
    ("text-rendering", Kind::Plain),
    ("image-rendering", Kind::Plain),
    ("paint-order", Kind::Plain),
    ("colspan", Kind::Plain),
    ("rowspan", Kind::Plain),
    ("scope", Kind::Plain),
    ("headers", Kind::IdRefs),
    ("start", Kind::Plain),
    ("reversed", Kind::Plain),
    ("datetime", Kind::Plain),
    ("abbr", Kind::Plain),
    ("align", Kind::Plain),
    ("valign", Kind::Plain),
    ("span", Kind::Plain),
];

/// `aria-*` attributes that name other elements by id.
const ARIA_REFS: &[&str] = &[
    "aria-labelledby",
    "aria-describedby",
    "aria-controls",
    "aria-owns",
    "aria-details",
    "aria-flowto",
    "aria-errormessage",
    "aria-activedescendant",
];

/// CSS properties a `style` attribute keeps.
const STYLE: &[&str] = &[
    "fill",
    "fill-opacity",
    "fill-rule",
    "stroke",
    "stroke-width",
    "stroke-opacity",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-miterlimit",
    "opacity",
    "color",
    "background-color",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "font-variant",
    "font-stretch",
    "letter-spacing",
    "word-spacing",
    "line-height",
    "text-align",
    "text-anchor",
    "text-decoration",
    "text-transform",
    "text-indent",
    "dominant-baseline",
    "white-space",
    "vertical-align",
    "display",
    "visibility",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-width",
    "border-style",
    "border-color",
    "border-collapse",
    "border-spacing",
    "border-radius",
    "position",
    "top",
    "right",
    "bottom",
    "left",
    "z-index",
    "overflow",
    "box-sizing",
    "flex",
    "flex-direction",
    "flex-wrap",
    "justify-content",
    "align-items",
    "gap",
    "stop-color",
    "stop-opacity",
    "transform",
    "transform-origin",
];

/// Raster images a page may carry inline.
const IMAGE_TYPES: &[&str] = &[
    "data:image/png;",
    "data:image/jpeg;",
    "data:image/jpg;",
    "data:image/gif;",
    "data:image/webp;",
];

/// Deepest nesting of elements kept; deeper tags are dropped, their text kept.
const MAX_DEPTH: usize = 256;

/// Most `use` elements one fragment keeps; each can repeat a whole subtree when the browser
/// renders it, so later ones are removed.
pub const MAX_USE_ELEMENTS: usize = 2_000;

/// Most attributes read on one element; later ones are removed.
const MAX_ATTRS: usize = 64;

/// Longest id kept, in bytes.
const MAX_ID: usize = 100;

/// Cleans an SVG or HTML fragment with the allow-list of this module and returns it ready to
/// insert into a page.
///
/// Every id and every reference to one (`url(#id)`, `href="#id"`, `aria-labelledby`) gets
/// `id_prefix` in front. Without a prefix, a fragment could name its elements like globals of the
/// page's scripts or like other elements of the page (DOM clobbering: `id="zeynCsrf"` becomes
/// `window.zeynCsrf`), and two fragments on one page would share ids. Use a prefix unique on the
/// page that contains a hyphen, such as `doc-3-p12-`: ids with a hyphen are never names of
/// globals. Characters other than ASCII letters, digits, `-` and `_` are removed from it.
///
/// The work is linear in the size of the fragment; clean large fragments off the async runtime
/// (`tokio::task::spawn_blocking`), as `DocumentViewer` does.
///
/// ```
/// let clean = zeyn::ui::sanitize_markup(
///     r#"<svg viewBox="0 0 10 10" onload="x()"><script>x()</script><rect id="r" width="4" height="4"/></svg>"#,
///     "a-",
/// );
/// assert_eq!(clean, r#"<svg viewBox="0 0 10 10"><rect id="a-r" width="4" height="4"/></svg>"#);
/// ```
pub fn sanitize_markup(html: &str, id_prefix: &str) -> String {
    let prefix: String = id_prefix
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(MAX_ID)
        .collect();
    clean(html, &prefix)
}

/// Cleans a fragment, with `prefix` in front of every id and id reference.
pub(crate) fn clean(html: &str, prefix: &str) -> String {
    let mut s = Sanitizer {
        src: html,
        at: 0,
        out: String::with_capacity(html.len()),
        open: Vec::new(),
        svg: 0,
        uses: 0,
        prefix,
    };
    s.run();
    s.out
}

/// An element as the tokenizer read it.
struct Tag<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, Option<&'a str>)>,
    closed: bool,
}

struct Sanitizer<'a, 'p> {
    src: &'a str,
    at: usize,
    out: String,
    open: Vec<&'static str>,
    svg: usize,
    uses: usize,
    prefix: &'p str,
}

/// Flags of an element name: the lists it is on.
const F_RAW: u8 = 1;
const F_DROP: u8 = 2;
const F_SVG: u8 = 4;
const F_HTML: u8 = 8;
const F_VOID: u8 = 16;
const F_SKIPPED_VOID: u8 = 32;

/// The canonical name and the flags of an element name, compared without case through a table
/// built once, so each tag costs one hash lookup.
fn element(name: &str) -> Option<(&'static str, u8)> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<String, (&'static str, u8)>> =
        std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t: std::collections::HashMap<String, (&'static str, u8)> =
            std::collections::HashMap::new();
        let lists: [(&[&'static str], u8); 6] = [
            (RAW, F_RAW),
            (DROP, F_DROP),
            (SVG, F_SVG),
            (HTML, F_HTML),
            (VOID, F_VOID),
            (SKIPPED_VOID, F_SKIPPED_VOID),
        ];
        for (list, flag) in lists {
            for n in list {
                t.entry(n.to_ascii_lowercase())
                    .or_insert((n, 0))
                    .1 |= flag;
            }
        }
        t
    });
    if name.len() > 32 {
        return None;
    }
    let mut buf = [0u8; 32];
    let lower = &mut buf[..name.len()];
    lower.copy_from_slice(name.as_bytes());
    lower.make_ascii_lowercase();
    std::str::from_utf8(lower)
        .ok()
        .and_then(|l| table.get(l).copied())
}

/// Whether element `name` has `flag`.
fn has(name: &str, flag: u8) -> bool {
    element(name).is_some_and(|(_, f)| f & flag != 0)
}

fn name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.')
}

impl<'a> Sanitizer<'a, '_> {
    fn rest(&self) -> &'a str {
        &self.src[self.at..]
    }

    fn run(&mut self) {
        while self.at < self.src.len() {
            let rest = self.rest();
            let Some(lt) = rest.find('<') else {
                let text = rest;
                self.text(text);
                self.at = self.src.len();
                break;
            };
            if lt > 0 {
                let text = &rest[..lt];
                self.text(text);
                self.at += lt;
            }
            self.markup();
        }
        while let Some(name) = self.open.pop() {
            self.close(name);
        }
    }

    /// Reads what starts at a `<`.
    fn markup(&mut self) {
        let rest = self.rest();
        if let Some(body) = rest.strip_prefix("<!--") {
            self.at += 4 + body
                .find("-->")
                .map_or(body.len(), |i| i + 3);
            return;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").unwrap_or(body.len());
            let text = &body[..end];
            self.text_raw(text);
            self.at += 9 + (end + 3).min(body.len());
            return;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            self.at += rest
                .find('>')
                .map_or(rest.len(), |i| i + 1);
            return;
        }
        let bytes = rest.as_bytes();
        if bytes.get(1) == Some(&b'/') {
            let n = bytes[2..]
                .iter()
                .take_while(|b| name_char(**b))
                .count();
            let name = &rest[2..2 + n];
            self.at += rest
                .find('>')
                .map_or(rest.len(), |i| i + 1);
            if n > 0 {
                self.end(name);
            }
            return;
        }
        if !bytes
            .get(1)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            self.out.push_str("&lt;");
            self.at += 1;
            return;
        }
        match self.tag() {
            Some(tag) => self.start(tag),
            None => self.at = self.src.len(),
        }
    }

    /// Reads a start tag at `<`, moving past it. `None` when the input ends inside it.
    fn tag(&mut self) -> Option<Tag<'a>> {
        let src: &'a str = self.src;
        let b = src.as_bytes();
        let mut i = self.at + 1;
        let start = i;
        while i < b.len() && name_char(b[i]) {
            i += 1;
        }
        let name = &src[start..i];
        let mut attrs = Vec::new();
        loop {
            while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
                i += 1;
            }
            if i >= b.len() {
                return None;
            }
            if b[i] == b'>' {
                let closed = b[i - 1] == b'/';
                self.at = i + 1;
                return Some(Tag {
                    name,
                    attrs,
                    closed,
                });
            }
            let a = i;
            while i < b.len() && !matches!(b[i], b'=' | b'>' | b'/') && !b[i].is_ascii_whitespace()
            {
                i += 1;
            }
            let attr = &src[a..i];
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && b[i] == b'=' {
                i += 1;
                while i < b.len() && b[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= b.len() {
                    return None;
                }
                let value = if b[i] == b'"' || b[i] == b'\'' {
                    let q = b[i];
                    let v = i + 1;
                    let end = v + b[v..].iter().position(|c| *c == q)?;
                    i = end + 1;
                    &src[v..end]
                } else {
                    let v = i;
                    while i < b.len() && b[i] != b'>' && !b[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    &src[v..i]
                };
                attrs.push((attr, Some(value)));
            } else if !attr.is_empty() {
                attrs.push((attr, None));
            }
        }
    }

    /// Moves past the end of an element that is removed with its content.
    fn skip(&mut self, name: &str, raw: bool) {
        if raw {
            let lower = name.to_ascii_lowercase();
            let rest = self.rest();
            let mut from = 0;
            let found = loop {
                let Some(i) = rest[from..].find("</") else {
                    break None;
                };
                let at = from + i;
                let after = &rest[at + 2..];
                if after.len() >= lower.len()
                    && after.as_bytes()[..lower.len()].eq_ignore_ascii_case(lower.as_bytes())
                    && !after
                        .as_bytes()
                        .get(lower.len())
                        .is_some_and(|b| name_char(*b))
                {
                    break Some(at);
                }
                from = at + 2;
            };
            let Some(at) = found else {
                self.at = self.src.len();
                return;
            };
            let rest = &self.rest()[at..];
            self.at += at
                + rest
                    .find('>')
                    .map_or(rest.len(), |i| i + 1);
            return;
        }
        let mut depth = 1usize;
        while depth > 0 && self.at < self.src.len() {
            let rest = self.rest();
            let Some(lt) = rest.find('<') else {
                self.at = self.src.len();
                return;
            };
            self.at += lt;
            let rest = self.rest();
            if let Some(body) = rest.strip_prefix("<!--") {
                self.at += 4 + body
                    .find("-->")
                    .map_or(body.len(), |i| i + 3);
            } else if rest.starts_with("</") {
                depth -= 1;
                self.at += rest
                    .find('>')
                    .map_or(rest.len(), |i| i + 1);
            } else if rest
                .as_bytes()
                .get(1)
                .is_some_and(u8::is_ascii_alphabetic)
            {
                match self.tag() {
                    Some(t) => {
                        if has(t.name, F_RAW) && !t.closed {
                            let n = t.name.to_string();
                            self.skip(&n, true);
                        } else if !t.closed && !has(t.name, F_SKIPPED_VOID) {
                            depth += 1;
                        }
                    }
                    None => self.at = self.src.len(),
                }
            } else {
                self.at += 1;
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        let Some((name, flags)) = element(tag.name) else {
            return;
        };
        if flags & F_RAW != 0 {
            if !tag.closed {
                self.skip(name, true);
            }
            return;
        }
        if flags & F_DROP != 0 {
            if !tag.closed {
                self.skip(name, false);
            }
            return;
        }
        let in_svg = self.svg > 0;
        if flags & (F_SVG | F_HTML) == 0 || (in_svg && flags & F_SVG == 0) {
            return;
        }
        if name == "use" {
            if self.uses >= MAX_USE_ELEMENTS {
                if !tag.closed {
                    self.skip(name, false);
                }
                return;
            }
            self.uses += 1;
        }
        let svg = in_svg || (name == "svg") || flags & F_HTML == 0;
        if self.open.len() >= MAX_DEPTH {
            return;
        }
        self.out.push('<');
        self.out.push_str(name);
        let mut seen: Vec<&str> = Vec::new();
        for (attr, value) in tag.attrs.iter().take(MAX_ATTRS) {
            if let Some((canonical, value)) = self.attr(name, attr, *value)
                && !seen.contains(&canonical)
            {
                seen.push(canonical);
                self.out.push(' ');
                self.out.push_str(canonical);
                self.out.push_str("=\"");
                escape_into(&mut self.out, &value);
                self.out.push('"');
            }
        }
        let void = !svg && flags & F_VOID != 0;
        if void {
            self.out.push('>');
        } else if tag.closed && svg {
            self.out.push_str("/>");
        } else if tag.closed {
            self.out.push('>');
            self.close(name);
        } else {
            self.out.push('>');
            if name == "svg" {
                self.svg += 1;
            }
            self.open.push(name);
        }
    }

    fn end(&mut self, name: &str) {
        let Some(i) = self
            .open
            .iter()
            .rposition(|n| n.eq_ignore_ascii_case(name))
        else {
            return;
        };
        while self.open.len() > i {
            if let Some(n) = self.open.pop() {
                self.close(n);
            }
        }
    }

    fn close(&mut self, name: &'static str) {
        if name == "svg" {
            self.svg = self.svg.saturating_sub(1);
        }
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
    }

    /// Text with character references, decoded and escaped again.
    fn text(&mut self, text: &str) {
        let decoded = decode(text);
        escape_into(&mut self.out, &decoded);
    }

    /// Text without character references, such as CDATA.
    fn text_raw(&mut self, text: &str) {
        escape_into(&mut self.out, text);
    }

    /// The attribute as it is written, or `None` when it is removed.
    fn attr(
        &self,
        element: &str,
        name: &str,
        value: Option<&str>,
    ) -> Option<(&'static str, String)> {
        if name.len() > 32 {
            return None;
        }
        let lower = name.to_ascii_lowercase();
        let value = decode(value.unwrap_or(""));
        if lower == "href" || lower == "xlink:href" {
            return match element {
                "use" => self.local(&value).map(|v| ("href", v)),
                "a" => self.link(&value).map(|v| ("href", v)),
                "image" => image(&value).map(|v| ("href", v)),
                _ => None,
            };
        }
        if lower == "src" {
            return (element == "img")
                .then(|| image(&value))
                .flatten()
                .map(|v| ("src", v));
        }
        if lower.starts_with("aria-") {
            let canonical = ARIA
                .iter()
                .copied()
                .find(|a| *a == lower)?;
            return match ARIA_REFS.contains(&canonical) {
                true => self
                    .refs(&value)
                    .map(|v| (canonical, v)),
                false => Some((canonical, value)),
            };
        }
        let (canonical, kind) = attr_table()
            .get(lower.as_str())
            .copied()?;
        let value = match kind {
            Kind::Plain => Some(value),
            Kind::Id => self.id(&value),
            Kind::IdRefs => self.refs(&value),
            Kind::Paint => self.paint(&value),
            Kind::Class => class(&value),
            Kind::Style => self.style(&value),
        }?;
        Some((canonical, value))
    }

    fn id(&self, value: &str) -> Option<String> {
        let v = value.trim();
        (!v.is_empty()
            && v.len() <= MAX_ID
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':')))
        .then(|| format!("{}{v}", self.prefix))
    }

    fn refs(&self, value: &str) -> Option<String> {
        let ids: Option<Vec<String>> = value
            .split_ascii_whitespace()
            .map(|v| self.id(v))
            .collect();
        ids.filter(|i| !i.is_empty())
            .map(|i| i.join(" "))
    }

    /// A reference to an element of the same fragment: `#id`.
    fn local(&self, value: &str) -> Option<String> {
        let id = value.trim().strip_prefix('#')?;
        self.id(id).map(|v| format!("#{v}"))
    }

    /// A link target: a fragment of the page, or a path of the same origin.
    fn link(&self, value: &str) -> Option<String> {
        if value.trim().starts_with('#') {
            return self.local(value);
        }
        let v: String = value
            .chars()
            .filter(|c| !c.is_ascii_whitespace() && !c.is_control())
            .collect();
        let head = v
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("");
        let relative =
            !v.is_empty() && !head.contains(':') && !v.starts_with("//") && !v.contains('\\');
        relative.then_some(v)
    }

    /// Paint and effect values: `none`, a color, or `url(#id)` with an optional color after it.
    /// Anything else, such as `image-set()` or a second `url()`, removes the attribute.
    fn paint(&self, value: &str) -> Option<String> {
        let v = value.trim();
        let Some(inner) = v
            .get(..4)
            .filter(|h| h.eq_ignore_ascii_case("url("))
            .map(|_| &v[4..])
        else {
            return color(v).then(|| v.to_string());
        };
        let end = inner.find(')')?;
        let target = inner[..end]
            .trim()
            .trim_matches(['"', '\'']);
        let local = self.local(target)?;
        let fallback = inner[end + 1..].trim();
        if !fallback.is_empty() && !color(fallback) {
            return None;
        }
        Some(match fallback.is_empty() {
            true => format!("url({local})"),
            false => format!("url({local}) {fallback}"),
        })
    }

    /// Declarations of a `style` attribute on the list, without `url()`, escapes or comments.
    fn style(&self, value: &str) -> Option<String> {
        let mut out = String::new();
        for decl in value.split(';') {
            let Some((prop, v)) = decl.split_once(':') else {
                continue;
            };
            let prop = prop.trim().to_ascii_lowercase();
            let v = v.trim();
            let lower = v.to_ascii_lowercase();
            let bad = ["url(", "expression", "image", "\\", "/*", "@", "<", "var("]
                .iter()
                .any(|b| lower.contains(b));
            if v.is_empty() || bad || !STYLE.contains(&prop.as_str()) {
                continue;
            }
            if prop == "position" && !matches!(lower.as_str(), "static" | "relative" | "absolute") {
                continue;
            }
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(&prop);
            out.push(':');
            out.push_str(v);
        }
        (!out.is_empty()).then_some(out)
    }
}

/// `aria-*` attributes kept, besides those in [`ARIA_REFS`].
const ARIA: &[&str] = &[
    "aria-label",
    "aria-labelledby",
    "aria-describedby",
    "aria-controls",
    "aria-owns",
    "aria-details",
    "aria-flowto",
    "aria-errormessage",
    "aria-activedescendant",
    "aria-hidden",
    "aria-level",
    "aria-roledescription",
    "aria-colcount",
    "aria-colindex",
    "aria-colspan",
    "aria-rowcount",
    "aria-rowindex",
    "aria-rowspan",
    "aria-sort",
    "aria-current",
    "aria-posinset",
    "aria-setsize",
    "aria-description",
];

/// Whether `v` is a paint keyword or a color: `none`, `currentColor`, `transparent`, a color
/// name, `#rgb` to `#rrggbbaa`, or `rgb()`, `rgba()`, `hsl()`, `hsla()` of numbers.
fn color(v: &str) -> bool {
    if let Some(hex) = v.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8)
            && hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit());
    }
    if !v.is_empty()
        && v.len() <= 32
        && v.bytes()
            .all(|b| b.is_ascii_alphabetic())
    {
        return true;
    }
    let Some((func, args)) = v.split_once('(') else {
        return false;
    };
    let func = func.trim().to_ascii_lowercase();
    let Some(args) = args.strip_suffix(')') else {
        return false;
    };
    matches!(func.as_str(), "rgb" | "rgba" | "hsl" | "hsla")
        && args.len() <= 64
        && args.bytes().all(|b| {
            b.is_ascii_digit()
                || matches!(
                    b,
                    b' ' | b'.'
                        | b','
                        | b'%'
                        | b'/'
                        | b'-'
                        | b'+'
                        | b'd'
                        | b'e'
                        | b'g'
                        | b't'
                        | b'u'
                        | b'r'
                        | b'n'
                )
        })
}

/// [`ATTRS`] by lowercase name, built once.
fn attr_table() -> &'static std::collections::HashMap<String, (&'static str, Kind)> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<String, (&'static str, Kind)>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        ATTRS
            .iter()
            .map(|(n, k)| (n.to_ascii_lowercase(), (*n, *k)))
            .collect()
    })
}

/// Class names without the framework's `z-` names.
fn class(value: &str) -> Option<String> {
    let kept: Vec<&str> = value
        .split_ascii_whitespace()
        .filter(|c| !c.to_ascii_lowercase().starts_with("z-"))
        .collect();
    (!kept.is_empty()).then(|| kept.join(" "))
}

/// An inline raster image: `data:image/png`, `jpeg`, `gif` or `webp` in base64.
fn image(value: &str) -> Option<String> {
    let v: String = value
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let lower = v
        .get(..20)
        .unwrap_or(&v)
        .to_ascii_lowercase();
    let typed = IMAGE_TYPES
        .iter()
        .find(|t| lower.starts_with(*t))?;
    let data = v[typed.len()..].strip_prefix("base64,")?;
    data.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        .then_some(v)
}

/// Decodes the character references of HTML and XML that pages use: the five named ones of XML,
/// `&nbsp;` and numeric ones. Others stay as written.
fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest
            .bytes()
            .take(12)
            .position(|b| b == b';');
        let c = end.and_then(|e| entity(&rest[1..e]));
        match (c, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    (!c.is_control() || matches!(c, '\t' | '\n' | '\r')).then_some(c)
}
