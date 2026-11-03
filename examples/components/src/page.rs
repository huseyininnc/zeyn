//! The screens of the catalog and the few components only it needs: the gallery of previews, an
//! example with its code, and the API table.

use crate::{API, CATEGORIES, COMPONENTS, Component as Entry, Example, component, matching};
use zeyn::prelude::*;
use zeyn::render::escape;
use zeyn::ui::{ExternalLink, Paragraph, SearchBox, render_child};
use zeyn::{Component, Render};
use zeyn_example_docs::highlight::highlight;

/// Address of the documentation site.
pub const DOCS: &str = "https://docs.zeynsoft.com";

/// The code of example `func` in a component's file: the file's `use` lines (but the catalog's
/// own), then the function from its `pub fn` line to the closing brace at the start of a line.
pub fn snippet(source: &str, func: &str) -> String {
    let uses: Vec<&str> = source
        .lines()
        .filter(|l| l.starts_with("use ") && *l != "use crate::{Entry, ex};")
        .collect();
    let start = format!("pub fn {func}(");
    let mut body = Vec::new();
    let mut inside = false;
    for line in source.lines() {
        if !inside && line.starts_with(&start) {
            inside = true;
        }
        if inside {
            body.push(line);
            if line == "}" {
                break;
            }
        }
    }
    let mut out = uses.join("\n");
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&body.join("\n"));
    out
}

/// The text of a catalog key.
fn key(name: String) -> Text {
    Text::key(name)
}

/// The index screen: a search box, then every category with a card per component, each with a
/// small live preview of the component's first example.
pub fn index_screen(q: &str, ctx: &Ctx) -> Screen {
    let found = matching(q, ctx);
    let mut sections = Vec::new();
    let mut previews = Vec::new();
    for cat in CATEGORIES {
        let cards: Vec<&'static Entry> = found
            .iter()
            .copied()
            .filter(|c| c.category == *cat)
            .collect();
        if cards.is_empty() {
            continue;
        }
        for c in &cards {
            previews.push(
                c.entry
                    .overview
                    .first()
                    .map(|e| (e.build)(ctx))
                    .unwrap_or_else(|| Stack::new().into()),
            );
        }
        sections.push((*cat, cards));
    }
    let search = SearchBox::of(crate::find_components)
        .label(t!("cx.search"))
        .suggestion(|f: &crate::Found| f.name.clone())
        .value(q)
        .results(crate::catalog_home);
    Screen::new(t!("cx.title"))
        .body(Paragraph::new(t!("cx.intro")))
        .body(search)
        .body(Node::new(Gallery {
            sections,
            previews,
            q: q.to_string(),
        }))
}

/// The cards of the index, grouped by category. The previews are real components rendered with
/// the page; they are `inert`, so they take no focus and no clicks, and the card's title link
/// covers the whole card.
struct Gallery {
    sections: Vec<(&'static str, Vec<&'static Entry>)>,
    previews: Vec<Node>,
    q: String,
}

impl Component for Gallery {
    fn render(&self, out: &mut String, r: &Render) {
        if self.sections.is_empty() {
            out.push_str(&format!(
                r#"<p class="cx-none" role="status">{}</p>"#,
                r.text(&t!("cx.none", q = self.q.clone()))
            ));
            return;
        }
        let mut i = 0;
        for (cat, cards) in &self.sections {
            let id = format!("cx-cat-{cat}");
            out.push_str(&format!(
                r#"<section class="cx-section" aria-labelledby="{id}"><h2 class="cx-section-title" id="{id}">{}</h2><ul class="cx-cards">"#,
                r.text(&key(format!("cx.cat.{cat}")))
            ));
            for c in cards {
                out.push_str(r#"<li class="cx-card"><div class="cx-preview" inert>"#);
                if let Some(node) = self.previews.get(i) {
                    render_child(out, r, i, node);
                }
                out.push_str(&format!(
                    r#"</div><div class="cx-card-body"><h3 class="cx-card-title"><a class="cx-card-link" href="{}">{}</a></h3><p class="cx-card-text">{}</p></div></li>"#,
                    c.route,
                    escape(c.entry.name),
                    r.text(&c.summary())
                ));
                i += 1;
            }
            out.push_str("</ul></section>");
        }
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "component_gallery", "children": self.previews.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.previews
    }
}

/// One live example with a heading, a "Show code" disclosure and a copy button under it.
struct ExampleBlock {
    title: Text,
    code: String,
    demo: Vec<Node>,
}

impl Component for ExampleBlock {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<section class="cx-example"><h3 class="cx-example-title">{}</h3><div class="cx-demo">"#,
            r.text(&self.title)
        ));
        if let Some(node) = self.demo.first() {
            render_child(out, r, 0, node);
        }
        out.push_str(&format!(
            r#"</div><div class="cx-code-wrap"><details class="cx-code"><summary class="cx-code-toggle">{}</summary><pre class="z-code" tabindex="0"><code>{}</code></pre></details><button type="button" class="z-button cx-copy" data-cx-copy data-text-copy="{2}" data-text-copied="{3}" hidden>{2}</button><span class="z-sr-only" role="status" data-cx-copy-status></span></div></section>"#,
            r.text(&t!("cx.show_code")),
            highlight("rust", &self.code),
            r.text(&t!("cx.copy")),
            r.text(&t!("cx.copied")),
        ));
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "component_example", "title": self.title, "children": self.demo.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.demo
    }
}

