//! Zeyn documentation, written with Zeyn.
//!
//! Every page is Markdown embedded with `include_str!`, rendered once at startup into shared
//! `Arc<str>` pieces and served by a screen. The menu is the sidebar, the palette searches every
//! page through a server action, and `@live name` lines place real components next to their code.

pub mod highlight;
pub mod live;
pub mod markdown;

use markdown::{Part, Rendered, render};
use std::sync::Arc;
use zeyn::prelude::*;
use zeyn::render::escape;
use zeyn::ui::render_child;
use zeyn::{Component, Render, SearchHit};

/// A page of the documentation: where it lives, its menu label and its Markdown source. `file` is
/// the name other pages use to link to it, such as `theming.md`.
pub struct PageSource {
    pub route: &'static str,
    pub file: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub source: &'static str,
}

macro_rules! pages {
    ($( $group:literal $icon:ident { $( $name:ident $route:literal $file:literal $label:literal $source:literal ; )* } )*) => {
        $( $(
            #[doc = concat!("The page `", $route, "`.")]
            #[screen(route = $route, public)]
            pub fn $name(_: (), docs: Docs, ctx: Ctx) -> Screen {
                docs.screen($route, &ctx)
            }
        )* )*

        /// Every page in reading order.
        pub const PAGES: &[PageSource] = &[ $( $( PageSource { route: $route, file: $file, label: $label, group: $group, source: include_str!($source) }, )* )* ];

        fn with_pages(feature: Feature) -> Feature {
            feature $( $( .screen($name) )* )*
                .menu(|m| m $( .group(Text::key($group), Icon::$icon) $( .item($label, $name) )* )* )
        }
    };
}

pages! {
    "docs.group.start" Home {
        introduction "/" "introduction.md" "Introduction" "../pages/introduction.md";
        cli "/start/cli" "cli.md" "CLI and projects" "../pages/cli.md";
        project "/start/structure" "structure.md" "Project structure" "../pages/structure.md";
    }
    "docs.group.language" Edit {
        entities "/language/entities" "entities.md" "Entities and models" "../pages/entities.md";
        actions "/language/actions" "actions.md" "Actions" "../pages/actions.md";
        screens "/language/screens" "screens.md" "Screens and params" "../pages/screens.md";
        effects "/language/effects" "effects.md" "Effects and windows" "../pages/effects.md";
        forms "/language/forms" "forms.md" "Forms" "../pages/forms.md";
        datagrid_intro "/language/datagrid" "datagrid.md" "DataGrid" "../pages/datagrid.md";
        datagrid_reference "/language/datagrid-reference" "grid.md" "DataGrid reference" "../../../docs/grid.md";
        texts "/language/texts" "texts.md" "Texts and t!" "../pages/texts.md";
    }
    "docs.group.components" Table {
        layouts "/components/layouts" "layouts.md" "Layouts" "../pages/layouts.md";
        controls "/components/controls" "controls.md" "Controls and pickers" "../pages/controls.md";
        lines "/components/lines" "lines.md" "Line editor" "../pages/lines.md";
        display "/components/display" "display.md" "Display" "../pages/display.md";
        content "/components/content" "content.md" "Text and content" "../pages/content.md";
        data_views "/components/data" "data.md" "Tree and charts" "../pages/data.md";
        catalog "/components/catalog" "catalog.md" "Cards, rails and search" "../pages/catalog.md";
        shell "/components/shell" "shell.md" "Shell and commands" "../pages/shell.md";
        media "/components/media" "media.md" "Media" "../../../docs/media.md";
    }
    "docs.group.guides" Box {
        time_i18n "/guides/time-and-languages" "time-and-languages.md" "Time and languages" "../pages/time-and-languages.md";
        security "/guides/security" "security.md" "Security" "../pages/security.md";
        public_pages "/guides/public-pages" "public-pages.md" "Public pages" "../pages/public-pages.md";
        accounts "/guides/accounts" "accounts.md" "User accounts" "../pages/accounts.md";
        auth "/guides/sign-in" "auth.md" "Sign in: OIDC, JWT and MFA" "../../../docs/auth.md";
        enterprise "/guides/enterprise" "enterprise.md" "Audit, masking and jobs" "../../../docs/enterprise.md";
        api "/guides/api" "api.md" "API and OpenAPI" "../pages/api.md";
        mcp "/guides/mcp" "mcp.md" "MCP and AI agents" "../pages/mcp.md";
        database "/guides/database" "database.md" "Data and databases" "../pages/database.md";
        theming "/guides/theming" "theming.md" "Theming" "../../../docs/theming.md";
        polyrepo "/guides/polyrepo" "polyrepo.md" "Shared libraries" "../../../docs/polyrepo.md";
        operations "/guides/operations" "operations.md" "Operations" "../../../docs/operations.md";
        assistant "/guides/assistant" "assistant.md" "AI assistant" "../../../docs/assistant.md";
        islands "/guides/islands" "islands.md" "WASM islands" "../../../docs/islands.md";
        realtime "/guides/realtime" "realtime.md" "Realtime and CRDT" "../../../docs/realtime.md";
        desktop "/guides/desktop" "desktop.md" "Desktop apps" "../../../docs/desktop.md";
    }
    "docs.group.reference" Search {
        protocol "/reference/runtime-protocol" "runtime-protocol.md" "Runtime protocol" "../../../docs/runtime-protocol.md";
        database_reference "/reference/database" "database.md" "Database reference" "../../../docs/database.md";
        architecture "/reference/architecture" "vision-and-architecture.md" "Architecture (historical)" "../../../docs/vision-and-architecture.md";
    }
}

