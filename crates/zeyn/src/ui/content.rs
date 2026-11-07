//! Content components for reading pages: [`Heading`], [`Paragraph`], [`MetaLine`],
//! [`PrevNext`], [`Avatar`], [`Spoiler`] and the [`AutoGrid`] layout. They are plain server
//! rendered HTML with no browser script; their styles live in `content.css`.

use super::layout::into_node;
use super::render_child;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(
    Heading, Paragraph, MetaLine, PrevNext, AutoGrid, Avatar, Spoiler
);

/// A section heading with an explicit level, `h2` to `h6`. The screen title is the page's only
/// `h1`, so there is no `h1` here; pick the level that follows the outline instead of the size
/// you want.
///
/// ```
/// use zeyn::ui::Heading;
/// let h = Heading::h2("Recent articles");
/// let sub = Heading::h3("This week").id("this-week");
/// ```
///
/// The screen title owns `h1`:
///
/// ```compile_fail
/// let h = zeyn::ui::Heading::h1("Title");
/// ```
pub struct Heading {
    level: u8,
    text: Text,
    id: Option<String>,
}

impl Heading {
    fn level(level: u8, text: impl Into<Text>) -> Self {
        Heading {
            level,
            text: text.into(),
            id: None,
        }
    }

    /// A heading of a section of the screen.
    pub fn h2(text: impl Into<Text>) -> Self {
        Self::level(2, text)
    }

    /// A heading inside an `h2` section.
    pub fn h3(text: impl Into<Text>) -> Self {
        Self::level(3, text)
    }

    pub fn h4(text: impl Into<Text>) -> Self {
        Self::level(4, text)
    }

    pub fn h5(text: impl Into<Text>) -> Self {
        Self::level(5, text)
    }

    pub fn h6(text: impl Into<Text>) -> Self {
        Self::level(6, text)
    }

    /// An anchor so links can point at the heading (`#id`). Letters, digits, `-` and `_` are
    /// kept; anything else is dropped.
    pub fn id(mut self, id: &str) -> Self {
        let clean: String = id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(64)
            .collect();
        self.id = (!clean.is_empty()).then_some(clean);
        self
    }
}