fn blocks(c: &Entry, list: &[Example], ctx: &Ctx) -> Node {
    list.iter()
        .fold(Stack::new().gap(6), |stack, e| {
            stack.add(Node::new(ExampleBlock {
                title: key(format!("cx.ex.{}.{}", c.slug, e.func)),
                code: snippet(c.source, e.func),
                demo: vec![(e.build)(ctx)],
            }))
        })
        .into()
}

/// The public methods of the component's types, taken from the framework's source at build time.
struct ApiTable {
    types: &'static [&'static str],
}

impl Component for ApiTable {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="cx-api"><p class="cx-api-note">{}</p>"#,
            r.text(&t!("cx.api_note"))
        ));
        for ty in self.types {
            let methods = API
                .iter()
                .find(|(name, _)| name == ty)
                .map(|(_, m)| *m)
                .unwrap_or(&[]);
            out.push_str(&format!(
                r#"<section class="cx-api-type"><h3 class="cx-api-name"><code>{0}</code></h3><p>{1}</p>"#,
                escape(ty),
                ExternalLink::new(format!("https://docs.rs/zeyn/latest/zeyn/?search={ty}"), t!("cx.docs_rs")).html(r)
            ));
            if methods.is_empty() {
                out.push_str("</section>");
                continue;
            }
            out.push_str(&format!(
                r#"<div class="cx-api-scroll" tabindex="0" role="region" aria-label="{}"><table class="cx-api-table"><thead><tr><th scope="col">{}</th><th scope="col">{}</th></tr></thead><tbody>"#,
                escape(ty),
                r.text(&t!("cx.api_method")),
                r.text(&t!("cx.api_doc"))
            ));
            for (sig, doc) in methods {
                out.push_str(&format!(
                    r#"<tr><td><code>{}</code></td><td>{}</td></tr>"#,
                    highlight("rust", sig),
                    escape(doc)
                ));
            }
            out.push_str("</tbody></table></div></section>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "component_api", "types": self.types })
    }
}

/// The page of a component: its description and links, then the Overview, API and Examples tabs.
pub fn component_screen(slug: &str, ctx: &Ctx) -> Screen {
    let Some(c) = component(slug) else {
        return Screen::new(t!("cx.missing"));
    };
    let e = c.entry;
    let links = Row::new()
        .compact()
        .col(ExternalLink::new(
            format!("{DOCS}/{}", e.docs),
            t!("cx.docs"),
        ))
        .col(ExternalLink::new(
            format!(
                "https://docs.rs/zeyn/latest/zeyn/?search={}",
                e.api.first().unwrap_or(&e.name)
            ),
            t!("cx.docs_rs"),
        ));
    let mut tabs = Tabs::new(t!("cx.tabs", name = e.name))
        .tab(t!("cx.overview"), blocks(c, e.overview, ctx))
        .tab(t!("cx.api"), Node::new(ApiTable { types: e.api }));
    if !e.examples.is_empty() {
        tabs = tabs.tab(t!("cx.examples"), blocks(c, e.examples, ctx));
    }
    Screen::new(e.name)
        .body(Paragraph::new(c.summary()))
        .body(links)
        .body(tabs)
}

/// Every key the catalog looks up at run time, for the locale test.
pub fn keys() -> Vec<String> {
    let mut keys: Vec<String> = CATEGORIES
        .iter()
        .map(|c| format!("cx.cat.{c}"))
        .collect();
    for c in COMPONENTS {
        keys.push(format!("cx.summary.{}", c.slug));
        for e in c
            .entry
            .overview
            .iter()
            .chain(c.entry.examples)
        {
            keys.push(format!("cx.ex.{}.{}", c.slug, e.func));
        }
    }
    keys
}
