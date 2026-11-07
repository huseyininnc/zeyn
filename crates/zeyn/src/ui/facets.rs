//! [`FacetPanel`]: counted filters, a price range and the chosen filters as removable chips.

use super::layout::into_node;
use super::widgets::screen_href;
use super::{PageCheck, RangeSlider, dom_id, topics_attr};
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Map, Value as Json, json};

into_node!(FacetPanel);

/// Most options one facet shows; the action counting them should ask for no more.
pub const MAX_FACET_OPTIONS: usize = 50;

/// Most facets one panel holds; more are left out.
pub const MAX_FACETS: usize = 12;

/// One counted filter of a [`FacetPanel`], such as the colors with the number of products in
/// each: a group of checkboxes sent under `name`, a screen param that holds a list of strings
/// (`Vec<String>`).
///
/// ```
/// use zeyn::ui::Facet;
/// let colors = Facet::new("color", "Color")
///     .option("red", "Red", 124)
///     .option("blue", "Blue", 0)
///     .chosen(["red"]);
/// ```
pub struct Facet {
    name: &'static str,
    label: Text,
    options: Vec<(String, Text, u64)>,
    chosen: Vec<String>,
    closed: bool,
}

impl Facet {
    pub fn new(name: &'static str, label: impl Into<Text>) -> Self {
        Facet {
            name,
            label: label.into(),
            options: Vec::new(),
            chosen: Vec::new(),
            closed: false,
        }
    }

    /// An option: the value it sends, its text and how many rows have it with the other
    /// facets applied. At most [`MAX_FACET_OPTIONS`]; an option with no rows is turned off unless
    /// it is chosen.
    pub fn option(mut self, value: impl Into<String>, label: impl Into<Text>, count: u64) -> Self {
        let value = value.into();
        if self.options.len() < MAX_FACET_OPTIONS
            && !self
                .options
                .iter()
                .any(|(v, _, _)| *v == value)
        {
            self.options
                .push((value, label.into(), count));
        }
        self
    }

    /// The values chosen now, from the screen params. Values that are no option are ignored.
    pub fn chosen<V: Into<String>>(mut self, values: impl IntoIterator<Item = V>) -> Self {
        self.chosen = values
            .into_iter()
            .map(Into::into)
            .take(MAX_FACET_OPTIONS)
            .collect();
        self
    }

    /// Starts the group folded; it opens on a press of its heading. Groups start open.
    pub fn folded(mut self) -> Self {
        self.closed = true;
        self
    }

    fn is_chosen(&self, value: &str) -> bool {
        self.chosen.iter().any(|c| c == value)
    }
}

enum Part {
    Facet(Facet),
    Range(RangeSlider),
}

/// The filters of a product listing: counted facets ("Red (124)"), a price range and the chosen
/// filters as chips with "Clear all". It is a filter form that drives `topics` and listens to
/// them itself: a change puts the values into the screen params of the facets' names, keeps the
/// address in step and refreshes, in one `POST /_zeyn/nodes`, every node that listens (the card
/// grid, a count, the panel with its new counts); a newer change cancels the request still on its
/// way. Facets are checkboxes in foldable groups (`<details>`, open groups stay open across
/// refreshes), each named "Red, 124 products" for screen readers; the chips are links to the
/// screen without that filter, so they work without script too.
///
/// The counts come from the screen, which asks its data layer once per facet with the other
/// facets applied (`GROUP BY` with a `LIMIT` of [`MAX_FACET_OPTIONS`]); the grid takes the same
/// choices with `CardGrid::scope_any` and `scope_range`, so the browser never sends filters of its
/// own. Facet params are lists of strings (`Vec<String>` with `#[serde(default)]`).
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Facet, FacetPanel};
///
/// #[params]
/// #[derive(Default)]
/// pub struct Listing {
///     #[serde(default)]
///     pub color: Vec<String>,
///     pub min_price: Option<i64>,
///     pub max_price: Option<i64>,
/// }
///
/// #[screen(route = "/shop", public)]
/// pub fn shop(p: Listing) -> Screen {
///     Screen::new("Shop")
///         .body(
///             FacetPanel::of(shop, &p)
///                 .facet(
///                     Facet::new("color", "Color")
///                         .option("red", "Red", 124)
///                         .option("blue", "Blue", 37)
///                         .chosen(p.color.clone()),
///                 )
///                 .range(
///                     RangeSlider::new("Price", ["min_price", "max_price"], 0, 5_000)
///                         .money(Currency::Try)
///                         .value(p.min_price, p.max_price),
///                 )
///                 .drives("products"),
///         )
///         .body(Kpi::new("Products", 161.0).listens("products"))
/// }
///
/// let app = App::new("Shop").feature(Feature::new("shop").screen(shop));
/// assert!(app.check_page(&shop::run(Listing::default())).is_ok());
/// ```
pub struct FacetPanel {
    screen: &'static str,
    params: Json,
    label: Option<Text>,
    parts: Vec<Part>,
    drives: Vec<&'static str>,
}

