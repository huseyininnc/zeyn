//! [`Chips`]: a row of small rounded links, plain or as filters of a screen's params.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(Chips);

/// Most chips one row holds; more are left out.
pub const MAX_CHIPS: usize = 50;

type FilterFn = Box<dyn Fn(&[&str]) -> Json + Send + Sync>;

struct LinkChip {
    text: Text,
    screen: &'static str,
    params: Json,
    shortcut: Option<String>,
}

enum Kind {
    Links(Vec<LinkChip>),
    Filter {
        screen: &'static str,
        params: FilterFn,
        options: Vec<(String, Text)>,
        current: Vec<String>,
        many: bool,
    },
}

/// A row of chips: small rounded links that wrap onto more lines.
///
/// Link chips ([`Chips::links`]) each open a screen, such as the tags of an article. Filter
/// chips ([`Chips::filter`]) are toggle buttons over a screen param: each chip is a link to the
/// same screen with its value added to or taken out of the param, so the server renders the
/// pressed state (`aria-pressed`) from the params it got and the page needs no state of its
/// own. Pressing a chip opens the new address through `nav.js` without a page reload; focus
/// and the scroll position stay on the chip. By default one value is chosen at a time (pressing
/// the pressed chip clears it); `.multiple()` lets several be pressed. Values in the params that
/// are not options are ignored. Keyboard: Tab moves between chips, Enter or Space presses one.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Chips;
///
/// #[params]
/// pub struct ArticleParams {
///     pub tags: String,
/// }
///
/// #[screen(route = "/articles", public)]
/// pub fn articles(p: ArticleParams) -> Screen {
///     Screen::new("Articles").body(
///         Chips::filter("Tags", articles, |tags| ArticleParams { tags: tags.join(",") })
///             .current(p.tags.split(','))
///             .option("guide", "Guides")
///             .option("news", "News")
///             .option("release", "Releases")
///             .multiple(),
///     )
/// }
///
/// let topics = Chips::links("Topics")
///     .link("Guides", articles, ArticleParams { tags: "guide".into() })
///     .link("News", articles, ArticleParams { tags: "news".into() })
///     .shortcut("Alt+Shift+N");
/// ```
pub struct Chips {
    label: Text,
    kind: Kind,
    drives: Vec<&'static str>,
}

impl Chips {
    /// Chips that are plain links. `label` names the row for screen readers.
    pub fn links(label: impl Into<Text>) -> Self {
        Chips {
            label: label.into(),
            kind: Kind::Links(Vec::new()),
            drives: Vec::new(),
        }
    }

    /// A link chip to a screen. Params of the wrong type do not compile.
    pub fn link<S: ScreenDef>(
        mut self,
        text: impl Into<Text>,
        _screen: S,
        params: S::Params,
    ) -> Self {
        if let Kind::Links(links) = &mut self.kind
            && links.len() < MAX_CHIPS
        {
            links.push(LinkChip {
                text: text.into(),
                screen: S::META.name,
                params: serde_json::to_value(params).unwrap_or(Json::Null),
                shortcut: None,
            });
        }
        self
    }

    /// A keyboard shortcut such as `"Alt+Shift+N"` for the link chip added last. Pressing it
    /// follows the link through `nav.js`, unless focus is in a text field. The link carries
    /// `aria-keyshortcuts` and shows the keys in a `<kbd>` hint. Shortcuts the browser keeps
    /// or uses are not bound; `App::check` (header slots) and `App::check_page` (screens)
    /// report them, single character keys and clashes with commands or other links.
    pub fn shortcut(mut self, keys: impl Into<String>) -> Self {
        if let Kind::Links(links) = &mut self.kind
            && let Some(last) = links.last_mut()
        {
            last.shortcut = Some(keys.into());
        }
        self
    }

    /// Filter chips over a param of `screen`: `params(values)` builds the screen's params from
    /// the values that are pressed after a chip is pressed, in the order of the options. `label`
    /// names the group for screen readers. Params of the wrong type do not compile.
    pub fn filter<S: ScreenDef>(
        label: impl Into<Text>,
        _screen: S,
        params: impl Fn(&[&str]) -> S::Params + Send + Sync + 'static,
    ) -> Self {
        Chips {
            label: label.into(),
            kind: Kind::Filter {
                screen: S::META.name,
                params: Box::new(move |values| {
                    serde_json::to_value(params(values)).unwrap_or(Json::Null)
                }),
                options: Vec::new(),
                current: Vec::new(),
                many: false,
            },
            drives: Vec::new(),
        }
    }

    /// Filter chips that refresh the nodes listening to `topics` instead of opening the screen
    /// again: pressing a chip puts its params into the screen's params, replaces the address
    /// in the history and refreshes the listeners (and the chips) in place, so the listeners
    /// read the value from their screen params. Only for [`Chips::filter`]; `App::check_page`
    /// reports it on link chips and when nothing on the page listens.
    ///
    /// ```
    /// use zeyn::prelude::*;
    ///
    /// #[params]
    /// pub struct SalesParams {
    ///     pub region: Option<String>,
    /// }
    ///
    /// #[screen(route = "/sales", public)]
    /// pub fn sales(p: SalesParams) -> Screen {
    ///     Screen::new("Sales")
    ///         .body(
    ///             Chips::filter("Region", sales, |v| SalesParams { region: v.first().map(|s| s.to_string()) })
    ///                 .current(p.region.clone())
    ///                 .option("north", "North")
    ///                 .option("south", "South")
    ///                 .drives("sales"),
    ///         )
    ///         .body(Kpi::new("Orders", 12.0).listens("sales"))
    /// }
    ///
    /// let app = App::new("Shop").feature(Feature::new("shop").screen(sales));
    /// assert!(app.check_page(&sales::run(SalesParams { region: None })).is_ok());
    /// ```
    pub fn drives(mut self, topics: impl super::Topics) -> Self {
        self.drives = super::topic_list(topics);
        self
    }

