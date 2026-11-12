//! Lists of rows: [`List`] and its [`ListItem`]s. A static list holds the items it is given; a
//! data bound list, made with [`List::of`], loads rows through an action and pages like a
//! [`super::CardGrid`], which it is, laid out as rows.

use super::catalog::safe_image;
use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::model::Model;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(List);

/// One row of a [`List`]: an optional leading image, a title, an optional subtitle and trailing
/// meta (text or an inline component), and the screen the row opens. The whole row is one link, so it is one stop in the
/// tab order; long text wraps.
///
/// ```
/// use zeyn::ui::ListItem;
/// let row = ListItem::new("Ada Lovelace")
///     .image("/img/people/ada.jpg")
///     .subtitle("Mathematics")
///     .meta("3 articles");
/// ```
pub struct ListItem {
    title: Text,
    subtitle: Option<Text>,
    meta: Option<Node>,
    image: Option<String>,
    alt: Text,
    link: Option<(&'static str, Json)>,
    preview: super::preview::PreviewTarget,
}

impl ListItem {
    pub fn new(title: impl Into<Text>) -> Self {
        ListItem {
            title: title.into(),
            subtitle: None,
            meta: None,
            image: None,
            alt: Text::from(""),
            link: None,
            preview: None,
        }
    }

    /// A line under the title, in a quieter color.
    pub fn subtitle(mut self, text: impl Into<Text>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// Short text at the end of the row, such as a date or a count, or an inline component such
    /// as a [`super::RelativeTime`]. The row is one link, so nothing interactive goes here.
    ///
    /// ```
    /// use zeyn::DateTime;
    /// use zeyn::ui::{ListItem, RelativeTime};
    /// let row = ListItem::new("Release notes")
    ///     .meta(RelativeTime::new(DateTime::from_unix_ms(1_778_580_000_000)));
    /// ```
    pub fn meta(mut self, meta: impl Into<Node>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// A small square image at the start of the row: a path of the app (`/...`), an `https:`
    /// address or a `data:image/` URI; anything else is left out. Its alternative text is empty,
    /// because the title next to it names the row; give [`ListItem::alt`] when it says more.
    pub fn image(mut self, src: impl Into<String>) -> Self {
        self.image = Some(src.into());
        self
    }

    /// Alternative text of the image, when it shows something the title does not say.
    pub fn alt(mut self, alt: impl Into<Text>) -> Self {
        self.alt = alt.into();
        self
    }

    /// The screen the row opens, without a page reload. Params of the wrong type do not compile.
    pub fn link<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.link = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }

    /// A preview of `screen` in a panel next to the row: after about half a second of hover
    /// with a mouse, or at once on keyboard focus. The screen loads only then, through the same
    /// access checks as a window, and is kept for the page's life. Escape closes the panel, the
    /// pointer can move onto it, frames and media inside are left out, and touch screens show no
    /// preview (a tap opens the link). Needs [`ListItem::link`]. Params of the wrong type do not
    /// compile.
    pub fn preview<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.preview = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }

    /// The row as a rich search suggestion: `{text, subtitle, image, href}`, texts in the user's
    /// language, the image only when the app's `img-src` allows it, the canonical address.
    pub(crate) fn suggestion(&self, r: &Render) -> Json {
        let plain = |t: &Text| r.i18n.text(r.locale, t);
        let href = self
            .link
            .as_ref()
            .map(|(screen, params)| {
                crate::screen::Routes::href(r, screen, params)
                    .unwrap_or_else(|| format!("/_zeyn/route/{screen}"))
            });
        let image = self
            .image
            .as_deref()
            .filter(|src| safe_image(src) && super::embed::policy(r).allows_image(src));
        json!({
            "text": plain(&self.title),
            "subtitle": self.subtitle.as_ref().map(plain),
            "image": image,
            "href": href,
        })
    }