impl FacetPanel {
    /// The facets of `screen`, whose current params the chips change. Params of the wrong type
    /// do not compile.
    pub fn of<S: ScreenDef>(_screen: S, params: &S::Params) -> Self {
        FacetPanel {
            screen: S::META.name,
            params: serde_json::to_value(params).unwrap_or(Json::Null),
            label: None,
            parts: Vec::new(),
            drives: Vec::new(),
        }
    }

    /// A counted facet. At most [`MAX_FACETS`] facets and ranges together.
    pub fn facet(mut self, facet: Facet) -> Self {
        if self.parts.len() < MAX_FACETS {
            self.parts.push(Part::Facet(facet));
        }
        self
    }

    /// A range, such as the price, in its own foldable group.
    pub fn range(mut self, range: RangeSlider) -> Self {
        if self.parts.len() < MAX_FACETS {
            self.parts.push(Part::Range(range));
        }
        self
    }

    /// The name of the panel for screen readers, "Filters" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The topics a change refreshes; the panel listens to them too, so its counts follow.
    pub fn drives(mut self, topics: impl super::Topics) -> Self {
        self.drives = super::topic_list(topics);
        self
    }

    /// A link to the screen with these params changed, and the params as the chip sends them.
    fn chip_link(&self, r: &Render, change: &[(&str, Json)]) -> (String, String) {
        let mut params = match &self.params {
            Json::Object(m) => m.clone(),
            _ => Map::new(),
        };
        for (k, v) in change {
            params.insert(k.to_string(), v.clone());
        }
        let params = Json::Object(params);
        (screen_href(r, self.screen, &params), attr(&params))
    }

    /// How many filters are chosen: each chosen option and each narrowed range.
    fn active(&self) -> usize {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Facet(f) => f
                    .options
                    .iter()
                    .filter(|(v, _, _)| f.is_chosen(v))
                    .count(),
                Part::Range(s) => usize::from(s.chosen().is_some()),
            })
            .sum()
    }

    fn chips(&self, r: &Render) -> String {
        let t = |key: &str| Text::key(format!("zeyn.ui.shop.facets.{key}"));
        let mut chips = String::new();
        let mut cleared: Vec<(&str, Json)> = Vec::new();
        for part in &self.parts {
            match part {
                Part::Facet(f) => {
                    cleared.push((f.name, json!([])));
                    for (value, label, _) in f
                        .options
                        .iter()
                        .filter(|(v, _, _)| f.is_chosen(v))
                    {
                        let rest: Vec<&String> = f
                            .chosen
                            .iter()
                            .filter(|c| *c != value)
                            .collect();
                        let (href, params) = self.chip_link(r, &[(f.name, json!(rest))]);
                        chips.push_str(&chip(
                            &href,
                            &params,
                            &r.text(label),
                            &r.text(&t("remove").arg("filter", label.clone())),
                        ));
                    }
                }
                Part::Range(s) => {
                    let [a, b] = s.names();
                    cleared.push((a, Json::Null));
                    cleared.push((b, Json::Null));
                    if let Some((lo, hi)) = s.chosen() {
                        let text = format!(
                            "{}: {} {} {}",
                            r.text(s.label()),
                            s.shown(r, lo),
                            r.text(&Text::key("zeyn.ui.shop.range.to")),
                            s.shown(r, hi)
                        );
                        let (href, params) = self.chip_link(r, &[(a, Json::Null), (b, Json::Null)]);
                        chips.push_str(&chip(
                            &href,
                            &params,
                            &text,
                            &r.text(&t("remove").arg("filter", s.label().clone())),
                        ));
                    }
                }
            }
        }
        if chips.is_empty() {
            return String::new();
        }
        let (href, params) = self.chip_link(r, &cleared);
        format!(
            r#"<div class="z-facets-chosen" role="group" aria-label="{}">{chips}<a class="z-facets-clear" href="{href}" data-zeyn-params="{params}" data-zeyn-base-href>{}</a></div>"#,
            r.text(&t("chosen")),
            r.text(&t("clear")),
        )
    }
}

/// A chosen filter as a link to the screen without it.
fn chip(href: &str, params: &str, text: &str, name: &str) -> String {
    format!(
        r#"<a class="z-facet-chip" href="{href}" data-zeyn-params="{params}" data-zeyn-base-href aria-label="{name}"><span>{text}</span><span class="z-facet-x" aria-hidden="true">&times;</span></a>"#
    )
}