/// The app path of a link target such as `theming.md#density` or `docs/theming.md`; other
/// targets are returned unchanged.
pub fn resolve(target: &str) -> String {
    let (path, hash) = target.split_once('#').map(|(p, h)| (p, format!("#{h}"))).unwrap_or((target, String::new()));
    let file = path.rsplit('/').next().unwrap_or(path);
    match PAGES.iter().find(|p| p.file == file).filter(|_| file.ends_with(".md")) {
        Some(p) => format!("{}{hash}", p.route),
        None => target.to_string(),
    }
}

/// A rendered page: HTML pieces with the places of live examples between them, the "On this
/// page" list, previous and next pages and the text for search.
pub struct Doc {
    pub route: &'static str,
    pub title: String,
    chunks: Vec<Arc<str>>,
    lives: Vec<String>,
    toc: Arc<str>,
    prev: Option<usize>,
    next: Option<usize>,
    sections: Vec<markdown::Section>,
}

impl Doc {
    /// The page's HTML pieces, without live examples. For tests and size checks.
    pub fn chunks(&self) -> &[Arc<str>] {
        &self.chunks
    }

    pub fn lives(&self) -> &[String] {
        &self.lives
    }
}

/// Every page, rendered once. Cheap to clone; register it with `App::service`.
#[derive(Clone)]
pub struct Docs(Arc<Vec<Doc>>);

impl Service for Docs {}