    /// A filter chip: the value it puts in the param and its text.
    pub fn option(mut self, value: impl Into<String>, text: impl Into<Text>) -> Self {
        if let Kind::Filter { options, .. } = &mut self.kind
            && options.len() < MAX_CHIPS
        {
            options.push((value.into(), text.into()));
        }
        self
    }

    /// The values pressed now, from the screen's params. Empty strings are skipped.
    pub fn current<V: Into<String>>(mut self, values: impl IntoIterator<Item = V>) -> Self {
        if let Kind::Filter { current, .. } = &mut self.kind {
            *current = values
                .into_iter()
                .map(Into::into)
                .filter(|v: &String| !v.is_empty())
                .take(MAX_CHIPS)
                .collect();
        }
        self
    }

    /// Lets several filter chips be pressed at once.
    pub fn multiple(mut self) -> Self {
        if let Kind::Filter { many, .. } = &mut self.kind {
            *many = true;
        }
        self
    }
}

impl Component for Chips {
    fn render(&self, out: &mut String, r: &Render) {
        let label = r.text(&self.label);
        match &self.kind {
            Kind::Links(links) => {
                out.push_str(&format!(
                    r#"<nav class="z-chips" aria-label="{label}"><ul class="z-chips-list">"#
                ));
                for chip in links {
                    let (keys, hint) = crate::command::link_shortcut(chip.shortcut.as_deref());
                    out.push_str(&format!(
                        r#"<li><a class="z-chip" href="{}" data-zeyn-base-href{keys}>{}{hint}</a></li>"#,
                        screen_href(r, chip.screen, &chip.params),
                        r.text(&chip.text)
                    ));
                }
                out.push_str("</ul></nav>");
            }
            Kind::Filter {
                screen,
                params,
                options,
                current,
                many,
            } => {
                let pressed: Vec<&str> = options
                    .iter()
                    .map(|(v, _)| v.as_str())
                    .filter(|v| current.iter().any(|c| c == v))
                    .collect();
                out.push_str(&format!(
                    r#"<div class="z-chips z-chips-filter" role="group" aria-label="{label}"{}><ul class="z-chips-list">"#,
                    super::topics_attr("data-zeyn-drives", &self.drives)
                ));
                for (value, text) in options {
                    let on = pressed.contains(&value.as_str());
                    let next: Vec<&str> = if *many {
                        options
                            .iter()
                            .map(|(v, _)| v.as_str())
                            .filter(|v| if v == value { !on } else { pressed.contains(v) })
                            .collect()
                    } else if on {
                        Vec::new()
                    } else {
                        vec![value.as_str()]
                    };
                    let next = params(&next);
                    let driven = if self.drives.is_empty() {
                        String::new()
                    } else {
                        format!(r#" data-zeyn-params="{}""#, attr(&next))
                    };
                    out.push_str(&format!(
                        r#"<li><a class="z-chip" role="button" aria-pressed="{on}" href="{}" data-zeyn-chip data-value="{}" data-zeyn-base-href{driven}><span class="z-chip-check" aria-hidden="true"></span>{}</a></li>"#,
                        screen_href(r, screen, &next),
                        escape(value),
                        r.text(text)
                    ));
                }
                out.push_str("</ul></div>");
            }
        }
    }

    fn schema(&self) -> Json {
        match &self.kind {
            Kind::Links(links) => json!({
                "type": "chips",
                "label": self.label,
                "links": links
                    .iter()
                    .map(|c| json!({ "text": c.text, "screen": c.screen, "params": c.params, "shortcut": c.shortcut }))
                    .collect::<Vec<_>>(),
            }),
            Kind::Filter {
                screen,
                options,
                current,
                many,
                ..
            } => json!({
                "type": "filter_chips",
                "label": self.label,
                "screen": screen,
                "options": options
                    .iter()
                    .map(|(v, t)| json!({ "value": v, "text": t }))
                    .collect::<Vec<_>>(),
                "current": current,
                "multiple": many,
                "drives": self.drives,
            }),
        }
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        if matches!(self.kind, Kind::Links(_)) && !self.drives.is_empty() {
            check.problem(
                crate::Problem::new("Z0611", "`Chips::drives` works on filter chips only")
                    .why("link chips open another screen, so there is no change to refresh listeners with")
                    .fix("make them with `Chips::filter`")
                    .docs("components/content#chips"),
            );
        }
        for t in &self.drives {
            check.drives(t);
        }
    }

    fn shortcuts(&self) -> Vec<&str> {
        match &self.kind {
            Kind::Links(links) => links
                .iter()
                .filter_map(|c| c.shortcut.as_deref())
                .collect(),
            Kind::Filter { .. } => Vec::new(),
        }
    }
}