impl Component for FacetPanel {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "facets");
        let label = match &self.label {
            Some(l) => r.text(l),
            None => r.text(&Text::key("zeyn.ui.shop.facets.label")),
        };
        let mut kinds = Map::new();
        for part in &self.parts {
            match part {
                Part::Facet(f) => {
                    kinds.insert(f.name.to_string(), Json::from("list"));
                }
                Part::Range(s) => s.kinds(&mut kinds),
            }
        }
        out.push_str(&format!(
            r#"<form class="z-facets" aria-label="{label}"{}{} data-zeyn-kinds="{}" novalidate>"#,
            topics_attr("data-zeyn-drives", &self.drives),
            topics_attr("data-zeyn-listens", &self.drives),
            attr(&kinds),
        ));
        out.push_str(&self.chips(r));
        let active = self.active();
        let count = if active > 0 {
            format!(
                r#"<span class="z-facets-count" aria-hidden="true">{active}</span><span class="z-sr-only">{}</span>"#,
                r.text(&Text::key("zeyn.ui.shop.facets.active").arg("n", active.to_string()))
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<details class="z-facets-fold" id="{id}-fold"><summary class="z-facets-toggle"><span>{label}</span>{count}</summary>"#
        ));
        for (g, part) in self.parts.iter().enumerate() {
            match part {
                Part::Facet(f) => {
                    let open = if f.closed { "" } else { " open" };
                    out.push_str(&format!(
                        r#"<details class="z-facet" id="{id}-{g}"{open}><summary class="z-facet-title">{}</summary><fieldset class="z-facet-options"><legend class="z-sr-only">{}</legend>"#,
                        r.text(&f.label),
                        r.text(&f.label),
                    ));
                    for (i, (value, text, count)) in f.options.iter().enumerate() {
                        let chosen = f.is_chosen(value);
                        let state = match (chosen, *count) {
                            (true, _) => " checked",
                            (false, 0) => " disabled",
                            _ => "",
                        };
                        let n = count.to_string();
                        out.push_str(&format!(
                            r#"<label class="z-facet-option"><input type="checkbox" class="z-facet-input" id="{id}-{g}-{i}" name="{}" value="{}" aria-label="{}" data-testid="{}.facet.{}.{}"{state}><span class="z-facet-text" aria-hidden="true">{}</span><span class="z-facet-count" aria-hidden="true">{n}</span></label>"#,
                            escape(f.name),
                            escape(value),
                            r.text(
                                &Text::key("zeyn.ui.shop.facets.option")
                                    .arg("option", text.clone())
                                    .arg("count", n.clone())
                            ),
                            r.scope,
                            escape(f.name),
                            escape(value),
                            r.text(text),
                        ));
                    }
                    out.push_str("</fieldset></details>");
                }
                Part::Range(s) => {
                    out.push_str(&format!(
                        r#"<details class="z-facet" id="{id}-{g}" open><summary class="z-facet-title">{}</summary><div class="z-facet-range">{}</div></details>"#,
                        r.text(s.label()),
                        s.html(r),
                    ));
                }
            }
        }
        out.push_str("</details></form>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "facet_panel",
            "screen": self.screen,
            "drives": self.drives,
            "facets": self
                .parts
                .iter()
                .map(|p| match p {
                    Part::Facet(f) => json!({
                        "name": f.name,
                        "label": f.label,
                        "chosen": f.chosen,
                        "options": f
                            .options
                            .iter()
                            .map(|(v, l, c)| json!({ "value": v, "label": l, "count": c }))
                            .collect::<Vec<_>>(),
                    }),
                    Part::Range(s) => s.schema(),
                })
                .collect::<Vec<_>>(),
        })
    }

    fn page_check(&self, check: &mut PageCheck) {
        if self.drives.is_empty() {
            check.problem("a FacetPanel refreshes nothing; add `.drives(\"products\")` and `.listens(\"products\")` on the grid it filters");
        }
        for t in &self.drives {
            check.drives(t);
            check.listens(t);
        }
        let keys = self.params.as_object();
        let mut names: Vec<&str> = Vec::new();
        for part in &self.parts {
            match part {
                Part::Facet(f) => names.push(f.name),
                Part::Range(s) => {
                    s.check(check);
                    names.extend(s.names());
                }
            }
        }
        for name in names {
            if !keys.is_some_and(|k| k.contains_key(name)) {
                check.problem(format!(
                    "facet `{name}` is not a field of the screen's params; add `pub {name}: ...` to the params struct so the choice survives a refresh"
                ));
            }
        }
    }
}
