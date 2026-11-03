//! Zeyn documentation, written with Zeyn.
//!
//! Every page is Markdown embedded with `include_str!`, rendered once at startup into shared
//! `Arc<str>` pieces and served by a screen. The menu is the sidebar, the palette searches every
//! page through a server action, and `@live name` lines place real components next to their code.

pub mod highlight;
pub mod live;
pub mod markdown;

use markdown::{Part, Rendered};
use std::sync::Arc;
use zeyn::prelude::*;
use zeyn::render::escape;
use zeyn::seo::{SitemapPart, SitemapUrl};
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
                .menu(|m| m $( .group(Text::key($group), Icon::$icon) $( .item(Text::key(concat!("docs.page.", stringify!($name))), $name) )* )* )
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
    }
}

include!(concat!(env!("OUT_DIR"), "/translations.rs"));

/// Languages of the documentation, English (the source) first: the path segment and locale
/// code, and the `hreflang` tag search engines get.
pub const LANGS: &[(&str, &str)] = &[
    ("en", "en"),
    ("tr", "tr"),
    ("zh", "zh-Hans"),
    ("ru", "ru"),
    ("es", "es"),
    ("ko", "ko"),
    ("ja", "ja"),
    ("hi", "hi"),
    ("ar", "ar"),
];

/// Content hashes of the English sources each translation was made from, per language and file
/// (`pages/translations.json`). A page whose English source changed since is stale and is served
/// in English until it is translated again. After translating pages, record them with
/// `ZEYN_DOCS_TRANSLATED=tr,zh cargo test -p zeyn-example-docs record_translations` (or `all`).
pub const MANIFEST: &str = include_str!("../pages/translations.json");

/// Hash of a page source for the manifest: 64 bit FNV-1a over the bytes, carriage returns left
/// out so a checkout with CRLF line ends gives the same hash.
///
/// ```
/// assert_eq!(zeyn_example_docs::source_hash("a\r\nb"), zeyn_example_docs::source_hash("a\nb"));
/// ```
pub fn source_hash(source: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in source.bytes().filter(|b| *b != b'\r') {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

impl PageSource {
    /// File name of the page's translations in `pages/<lang>/`. `docs/database.md` shares its
    /// name with the guide, so its translations are `database-reference.md`.
    pub fn translation_file(&self) -> &'static str {
        if self.route == "/reference/database" { "database-reference.md" } else { self.file }
    }
}

/// The app path of a route in a language: `/language/datagrid` in English,
/// `/tr/language/datagrid` in Turkish, `/tr/` for the home page.
pub fn lang_path(lang: &str, route: &str) -> String {
    match (lang, route) {
        ("en", _) => route.to_string(),
        (l, "/") => format!("/{l}/"),
        (l, r) => format!("/{l}{r}"),
    }
}

/// The app path of a link target such as `theming.md#density` or `docs/theming.md`; other
/// targets are returned unchanged.
pub fn resolve(target: &str) -> String {
    resolve_in("en", target)
}

/// [`resolve`] for a page in a language: links to other pages stay in that language.
pub fn resolve_in(lang: &str, target: &str) -> String {
    let (path, hash) = target.split_once('#').map(|(p, h)| (p, format!("#{h}"))).unwrap_or((target, String::new()));
    let file = path.rsplit('/').next().unwrap_or(path);
    let page = PAGES
        .iter()
        .find(|p| p.file == file)
        .filter(|_| file.ends_with(".md"))
        .or_else(|| PAGES.iter().find(|p| p.route == path));
    match page {
        Some(p) => format!("{}{hash}", lang_path(lang, p.route)),
        None => target.to_string(),
    }
}

/// Whether a page is shown in its language and, when not, why it is shown in English.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The English source.
    Source,
    /// A current translation.
    Translated,
    /// No translation file in this language.
    Missing,
    /// The English source changed after the translation was made.
    Stale,
    /// The translation's headings do not match the English page, so its anchors would differ.
    Headings,
}

impl Status {
    /// Whether the page is shown in its own language.
    pub fn translated(self) -> bool {
        matches!(self, Status::Source | Status::Translated)
    }
}

/// A rendered page: HTML pieces with the places of live examples between them, the "On this
/// page" list, previous and next pages and the text for search.
pub struct Doc {
    pub route: &'static str,
    pub title: String,
    pub lang: &'static str,
    pub status: Status,
    chunks: Vec<Arc<str>>,
    lives: Vec<String>,
    toc: Arc<str>,
    headings: Vec<(u8, String)>,
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

    /// Level and anchor of every heading, in order.
    pub fn headings(&self) -> &[(u8, String)] {
        &self.headings
    }

    /// The page's path in its language.
    pub fn url(&self) -> String {
        lang_path(self.lang, self.route)
    }

    fn new(p: &PageSource, lang: &'static str, status: Status, source: &str, ids: &[String]) -> Doc {
        let link = |t: &str| resolve_in(lang, t);
        let Rendered { title, parts, headings, sections } = markdown::render_with_ids(source, &link, ids);
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
        Doc {
            route: p.route,
            title,
            lang,
            status,
            chunks,
            lives,
            toc: Arc::from(toc),
            headings: headings.into_iter().map(|h| (h.level, h.id)).collect(),
            prev: None,
            next: None,
            sections,
        }
    }
}