    /// Writes the row as an `<li>`.
    pub(crate) fn html(&self, out: &mut String, r: &Render) {
        let image = match &self.image {
            Some(src) if safe_image(src) => format!(
                r#"<img class="z-list-img" src="{}" alt="{}" width="48" height="48" loading="lazy" decoding="async">"#,
                escape(src),
                r.text(&self.alt)
            ),
            Some(_) => r#"<span class="z-list-img"></span>"#.to_string(),
            None => String::new(),
        };
        let line = |t: &Option<Text>, class: &str| {
            t.as_ref()
                .map(|t| format!(r#"<span class="{class}">{}</span>"#, r.text(t)))
                .unwrap_or_default()
        };
        let mut meta = String::new();
        if let Some(node) = &self.meta {
            meta.push_str(r#"<span class="z-list-meta">"#);
            node.render(&mut meta, r);
            meta.push_str("</span>");
        }
        let body = format!(
            r#"{image}<span class="z-list-body"><span class="z-list-title">{}</span>{}</span>{meta}"#,
            r.text(&self.title),
            line(&self.subtitle, "z-list-subtitle"),
        );
        match &self.link {
            Some((screen, params)) => out.push_str(&format!(
                r#"<li class="z-list-item"><a class="z-list-row" href="{}" data-zeyn-base-href{}>{body}</a></li>"#,
                screen_href(r, screen, params),
                super::preview::preview_attr(&self.preview)
            )),
            None => out.push_str(&format!(r#"<li class="z-list-item"><div class="z-list-row">{body}</div></li>"#)),
        }
    }

    fn schema(&self) -> Json {
        json!({
            "title": self.title,
            "subtitle": self.subtitle,
            "meta": self
                .meta
                .as_ref()
                .map(Node::schema),
            "image": self.image,
            "link": self.link.as_ref().map(|(s, p)| json!({ "screen": s, "params": p })),
        })
    }
}

/// Rows one under the other in a `<ul>`, each a [`ListItem`]. `List::new()` holds the items it
/// is given; `List::of(action)` loads them from an action and pages like a card grid.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::ListItem;
/// let people = List::new()
///     .title("Team")
///     .item(ListItem::new("Ada Lovelace").subtitle("Mathematics"))
///     .item(ListItem::new("Alan Turing").subtitle("Computing").meta("Lead"));
/// ```
///
/// A data bound list runs the same kind of action a DataGrid uses, with the same paging, sorting
/// and access rules as [`super::CardGrid`]:
///
/// ```ignore
/// List::of(search_people)
///     .title(t!("people.title"))
///     .item(|p: &Person| ListItem::new(p.name.clone()).subtitle(p.city.clone()).link(person, PersonParams { id: p.id }))
///     .sort_option(t!("people.by_name"), Person::name)
///     .paging(Paging::pages(20))
/// ```
#[derive(Default)]
pub struct List {
    title: Option<Text>,
    items: Vec<ListItem>,
}

impl List {
    pub fn new() -> Self {
        Self::default()
    }

    /// A list that loads its rows from the given action, 20 at a time with a "Show more" button.
    /// Build each row with `.item(|row| ListItem::new(...))`.
    pub fn of<A, E>(source: A) -> super::CardGrid<E>
    where
        A: crate::action::Action<
                Input = crate::action::DataQuery,
                Output = crate::action::Result<crate::action::Page<E>>,
            >,
        E: Model,
    {
        super::CardGrid::of(source).into_list()
    }

    /// A heading over the list, which also names it for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn item(mut self, item: ListItem) -> Self {
        self.items.push(item);
        self
    }
}

/// Most items one [`Repeat`] builds; more are left out.
pub const MAX_REPEAT: usize = 500;

into_node!(Repeat);

/// One node per item of an in memory list, one under the other, like Flutter's
/// `ListView.builder` over a list the screen already has. For rows an action loads, use
/// `List::of(...).builder(...)`, which pages. The nodes are children of the repeat, so a form
/// or a lookup in them keeps working (child `i` has the path `p.i`). At most [`MAX_REPEAT`]
/// items.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::{Component, I18n, Render};
///
/// let steps = Repeat::of(["Pack", "Ship", "Deliver"], |s| Badge::new(s))
///     .separator(Paragraph::new("→"));
/// let mut html = String::new();
/// steps.render(&mut html, &Render::new("s", "en", I18n::builtin()));
/// assert_eq!(html.matches(r#"class="z-separator""#).count(), 2);
/// assert_eq!(steps.children().len(), 3);
/// ```
pub struct Repeat {
    nodes: Vec<Node>,
    separator: Option<Node>,
}

impl Repeat {
    pub fn of<T, N: Into<Node>>(
        items: impl IntoIterator<Item = T>,
        build: impl Fn(T) -> N,
    ) -> Self {
        Repeat {
            nodes: items
                .into_iter()
                .take(MAX_REPEAT)
                .map(|item| build(item).into())
                .collect(),
            separator: None,
        }
    }

    /// A node shown between two items, hidden from screen readers.
    pub fn separator(mut self, node: impl Into<Node>) -> Self {
        self.separator = Some(node.into());
        self
    }
}

impl Component for Repeat {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(r#"<div class="z-stack z-repeat">"#);
        for (i, node) in self.nodes.iter().enumerate() {
            if let (Some(sep), true) = (&self.separator, i > 0) {
                out.push_str(r#"<div class="z-separator" aria-hidden="true">"#);
                sep.render(out, r);
                out.push_str("</div>");
            }
            super::render_child(out, r, i, node);
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "repeat", "children": self.nodes.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

impl Component for List {
    fn render(&self, out: &mut String, r: &Render) {
        let id = super::dom_id(r, "list");
        out.push_str(r#"<section class="z-list-section">"#);
        let labelled = match &self.title {
            Some(t) => {
                out.push_str(&format!(
                    r#"<h2 class="z-cards-title" id="{id}-title">{}</h2>"#,
                    r.text(t)
                ));
                format!(r#" aria-labelledby="{id}-title""#)
            }
            None => String::new(),
        };
        out.push_str(&format!(r#"<ul class="z-list"{labelled}>"#));
        for item in &self.items {
            item.html(out, r);
        }
        out.push_str("</ul></section>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "list", "title": self.title, "items": self.items.iter().map(ListItem::schema).collect::<Vec<_>>() })
    }
}