impl Component for Heading {
    fn render(&self, out: &mut String, r: &Render) {
        let id = self
            .id
            .as_ref()
            .map(|i| format!(r#" id="{i}""#))
            .unwrap_or_default();
        out.push_str(&format!(
            r#"<h{0} class="z-heading z-heading-{0}"{id}>{1}</h{0}>"#,
            self.level,
            r.text(&self.text)
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "heading", "level": self.level, "text": self.text, "id": self.id })
    }
}

/// A paragraph of text, escaped. Long words wrap. `.muted()` and `.small()` make it a quieter
/// note.
///
/// ```
/// use zeyn::ui::Paragraph;
/// let intro = Paragraph::new("Every product ships within two days.");
/// let note = Paragraph::new("Prices include tax.").muted().small();
/// ```
pub struct Paragraph {
    text: Text,
    muted: bool,
    small: bool,
}

impl Paragraph {
    pub fn new(text: impl Into<Text>) -> Self {
        Paragraph {
            text: text.into(),
            muted: false,
            small: false,
        }
    }

    /// Secondary text color.
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// The small font size.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
}

impl Component for Paragraph {
    fn render(&self, out: &mut String, r: &Render) {
        let mut class = String::from("z-paragraph");
        if self.muted {
            class.push_str(" z-muted");
        }
        if self.small {
            class.push_str(" z-small");
        }
        out.push_str(&format!(r#"<p class="{class}">{}</p>"#, r.text(&self.text)));
    }

    fn schema(&self) -> Json {
        json!({ "type": "paragraph", "text": self.text, "muted": self.muted, "small": self.small })
    }
}

/// Short facts on one line, such as a date, a reading time and a category, joined by a
/// separator. Screen readers skip the separator and hear the parts one after the other.
///
/// ```
/// use zeyn::ui::MetaLine;
/// let meta = MetaLine::new().part("12 May 2026").part("5 min read").part("Guides");
/// let slash = MetaLine::new().part("Shop").part("Tea").separator("/");
/// ```
pub struct MetaLine {
    parts: Vec<Text>,
    separator: &'static str,
}

impl Default for MetaLine {
    fn default() -> Self {
        MetaLine {
            parts: Vec::new(),
            separator: "\u{00b7}",
        }
    }
}

impl MetaLine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a part. Empty parts are left out when rendering.
    pub fn part(mut self, text: impl Into<Text>) -> Self {
        self.parts.push(text.into());
        self
    }

    /// The separator shown between parts. A middle dot unless set.
    pub fn separator(mut self, separator: &'static str) -> Self {
        self.separator = separator;
        self
    }
}

impl Component for MetaLine {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(r#"<p class="z-meta">"#);
        let sep = crate::render::escape(self.separator);
        let mut first = true;
        for part in &self.parts {
            let text = r.text(part);
            if text.is_empty() {
                continue;
            }
            if !first {
                out.push_str(&format!(
                    r#"<span class="z-meta-sep" aria-hidden="true">{sep}</span>"#
                ));
            }
            first = false;
            out.push_str(&format!(r#"<bdi class="z-meta-part">{text}</bdi>"#));
        }
        out.push_str("</p>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "meta_line", "parts": self.parts, "separator": self.separator })
    }
}

struct Step {
    screen: &'static str,
    params: Json,
    title: Option<Text>,
    shortcut: Option<String>,
}

/// A pair of links to the previous and the next item, such as the article before and after this
/// one. The links get `rel="prev"` and `rel="next"`, open without a page reload, and their arrows
/// turn around in right to left languages. Either side may be missing.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::PrevNext;
///
/// #[params]
/// pub struct ArticleParams {
///     pub id: i64,
/// }
///
/// #[screen(route = "/articles/{id}", public)]
/// pub fn article(p: ArticleParams) -> Screen {
///     Screen::new("Article").body(
///         PrevNext::new()
///             .prev(article, ArticleParams { id: p.id - 1 })
///             .prev_title("Getting started")
///             .shortcut("Alt+Shift+B")
///             .next(article, ArticleParams { id: p.id + 1 })
///             .shortcut("Alt+Shift+N"),
///     )
/// }
/// ```
#[derive(Default)]
pub struct PrevNext {
    prev: Option<Step>,
    next: Option<Step>,
    next_last: bool,
}

impl PrevNext {
    pub fn new() -> Self {
        Self::default()
    }

    /// The previous item. Params of the wrong type do not compile.
    pub fn prev<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.prev = Some(Step {
            screen: S::META.name,
            params: serde_json::to_value(params).unwrap_or(Json::Null),
            title: None,
            shortcut: None,
        });
        self.next_last = false;
        self
    }

    /// The next item. Params of the wrong type do not compile.
    pub fn next<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.next = Some(Step {
            screen: S::META.name,
            params: serde_json::to_value(params).unwrap_or(Json::Null),
            title: None,
            shortcut: None,
        });
        self.next_last = true;
        self
    }

    /// The title of the previous item, shown under the word "Previous".
    pub fn prev_title(mut self, title: impl Into<Text>) -> Self {
        if let Some(s) = &mut self.prev {
            s.title = Some(title.into());
        }
        self
    }

    /// The title of the next item, shown under the word "Next".
    pub fn next_title(mut self, title: impl Into<Text>) -> Self {
        if let Some(s) = &mut self.next {
            s.title = Some(title.into());
        }
        self
    }

    /// A keyboard shortcut such as `"Alt+Shift+N"` for the side set last (`prev` or
    /// `next`). It works like [`super::Chips::shortcut`]: pressing it follows the link through
    /// `nav.js` unless focus is in a text field, and the link carries `aria-keyshortcuts` and a
    /// `<kbd>` hint.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        let side = if self.next_last {
            &mut self.next
        } else {
            &mut self.prev
        };
        if let Some(s) = side {
            s.shortcut = Some(keys.into());
        }
        self
    }
}

impl Component for PrevNext {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.prevnext.{key}")));
        out.push_str(&format!(
            r#"<nav class="z-prevnext" aria-label="{}">"#,
            t("label")
        ));
        for (step, rel, arrow) in [
            (&self.prev, "prev", "&#8592;"),
            (&self.next, "next", "&#8594;"),
        ] {
            let Some(s) = step else {
                out.push_str(r#"<span class="z-prevnext-gap"></span>"#);
                continue;
            };
            let word = t(if rel == "prev" { "previous" } else { "next" });
            let title = s
                .title
                .as_ref()
                .map(|title| format!(r#"<span class="z-prevnext-title">{}</span>"#, r.text(title)))
                .unwrap_or_default();
            let (keys, hint) = crate::command::link_shortcut(s.shortcut.as_deref());
            out.push_str(&format!(
                r#"<a class="z-prevnext-link z-prevnext-{rel}" href="{}" rel="{rel}" data-zeyn-base-href{keys}><span class="z-prevnext-arrow" aria-hidden="true">{arrow}</span><span class="z-prevnext-text"><span class="z-prevnext-word">{word}{hint}</span>{title}</span></a>"#,
                screen_href(r, s.screen, &s.params)
            ));
        }
        out.push_str("</nav>");
    }

    fn schema(&self) -> Json {
        let side = |s: &Option<Step>| {
            s.as_ref()
                .map(|s| json!({ "screen": s.screen, "params": s.params, "title": s.title, "shortcut": s.shortcut }))
        };
        json!({ "type": "prev_next", "prev": side(&self.prev), "next": side(&self.next) })
    }

    fn shortcuts(&self) -> Vec<&str> {
        [&self.prev, &self.next]
            .into_iter()
            .flatten()
            .filter_map(|s| s.shortcut.as_deref())
            .collect()
    }
}

/// Children in a grid whose number of columns follows the available width: as many columns of
/// at least `min` pixels as fit, sharing the rest of the width, and one column when even one
/// does not fit. The browser does the layout (`repeat(auto-fill, minmax(min(Npx, 100%), 1fr))`),
/// so it needs no script and no breakpoints. Not to be mixed up with a DataGrid, which shows
/// rows of data.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::AutoGrid;
/// let tiles = AutoGrid::new()
///     .min(220)
///     .add(Card::new().title("Orders"))
///     .add(Card::new().title("Stock"))
///     .add(Card::new().title("People"));
/// ```
pub struct AutoGrid {
    nodes: Vec<Node>,
    min: u16,
    gap: u8,
}

impl Default for AutoGrid {
    fn default() -> Self {
        AutoGrid {
            nodes: Vec::new(),
            min: 240,
            gap: 4,
        }
    }
}

impl AutoGrid {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }

    /// The narrowest a column may get, in CSS pixels, 80 to 1200. 240 unless set.
    pub fn min(mut self, px: u16) -> Self {
        self.min = px.clamp(80, 1200);
        self
    }

    /// Gap in spacing units (4 px at regular density), 0 to 12. 4 unless set.
    pub fn gap(mut self, units: u8) -> Self {
        self.gap = units.min(12);
        self
    }
}

impl Component for AutoGrid {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-autogrid" style="--z-autogrid-min:{}px;--z-autogrid-gap:{}">"#,
            self.min, self.gap
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "auto_grid",
            "min": self.min,
            "gap": self.gap,
            "children": self.nodes.iter().map(Node::schema).collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

/// How large an [`Avatar`] is: 32, 40 or 64 CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AvatarSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl AvatarSize {
    fn px(self) -> u32 {
        match self {
            AvatarSize::Small => 32,
            AvatarSize::Medium => 40,
            AvatarSize::Large => 64,
        }
    }
}

/// A round picture of a person or a team, or their initials when there is no picture. The name
/// is required: it is the image's alternative text and the source of the initials. When the
/// name is written right next to the avatar, call `.decorative()` so screen readers do not hear
/// it twice.
///
/// The picture follows the [`super::Image`] rules: a path of the app or an `https` address on a
/// host of the app's `img-src`; any other picture is left out and the initials are shown.
///
/// ```
/// use zeyn::ui::{Avatar, AvatarSize};
/// let ada = Avatar::new("Ada Lovelace").image("/img/people/ada.jpg").size(AvatarSize::Large);
/// let team = Avatar::new("Support team");
/// let beside_name = Avatar::new("Alan Turing").size(AvatarSize::Small).decorative();
/// ```
pub struct Avatar {
    name: Text,
    image: Option<String>,
    size: AvatarSize,
    decorative: bool,
}

impl Avatar {
    pub fn new(name: impl Into<Text>) -> Self {
        Avatar {
            name: name.into(),
            image: None,
            size: AvatarSize::Medium,
            decorative: false,
        }
    }

    /// The picture. Without one, or when it is not allowed, the initials are shown.
    pub fn image(mut self, src: impl Into<String>) -> Self {
        self.image = Some(src.into());
        self
    }

    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }

    /// Hides the avatar from screen readers, for when the name is written next to it.
    pub fn decorative(mut self) -> Self {
        self.decorative = true;
        self
    }
}

/// Up to two initials of a name, in capitals of the locale: the first letter of the first word
/// and of the last word. Turkish `i` becomes `İ`.
pub(crate) fn initials(name: &str, locale: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let firsts = match words.as_slice() {
        [] => Vec::new(),
        [one] => vec![one],
        [first, .., last] => vec![first, last],
    };
    let turkish = locale.starts_with("tr") || locale.starts_with("az");
    firsts
        .iter()
        .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
        .flat_map(|c| match c {
            'i' if turkish => "İ".chars().collect::<Vec<_>>(),
            'ı' if turkish => vec!['I'],
            c => c.to_uppercase().collect(),
        })
        .collect()
}

impl Component for Avatar {
    fn render(&self, out: &mut String, r: &Render) {
        let px = self.size.px();
        let name = r.i18n.text(r.locale, &self.name);
        let tone = name.bytes().fold(0u32, |h, b| {
            h.wrapping_mul(31)
                .wrapping_add(u32::from(b))
        }) % 5;
        let class = format!("z-avatar z-avatar-{} z-avatar-tone-{tone}", px);
        let hidden = if self.decorative {
            r#" aria-hidden="true""#
        } else {
            ""
        };
        let image = self
            .image
            .as_deref()
            .filter(|src| super::embed::policy(r).allows_image(src));
        match image {
            Some(src) => out.push_str(&format!(
                r#"<img class="{class}" src="{}" alt="{}" width="{px}" height="{px}" loading="lazy" decoding="async"{hidden}>"#,
                escape(src),
                if self.decorative { String::new() } else { escape(&name) }
            )),
            None => {
                let label = if self.decorative { String::new() } else { format!(r#" role="img" aria-label="{}""#, escape(&name)) };
                out.push_str(&format!(
                    r#"<span class="{class}"{label}{hidden}><span aria-hidden="true">{}</span></span>"#,
                    escape(&initials(&name, r.locale))
                ));
            }
        }
    }

    fn schema(&self) -> Json {
        json!({ "type": "avatar", "name": self.name, "image": self.image, "size": self.size, "decorative": self.decorative })
    }
}

/// Content hidden until the reader opens it, such as the answer to a quiz or the end of a
/// story. It is a native disclosure (`<details>` and `<summary>`): the label is a button for
/// keyboards and screen readers, and the content stays out of the reading order until it is
/// opened. The label comes from the app.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Spoiler;
/// let answer = Spoiler::new("Show the answer").add(Paragraph::new("Forty two."));
/// ```
pub struct Spoiler {
    label: Text,
    nodes: Vec<Node>,
}

impl Spoiler {
    pub fn new(label: impl Into<Text>) -> Self {
        Spoiler {
            label: label.into(),
            nodes: Vec::new(),
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a child component; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(node.into());
        self
    }
}

impl Component for Spoiler {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<details class="z-spoiler"><summary class="z-spoiler-label">{}</summary><div class="z-spoiler-body">"#,
            r.text(&self.label)
        ));
        for (i, node) in self.nodes.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div></details>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "spoiler", "label": self.label, "children": self.nodes.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}
