//! Content components for reading pages: [`Heading`], [`Paragraph`], [`MetaLine`],
//! [`PrevNext`], [`Avatar`], [`AvatarGroup`], [`Spoiler`], [`Divider`], [`Kbd`], [`Quote`] and
//! the [`AutoGrid`] layout. They are plain server rendered HTML with no browser script; their
//! styles live in `content.css`.

use super::layout::into_node;
use super::render_child;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(
    Heading,
    Paragraph,
    MetaLine,
    PrevNext,
    AutoGrid,
    Avatar,
    AvatarGroup,
    Spoiler,
    Divider,
    Kbd,
    Quote
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
    #[track_caller]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(Node::placed(node));
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

impl Avatar {
    /// Writes the avatar at `size`: its own size, or the one a group sets for all its avatars.
    fn write(&self, out: &mut String, r: &Render, size: AvatarSize) {
        let px = size.px();
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
}

impl Component for Avatar {
    fn render(&self, out: &mut String, r: &Render) {
        self.write(out, r, self.size);
    }

    fn schema(&self) -> Json {
        json!({ "type": "avatar", "name": self.name, "image": self.image, "size": self.size, "decorative": self.decorative })
    }
}

/// Most avatars an [`AvatarGroup`] shows before the rest become a count.
pub const MAX_AVATAR_GROUP: usize = 12;

/// Avatars of the people of one thing, such as the members of a team or the assignees of a task,
/// overlapping in a row. The label names the group for screen readers ("Assignees"). The group
/// shows the first `max` avatars (5 unless set, never more than [`MAX_AVATAR_GROUP`]) and then
/// the number of the rest as "+3", read as "3 more". Every avatar takes the size of the group.
///
/// ```
/// use zeyn::ui::{Avatar, AvatarGroup, AvatarSize};
/// let team = AvatarGroup::new("Assignees")
///     .add(Avatar::new("Ada Lovelace"))
///     .add(Avatar::new("Alan Turing"))
///     .add(Avatar::new("Grace Hopper"))
///     .max(2)
///     .size(AvatarSize::Small);
/// ```
pub struct AvatarGroup {
    label: Text,
    avatars: Vec<Avatar>,
    max: usize,
    size: AvatarSize,
}

impl AvatarGroup {
    /// A group with an accessible name, such as "Assignees".
    pub fn new(label: impl Into<Text>) -> Self {
        AvatarGroup {
            label: label.into(),
            avatars: Vec::new(),
            max: 5,
            size: AvatarSize::Medium,
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends an avatar; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, avatar: Avatar) -> Self {
        self.avatars.push(avatar);
        self
    }

    /// How many avatars show before the rest become a count, 1 to [`MAX_AVATAR_GROUP`].
    pub fn max(mut self, max: usize) -> Self {
        self.max = max.clamp(1, MAX_AVATAR_GROUP);
        self
    }

    /// The size of every avatar of the group.
    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }
}

impl Component for AvatarGroup {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-avatar-group" role="group" aria-label="{}">"#,
            r.text(&self.label)
        ));
        for avatar in self.avatars.iter().take(self.max) {
            avatar.write(out, r, self.size);
        }
        let rest = self
            .avatars
            .len()
            .saturating_sub(self.max);
        if rest > 0 {
            let more = Text::key("zeyn.ui.avatar.more").arg("n", rest.to_string());
            out.push_str(&format!(
                r#"<span class="z-avatar z-avatar-{} z-avatar-more" role="img" aria-label="{}"><span aria-hidden="true">+{rest}</span></span>"#,
                self.size.px(),
                r.text(&more)
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "avatar_group",
            "label": self.label,
            "max": self.max,
            "size": self.size,
            "avatars": self.avatars.iter().map(Component::schema).collect::<Vec<_>>(),
        })
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
    #[track_caller]
    pub fn add(mut self, node: impl Into<Node>) -> Self {
        self.nodes.push(Node::placed(node));
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

/// A line between two parts of a page. Without a label it is a thematic break (`<hr>`); with one
/// the text sits in the middle of the line, such as "or" between two ways to sign in.
/// `.vertical()` draws it between the columns of a compact [`super::Row`].
///
/// ```
/// use zeyn::ui::Divider;
/// let line = Divider::new();
/// let named = Divider::new().label("or");
/// let between = Divider::new().vertical();
/// ```
#[derive(Default)]
pub struct Divider {
    label: Option<Text>,
    vertical: bool,
}

impl Divider {
    pub fn new() -> Self {
        Self::default()
    }

    /// A text in the middle of the line. A vertical divider shows no label.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A line from top to bottom, for the space between columns.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
}

impl Component for Divider {
    fn render(&self, out: &mut String, r: &Render) {
        match (&self.label, self.vertical) {
            (_, true) => out.push_str(
                r#"<div class="z-divider z-divider-vertical" role="separator" aria-orientation="vertical"></div>"#,
            ),
            (Some(label), false) => {
                let label = r.text(label);
                out.push_str(&format!(
                    r#"<div class="z-divider z-divider-labeled" role="separator" aria-label="{label}"><span class="z-divider-label" aria-hidden="true">{label}</span></div>"#
                ));
            }
            (None, false) => out.push_str(r#"<hr class="z-divider">"#),
        }
    }

    fn schema(&self) -> Json {
        json!({ "type": "divider", "label": self.label, "vertical": self.vertical })
    }
}

/// Most keys a [`Kbd`] shows.
pub const MAX_KBD_KEYS: usize = 6;

/// Keys to press, such as `Ctrl` + `K`, written as keyboard input (`<kbd>`). `Kbd::new` reads a
/// combination with `+` between the keys; `Kbd::keys` takes the keys one by one, for a
/// combination that holds the plus key itself. Key names are written as given, at most
/// [`MAX_KBD_KEYS`] of them.
///
/// ```
/// use zeyn::ui::Kbd;
/// let search = Kbd::new("Ctrl+K");
/// let zoom = Kbd::keys(["Ctrl", "+"]);
/// ```
pub struct Kbd {
    keys: Vec<String>,
}

impl Kbd {
    /// A key or a combination with `+` between its keys: `"Esc"`, `"Ctrl+Shift+P"`.
    pub fn new(combination: &str) -> Self {
        Self::keys(combination.split('+').map(str::trim))
    }

    /// The keys of a combination, one by one.
    pub fn keys<I, S>(keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Kbd {
            keys: keys
                .into_iter()
                .map(|k| k.as_ref().to_string())
                .filter(|k| !k.is_empty())
                .take(MAX_KBD_KEYS)
                .collect(),
        }
    }
}

impl Component for Kbd {
    fn render(&self, out: &mut String, _r: &Render) {
        out.push_str(r#"<kbd class="z-keys">"#);
        for (i, key) in self.keys.iter().enumerate() {
            if i > 0 {
                out.push_str(r#"<span class="z-keys-plus">+</span>"#);
            }
            out.push_str(&format!("<kbd>{}</kbd>", escape(key)));
        }
        out.push_str("</kbd>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "kbd", "keys": self.keys })
    }
}

/// Words quoted from a person or a work (`<blockquote>` in a `<figure>`), with who said them and
/// where under the text.
///
/// ```
/// use zeyn::ui::Quote;
/// let q = Quote::new("The machine has no pretensions to originate anything.")
///     .author("Ada Lovelace")
///     .source("Notes on the Analytical Engine");
/// ```
pub struct Quote {
    text: Text,
    author: Option<Text>,
    source: Option<Text>,
}

impl Quote {
    pub fn new(text: impl Into<Text>) -> Self {
        Quote {
            text: text.into(),
            author: None,
            source: None,
        }
    }

    /// Who said it.
    pub fn author(mut self, author: impl Into<Text>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// The work it comes from, written as a title (`<cite>`).
    pub fn source(mut self, source: impl Into<Text>) -> Self {
        self.source = Some(source.into());
        self
    }
}

impl Component for Quote {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<figure class="z-quote"><blockquote class="z-quote-text"><p>{}</p></blockquote>"#,
            r.text(&self.text)
        ));
        if self.author.is_some() || self.source.is_some() {
            out.push_str(r#"<figcaption class="z-quote-by">"#);
            if let Some(author) = &self.author {
                out.push_str(&format!(
                    r#"<span class="z-quote-author">{}</span>"#,
                    r.text(author)
                ));
            }
            if let Some(source) = &self.source {
                out.push_str(&format!(
                    r#"<cite class="z-quote-source">{}</cite>"#,
                    r.text(source)
                ));
            }
            out.push_str("</figcaption>");
        }
        out.push_str("</figure>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "quote", "text": self.text, "author": self.author, "source": self.source })
    }
}