/// Every page in every language, rendered once. Cheap to clone; register it with `App::service`.
#[derive(Clone)]
pub struct Docs(Arc<Vec<Vec<Doc>>>);

impl Service for Docs {}

impl Docs {
    /// Renders every page in every language. Pages are small, so this takes a few milliseconds
    /// per language at startup.
    pub fn build() -> Docs {
        Docs::build_from(TRANSLATIONS, MANIFEST)
    }

    /// Renders the pages with these translations and this manifest (see [`MANIFEST`]). A page
    /// without a current translation whose headings match the English page is rendered from
    /// the English source, with links that stay in the language.
    pub fn build_from(translations: &[(&str, &str, &str)], manifest: &str) -> Docs {
        let manifest: serde_json::Value = serde_json::from_str(manifest).unwrap_or_default();
        let english: Vec<Doc> = PAGES.iter().map(|p| Doc::new(p, "en", Status::Source, p.source, &[])).collect();
        let mut all = vec![english];
        for (lang, _) in &LANGS[1..] {
            let docs = PAGES
                .iter()
                .zip(all[0].iter())
                .map(|(p, en)| {
                    let file = p.translation_file();
                    let ids: Vec<String> = en.headings.iter().map(|(_, id)| id.clone()).collect();
                    let found = translations.iter().find(|(l, f, _)| l == lang && *f == file).map(|(_, _, s)| *s);
                    let current = manifest[*lang][file].as_str() == Some(source_hash(p.source).as_str());
                    let status = match found {
                        None => Status::Missing,
                        Some(_) if !current => Status::Stale,
                        Some(source) => {
                            let doc = Doc::new(p, lang, Status::Translated, source, &ids);
                            let levels = |d: &Doc| d.headings.iter().map(|(l, _)| *l).collect::<Vec<_>>();
                            if levels(&doc) == levels(en) {
                                return doc;
                            }
                            Status::Headings
                        }
                    };
                    Doc::new(p, lang, status, p.source, &[])
                })
                .collect();
            all.push(docs);
        }
        for docs in &mut all {
            let n = docs.len();
            for (i, d) in docs.iter_mut().enumerate() {
                d.prev = i.checked_sub(1);
                d.next = (i + 1 < n).then_some(i + 1);
            }
        }
        Docs(Arc::new(all))
    }

    /// The English pages.
    pub fn pages(&self) -> &[Doc] {
        &self.0[0]
    }

    /// The pages of a language (English for an unknown one).
    pub fn lang(&self, lang: &str) -> &[Doc] {
        &self.0[lang_index(lang)]
    }

    fn index(&self, route: &str) -> Option<usize> {
        self.0[0].iter().position(|d| d.route == route)
    }

    /// The screen of a page in the request's language, with its live examples built for this
    /// request, its canonical URL and its language alternates.
    pub fn screen(&self, route: &str, ctx: &Ctx) -> Screen {
        let Some(i) = self.index(route) else { return Screen::new(t!("docs.missing")) };
        let lang = lang_index(ctx.locale());
        let doc = &self.0[lang][i];
        let lives = doc
            .lives
            .iter()
            .map(|n| live::example(n, ctx).unwrap_or_else(|| Alert::danger(t!("docs.missing")).into()))
            .collect();
        let canonical = if doc.status.translated() { doc.url() } else { route.to_string() };
        let mut screen = Screen::new(doc.title.clone()).canonical(canonical);
        for (l, (code, tag)) in LANGS.iter().enumerate() {
            if self.0[l][i].status.translated() {
                screen = screen.alternate(tag, lang_path(code, route));
            }
        }
        screen
            .alternate("x-default", route)
            .body(Node::new(DocPage { docs: self.clone(), lang, index: i, lives }))
    }

    /// Every URL of the sitemap: each page in each language it is translated into.
    pub fn urls(&self) -> Vec<SitemapUrl> {
        self.0.iter().flatten().filter(|d| d.status.translated()).map(|d| SitemapUrl::new(d.url())).collect()
    }

    /// Pages and sections of a language whose text has every word of the query, best first:
    /// words in a heading count most, then the page title, then the text.
    pub fn search(&self, lang: &str, query: &str) -> Vec<SearchHit> {
        let words: Vec<String> =
            query.split_whitespace().map(str::to_lowercase).filter(|w| !w.is_empty()).take(8).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<(usize, SearchHit)> = Vec::new();
        for d in self.lang(lang) {
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
                    (d.title.clone(), d.url())
                } else {
                    (s.heading.clone(), format!("{}#{}", d.url(), s.id))
                };
                found.push((score, SearchHit::new(label, d.title.clone(), url)));
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().take(zeyn::MAX_SEARCH_HITS).map(|(_, h)| h).collect()
    }
}

/// Position of a language in [`LANGS`], English for an unknown one.
fn lang_index(lang: &str) -> usize {
    LANGS.iter().position(|(code, _)| *code == lang).unwrap_or(0)
}