impl Docs {
    /// Renders every page. Pages are small, so this takes a few milliseconds at startup.
    pub fn build() -> Docs {
        let mut docs: Vec<Doc> = PAGES
            .iter()
            .map(|p| {
                let Rendered { title, parts, headings, sections } = render(p.source, &resolve);
                let mut chunks = Vec::new();
                let mut lives = Vec::new();
                let mut html = String::new();
                for part in parts {
                    match part {
                        Part::Html(h) => html.push_str(&h),
                        Part::Live(name) => {
                            chunks.push(Arc::from(std::mem::take(&mut html)));
                            lives.push(name);
                        }
                    }
                }
                chunks.push(Arc::from(html));
                let mut toc = String::from("<ul>");
                for h in headings.iter().filter(|h| h.level <= 3) {
                    let class = if h.level == 3 { r#" class="z-doc-toc-sub""# } else { "" };
                    toc.push_str(&format!(r##"<li{class}><a href="#{}">{}</a></li>"##, h.id, escape(&h.text)));
                }
                toc.push_str("</ul>");
                let title = if title.is_empty() { p.label.to_string() } else { title };
                Doc { route: p.route, title, chunks, lives, toc: Arc::from(toc), prev: None, next: None, sections }
            })
            .collect();
        let n = docs.len();
        for (i, d) in docs.iter_mut().enumerate() {
            d.prev = i.checked_sub(1);
            d.next = (i + 1 < n).then_some(i + 1);
        }
        Docs(Arc::new(docs))
    }

    pub fn pages(&self) -> &[Doc] {
        &self.0
    }

    fn index(&self, route: &str) -> Option<usize> {
        self.0.iter().position(|d| d.route == route)
    }

    /// The screen of a page, with its live examples built for this request.
    pub fn screen(&self, route: &str, ctx: &Ctx) -> Screen {
        let Some(i) = self.index(route) else { return Screen::new(t!("docs.missing")) };
        let lives = self.0[i]
            .lives
            .iter()
            .map(|n| live::example(n, ctx).unwrap_or_else(|| Alert::danger(t!("docs.missing")).into()))
            .collect();
        Screen::new(self.0[i].title.clone()).body(Node::new(DocPage { docs: self.clone(), index: i, lives }))
    }

    /// Pages and sections whose text has every word of the query, best first: words in a
    /// heading count most, then the page title, then the text.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let words: Vec<String> =
            query.split_whitespace().map(str::to_lowercase).filter(|w| !w.is_empty()).take(8).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(usize, SearchHit)> = Vec::new();
        for d in self.0.iter() {
            let title = d.title.to_lowercase();
            for s in &d.sections {
                let heading = s.heading.to_lowercase();
                let text = s.text.to_lowercase();
                if !words.iter().all(|w| heading.contains(w) || title.contains(w) || text.contains(w)) {
                    continue;
                }
                let score: usize = words
                    .iter()
                    .map(|w| {
                        10 * usize::from(heading.contains(w))
                            + 5 * usize::from(title.contains(w))
                            + text.matches(w.as_str()).count().min(5)
                    })
                    .sum();
                let (label, url) = if s.id.is_empty() {
                    (d.title.clone(), d.route.to_string())
                } else {
                    (s.heading.clone(), format!("{}#{}", d.route, s.id))
                };
                found.push((score, SearchHit::new(label, d.title.clone(), url)));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().take(zeyn::MAX_SEARCH_HITS).map(|(_, h)| h).collect()
    }
}

/// A documentation page on a screen: the article with its live examples, previous and next
/// links, and "On this page". The HTML pieces are shared; only the live examples are built per
/// request.
struct DocPage {
    docs: Docs,
    index: usize,
    lives: Vec<Node>,
}

impl DocPage {
    fn link(&self, r: &Render, at: Option<usize>, class: &str, key: &str) -> String {
        at.map(|i| {
            let d = &self.docs.0[i];
            format!(
                r#"<a class="{class}" href="{}"><span>{}</span>{}</a>"#,
                d.route,
                r.text(&Text::key(key.to_string())),
                escape(&d.title)
            )
        })
        .unwrap_or_default()
    }
}

impl Component for DocPage {
    fn render(&self, out: &mut String, r: &Render) {
        let d = &self.docs.0[self.index];
        out.push_str(r#"<div class="z-doc"><article class="z-prose">"#);
        for (i, chunk) in d.chunks.iter().enumerate() {
            out.push_str(chunk);
            if let Some(node) = self.lives.get(i) {
                out.push_str(&format!(r#"<div class="z-doc-live" role="group" aria-label="{0}"><p class="z-doc-live-label" aria-hidden="true">{0}</p>"#, r.text(&t!("docs.live.label"))));
                render_child(out, r, i, node);
                out.push_str("</div>");
            }
        }
        out.push_str(&format!(
            r#"<nav class="z-doc-pager" aria-label="{}">{}{}</nav></article><nav class="z-doc-toc" aria-labelledby="z-doc-toc-title"><h2 id="z-doc-toc-title">{}</h2>{}</nav></div>"#,
            r.text(&t!("docs.pager")),
            self.link(r, d.prev, "z-doc-prev", "docs.previous"),
            self.link(r, d.next, "z-doc-next", "docs.next"),
            r.text(&t!("docs.on_this_page")),
            d.toc
        ));
    }

    fn schema(&self) -> serde_json::Value {
        let d = &self.docs.0[self.index];
        serde_json::json!({ "type": "doc_page", "route": d.route, "title": d.title, "children": self.lives.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.lives
    }
}

/// Searches every page for the command palette.
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

/// The documentation feature: every page, the sidebar and the palette search.
pub fn feature() -> Feature {
    with_pages(Feature::new("docs")).search(search_docs)
}

/// The documentation app. `docs` is built once with [`Docs::build`].
pub fn app(docs: Docs) -> Result<App, String> {
    let i18n = zeyn::I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
    Ok(App::new(t!("docs.app"))
        .i18n(i18n)
        .llms_txt(include_str!("../llms.txt"))
        .service(docs)
        .home(introduction)
        .feature(feature()))
}
