//! [`Chips`]: a row of small rounded links, plain or as filters of a screen's params.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(Chips);

/// Most chips one row holds; more are left out.
pub const MAX_CHIPS: usize = 50;

type FilterFn = Box<dyn Fn(&[&str]) -> Json + Send + Sync>;

enum Kind {
    Links(Vec<(Text, &'static str, Json)>),
    Filter { screen: &'static str, params: FilterFn, options: Vec<(String, Text)>, current: Vec<String>, many: bool },
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
///     .link("News", articles, ArticleParams { tags: "news".into() });
/// ```
pub struct Chips {
    label: Text,
    kind: Kind,
}

impl Chips {
    /// Chips that are plain links. `label` names the row for screen readers.
    pub fn links(label: impl Into<Text>) -> Self {
        Chips { label: label.into(), kind: Kind::Links(Vec::new()) }
    }

    /// A link chip to a screen. Params of the wrong type do not compile.
    pub fn link<S: ScreenDef>(mut self, text: impl Into<Text>, _screen: S, params: S::Params) -> Self {
        if let Kind::Links(links) = &mut self.kind
            && links.len() < MAX_CHIPS
        {
            links.push((text.into(), S::META.name, serde_json::to_value(params).unwrap_or(Json::Null)));
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
                params: Box::new(move |values| serde_json::to_value(params(values)).unwrap_or(Json::Null)),
                options: Vec::new(),
                current: Vec::new(),
                many: false,
            },
        }
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
                out.push_str(&format!(r#"<nav class="z-chips" aria-label="{label}"><ul class="z-chips-list">"#));
                for (text, screen, params) in links {
                    out.push_str(&format!(
                        r#"<li><a class="z-chip" href="{}" data-zeyn-base-href>{}</a></li>"#,
                        screen_href(r, screen, params),
                        r.text(text)
                    ));
                }
                out.push_str("</ul></nav>");
            }
            Kind::Filter { screen, params, options, current, many } => {
                let pressed: Vec<&str> = options
                    .iter()
                    .map(|(v, _)| v.as_str())
                    .filter(|v| current.iter().any(|c| c == v))
                    .collect();
                out.push_str(&format!(
                    r#"<div class="z-chips z-chips-filter" role="group" aria-label="{label}"><ul class="z-chips-list">"#
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
                    out.push_str(&format!(
                        r#"<li><a class="z-chip" role="button" aria-pressed="{on}" href="{}" data-zeyn-chip data-value="{}" data-zeyn-base-href><span class="z-chip-check" aria-hidden="true"></span>{}</a></li>"#,
                        screen_href(r, screen, &params(&next)),
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
                    .map(|(t, s, p)| json!({ "text": t, "screen": s, "params": p }))
                    .collect::<Vec<_>>(),
            }),
            Kind::Filter { screen, options, current, many, .. } => json!({
                "type": "filter_chips",
                "label": self.label,
                "screen": screen,
                "options": options
                    .iter()
                    .map(|(v, t)| json!({ "value": v, "text": t }))
                    .collect::<Vec<_>>(),
                "current": current,
                "multiple": many,
            }),
        }
    }
}