/// A documentation page on a screen: the article with its live examples, previous and next
/// links, and "On this page". The HTML pieces are shared; only the live examples are built per
/// request. A translated page starts with a note that links to the English page; a page shown
/// in English under another language says it is not translated yet.
struct DocPage {
    docs: Docs,
    lang: usize,
    index: usize,
    lives: Vec<Node>,
}

impl DocPage {
    fn doc(&self, at: usize) -> &Doc {
        &self.docs.0[self.lang][at]
    }

    fn link(&self, r: &Render, at: Option<usize>, class: &str, key: &str) -> String {
        at.map(|i| {
            let d = self.doc(i);
            format!(
                r#"<a class="{class}" href="{}"><span>{}</span>{}</a>"#,
                d.url(),
                r.text(&Text::key(key.to_string())),
                escape(&d.title)
            )
        })
        .unwrap_or_default()
    }

    fn note(&self, r: &Render) -> String {
        let d = self.doc(self.index);
        match d.status {
            Status::Source => String::new(),
            Status::Translated => format!(
                r#"<p class="z-doc-note" role="note">{} <a href="{}" hreflang="en" lang="en">{}</a></p>"#,
                r.text(&t!("docs.translation.note")),
                d.route,
                r.text(&t!("docs.translation.english"))
            ),
            _ => format!(r#"<p class="z-doc-note" role="note">{}</p>"#, r.text(&t!("docs.translation.missing"))),
        }
    }
}

impl Component for DocPage {
    fn render(&self, out: &mut String, r: &Render) {
        let d = self.doc(self.index);
        let lang = if d.status.translated() { "" } else { r#" lang="en""# };
        out.push_str(r#"<div class="z-doc">"#);
        out.push_str(&format!(r#"<article class="z-prose"{lang}>"#));
        out.push_str(&self.note(r));
        for (i, chunk) in d.chunks.iter().enumerate() {
            out.push_str(chunk);
            if let Some(node) = self.lives.get(i) {
                out.push_str(&format!(r#"<div class="z-doc-live" role="group" aria-label="{0}"><p class="z-doc-live-label" aria-hidden="true">{0}</p>"#, r.text(&t!("docs.live.label"))));
                render_child(out, r, i, node);
                out.push_str("</div>");
            }
        }
        out.push_str(&format!(
            r#"<nav class="z-doc-pager" aria-label="{}">{}{}</nav></article><nav class="z-doc-toc" aria-labelledby="z-doc-toc-title"{lang}><h2 id="z-doc-toc-title">{}</h2>{}</nav></div>"#,
            r.text(&t!("docs.pager")),
            self.link(r, d.prev, "z-doc-prev", "docs.previous"),
            self.link(r, d.next, "z-doc-next", "docs.next"),
            r.text(&t!("docs.on_this_page")),
            d.toc
        ));
    }

    fn schema(&self) -> serde_json::Value {
        let d = self.doc(self.index);
        serde_json::json!({ "type": "doc_page", "route": d.route, "lang": d.lang, "title": d.title, "children": self.lives.iter().map(Node::schema).collect::<Vec<_>>() })
    }

    fn children(&self) -> &[Node] {
        &self.lives
    }
}

/// Searches every page of the request's language for the command palette.
#[action(public)]
pub async fn search_docs(query: String, docs: Docs, ctx: Ctx) -> Result<Vec<SearchHit>> {
    Ok(docs.search(ctx.locale(), &query))
}

/// The URLs of `/sitemap.xml`: every page in every language it is translated into.
#[action(public)]
pub async fn docs_sitemap(part: SitemapPart, docs: Docs) -> Result<Vec<SitemapUrl>> {
    Ok(part.of(&docs.urls()))
}

/// The documentation feature: every page, the sidebar, the palette search and the sitemap.
pub fn feature() -> Feature {
    with_pages(Feature::new("docs")).search(search_docs).sitemap(docs_sitemap)
}

/// The documentation app. `docs` is built once with [`Docs::build`]. English pages stay at their
/// paths, the other languages are served at `/<lang>/...` (`App::locale_paths`).
pub fn app(docs: Docs) -> Result<App, String> {
    let i18n = zeyn::I18n::new("en").catalogs(&[
        ("en", include_str!("../locales/en.json")),
        ("tr", include_str!("../locales/tr.json")),
        ("zh", include_str!("../locales/zh.json")),
        ("ru", include_str!("../locales/ru.json")),
        ("es", include_str!("../locales/es.json")),
        ("ko", include_str!("../locales/ko.json")),
        ("ja", include_str!("../locales/ja.json")),
        ("hi", include_str!("../locales/hi.json")),
        ("ar", include_str!("../locales/ar.json")),
    ])?;
    Ok(App::new(t!("docs.app"))
        .i18n(i18n)
        .locale_paths()
        .site_url(SITE)
        .llms_txt(include_str!("../llms.txt"))
        .robots_txt(include_str!("../robots.txt"))
        .service(docs)
        .home(introduction)
        .feature(feature()))
}

/// The public address of the documentation, for canonical URLs, alternates and the sitemap.
pub const SITE: &str = "https://docs.zeynsoft.com";
