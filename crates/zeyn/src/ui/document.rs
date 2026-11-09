//! [`DocumentViewer`]: a paged document, such as a report preview, whose pages are SVG or HTML
//! fragments rendered on the server and fetched only near the viewport.

use super::{DataSource, dom_id};
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Icon, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Largest page an action may return, in bytes, before and after cleaning. A larger page is
/// refused and logged.
pub const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

/// Most pages one document may have. The scroll area of a document is as tall as all its pages,
/// and browsers stop at about 33 million pixels: 5,000 A4 pages at 400 percent.
pub const MAX_DOCUMENT_PAGES: u32 = 5_000;

/// Most search hits one search shows, whatever the action returns.
pub const MAX_PAGE_HITS: usize = 50;

/// Longest search text, in characters; a longer one is refused.
pub const MAX_DOCUMENT_QUERY: usize = 100;

/// Longest text of a search hit, in characters; longer ones are cut.
const MAX_HIT_CHARS: usize = 160;

/// Most entries of an outline, at every level together, and its deepest level.
const MAX_OUTLINE: usize = 1_000;
const MAX_OUTLINE_DEPTH: usize = 6;

/// What the page action of a [`DocumentViewer`] gets: the document from the component (never
/// from the browser) and the page number, which the server checked is between 1 and the page
/// count.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocPage {
    pub document: String,
    pub page: u32,
}

/// What the search action of a [`DocumentViewer`] gets: the document from the component and the
/// trimmed text, 1 to [`MAX_DOCUMENT_QUERY`] characters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocSearch {
    pub document: String,
    pub text: String,
}

/// One search hit: the page and a short text around the match, shown as text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageHit {
    pub page: u32,
    pub text: String,
}

impl PageHit {
    pub fn new(page: u32, text: impl Into<String>) -> Self {
        PageHit {
            page,
            text: text.into(),
        }
    }
}

/// An entry of a document's outline: a title, its page and the entries below it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page: u32,
    #[serde(default)]
    pub children: Vec<OutlineItem>,
}

impl OutlineItem {
    pub fn new(title: impl Into<String>, page: u32) -> Self {
        OutlineItem {
            title: title.into(),
            page,
            children: Vec::new(),
        }
    }

    /// Adds an entry below this one.
    pub fn child(mut self, item: OutlineItem) -> Self {
        self.children.push(item);
        self
    }
}

/// The size of every page of a document, in CSS pixels (96 per inch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSize {
    width: f64,
    height: f64,
}

impl PageSize {
    /// ISO A4 portrait, 210 by 297 mm.
    pub const A4: PageSize = PageSize::pt(595.28, 841.89);
    /// US Letter portrait, 8.5 by 11 inches.
    pub const LETTER: PageSize = PageSize::pt(612.0, 792.0);

    /// A size in CSS pixels, each side 16 to 10,000.
    pub const fn px(width: u32, height: u32) -> Self {
        PageSize {
            width: clamp_side(width as f64),
            height: clamp_side(height as f64),
        }
    }

    /// A size in points (72 per inch), as PDF and report tools give it.
    pub const fn pt(width: f64, height: f64) -> Self {
        PageSize {
            width: clamp_side(width * 96.0 / 72.0),
            height: clamp_side(height * 96.0 / 72.0),
        }
    }

    /// Width and height in CSS pixels.
    pub const fn css(&self) -> (f64, f64) {
        (self.width, self.height)
    }
}

const fn clamp_side(v: f64) -> f64 {
    v.clamp(16.0, 10_000.0)
}

/// The page action, with the document and the page count it serves.
struct Pages {
    meta: ActionMeta,
    call: fn(Ctx, DocPage) -> BoxFuture<Result<Result<String>>>,
    document: String,
    count: u32,
    trusted: bool,
}

/// What the browser posts for a page.
#[derive(Deserialize)]
struct PageInput {
    page: u64,
}

impl DataSource for Pages {
    fn action(&self) -> ActionMeta {
        self.meta
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let page = serde_json::from_value::<PageInput>(input)
            .ok()
            .map(|p| p.page)
            .filter(|p| (1..=u64::from(self.count)).contains(p));
        let (call, document, trusted) = (self.call, self.document.clone(), self.trusted);
        Box::pin(async move {
            let page = page.ok_or(Error::NotFound)? as u32;
            let html = call(ctx, DocPage { document, page }).await??;
            let html = page_markup(&html, page, trusted)?;
            Ok(json!({ "page": page, "html": html }))
        })
    }
}

/// The page as it goes to the browser: cleaned unless trusted, and at most [`MAX_PAGE_BYTES`]
/// before and after.
fn page_markup(html: &str, page: u32, trusted: bool) -> Result<String> {
    let too_big = |n: usize| {
        Error::Internal(format!(
            "page {page} of a DocumentViewer is {n} bytes, more than MAX_PAGE_BYTES ({MAX_PAGE_BYTES})"
        ))
    };
    if html.len() > MAX_PAGE_BYTES {
        return Err(too_big(html.len()));
    }
    if trusted {
        return Ok(html.to_string());
    }
    let clean = super::sanitize::clean(html, &format!("zp{page}-"));
    match clean.len() > MAX_PAGE_BYTES {
        true => Err(too_big(clean.len())),
        false => Ok(clean),
    }
}

/// The `call` of a search action.
type SearchCall = fn(Ctx, DocSearch) -> BoxFuture<Result<Result<Vec<PageHit>>>>;

/// The search action.
struct Search {
    meta: ActionMeta,
    call: SearchCall,
    document: String,
    count: u32,
}

impl DataSource for Search {
    fn action(&self) -> ActionMeta {
        self.meta
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let text = input
            .get("text")
            .and_then(Json::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let fits = !text.is_empty() && text.chars().count() <= MAX_DOCUMENT_QUERY;
        let (call, document, count) = (self.call, self.document.clone(), self.count);
        Box::pin(async move {
            if !fits {
                return Err(Error::NotFound);
            }
            let hits = call(ctx, DocSearch { document, text }).await??;
            let mut kept: Vec<Json> = Vec::new();
            let mut more = false;
            for hit in hits
                .into_iter()
                .filter(|h| (1..=count).contains(&h.page))
            {
                if kept.len() == MAX_PAGE_HITS {
                    more = true;
                    break;
                }
                let text: String = hit
                    .text
                    .chars()
                    .take(MAX_HIT_CHARS)
                    .collect();
                kept.push(json!({ "page": hit.page, "text": text }));
            }
            Ok(json!({ "hits": kept, "more": more }))
        })
    }
}

/// Zoom steps of the zoom buttons and the zoom list, in percent.
const ZOOMS: [u32; 8] = [50, 75, 100, 125, 150, 200, 300, 400];

/// A paged document, such as a report preview, with pages rendered on the server.
///
/// The page action takes a [`DocPage`] and returns one page as an SVG or HTML fragment. Every
/// page has the size of [`DocumentViewer::page_size`] ([`PageSize::A4`] unless set), so the
/// scroll area is as tall as the whole document from the start; only the pages near the viewport
/// are in the page and fetched, at most 3 at a time, and a fetch scrolled away is aborted. At
/// most 12 rendered pages are kept in the browser, the farthest dropped first. Zoom (fit width,
/// fit page, 50 to 400 percent) scales the pages without fetching them again.
///
/// Pages are fetched through `POST /_zeyn/data` (CSRF, the write limit, the screen's access and
/// the page action's permission). The server refuses page numbers outside 1 to the page count,
/// pages over [`MAX_PAGE_BYTES`], and cleans every page with [`sanitize_markup`]'s allow-list
/// (no scripts, event handlers, `foreignObject`, frames, external references or `url()` in
/// styles) unless [`DocumentViewer::trusted`] is set. A viewer the user may not run the page
/// action of is left out of the page.
///
/// [`sanitize_markup`]: crate::ui::sanitize_markup
///
/// ```ignore
/// DocumentViewer::of(report_page, report.pages)
///     .document(report.id.to_string())
///     .page_size(PageSize::A4)
///     .outline(sections)
///     .search(search_report)
///     .download("PDF", format!("/reports/{}/pdf", report.id))
/// ```
///
/// The page action takes a [`DocPage`]; any other input does not compile:
///
/// ```compile_fail
/// # use zeyn::prelude::*;
/// # use zeyn::ui::DocumentViewer;
/// # #[action(public)]
/// # pub async fn page(n: u32) -> Result<String> { Ok(String::new()) }
/// let v = DocumentViewer::of(page, 10);
/// ```
pub struct DocumentViewer {
    pages: Pages,
    search: Option<Search>,
    size: PageSize,
    title: Option<Text>,
    outline: Vec<OutlineItem>,
    downloads: Vec<(Text, String)>,
}

impl DocumentViewer {
    /// A document of `page_count` pages (1 to [`MAX_DOCUMENT_PAGES`]) shown by `page_action`.
    pub fn of<A>(_page_action: A, page_count: u32) -> Self
    where
        A: Action<Input = DocPage, Output = Result<String>>,
    {
        DocumentViewer {
            pages: Pages {
                meta: A::META,
                call: A::call,
                document: String::new(),
                count: page_count.clamp(1, MAX_DOCUMENT_PAGES),
                trusted: false,
            },
            search: None,
            size: PageSize::A4,
            title: None,
            outline: Vec::new(),
            downloads: Vec::new(),
        }
    }

    /// The document the actions get as [`DocPage::document`] and [`DocSearch::document`], such
    /// as a report id. At most 200 characters.
    pub fn document(mut self, id: impl Into<String>) -> Self {
        let id: String = id.into();
        let id: String = id.chars().take(200).collect();
        if let Some(s) = &mut self.search {
            s.document = id.clone();
        }
        self.pages.document = id;
        self
    }

    /// The size of every page; [`PageSize::A4`] unless set.
    pub fn page_size(mut self, size: PageSize) -> Self {
        self.size = size;
        self
    }

    /// The name of the document for screen readers and the toolbar.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sends pages to the browser as the action returns them, without cleaning them.
    ///
    /// Only for pages the app renders itself from data it controls, such as a report template
    /// filled with escaped values. Any text of a user that reaches a trusted page unescaped runs
    /// as script in the browsers of everyone who opens the document, with their session. The
    /// size limit still applies.
    pub fn trusted(mut self) -> Self {
        self.pages.trusted = true;
        self
    }

    /// An outline panel: entries that go to their page, with entries below them. At most 1,000
    /// entries and 6 levels; entries outside the document are left out.
    pub fn outline(mut self, items: Vec<OutlineItem>) -> Self {
        self.outline = items;
        self
    }

    /// A search box: the action gets a [`DocSearch`] 300 ms after the last key and returns the
    /// pages that match, of which at most [`MAX_PAGE_HITS`] are shown.
    pub fn search<S>(mut self, _search: S) -> Self
    where
        S: Action<Input = DocSearch, Output = Result<Vec<PageHit>>>,
    {
        self.search = Some(Search {
            meta: S::META,
            call: S::call,
            document: self.pages.document.clone(),
            count: self.pages.count,
        });
        self
    }

    /// A download link in the toolbar, such as the PDF export of the report. `href` is a path of
    /// the app; the link downloads without leaving the screen.
    pub fn download(mut self, label: impl Into<Text>, href: impl Into<String>) -> Self {
        self.downloads
            .push((label.into(), href.into()));
        self
    }

    fn texts(&self, r: &Render) -> Json {
        let t = |k: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.document.{k}")))
        };
        json!({
            "position": t("position"),
            "page": t("page_label"),
            "loading": t("loading"),
            "error": t("error"),
            "hits": t("hits"),
            "none": t("no_hits"),
            "more": t("more_hits"),
            "hit": t("hit"),
        })
    }

    fn outline_into(
        &self,
        out: &mut String,
        items: &[OutlineItem],
        depth: usize,
        left: &mut usize,
    ) {
        out.push_str(r#"<ul role="list">"#);
        for item in items {
            if *left == 0 {
                break;
            }
            if !(1..=self.pages.count).contains(&item.page) {
                continue;
            }
            *left -= 1;
            out.push_str(&format!(
                r#"<li><button type="button" class="z-doc-goto" data-page="{}">{}</button>"#,
                item.page,
                escape(&item.title),
            ));
            if !item.children.is_empty() && depth + 1 < MAX_OUTLINE_DEPTH {
                self.outline_into(out, &item.children, depth + 1, left);
            }
            out.push_str("</li>");
        }
        out.push_str("</ul>");
    }

    fn button(&self, out: &mut String, r: &Render, what: &str, icon: Icon, key: &str) {
        let label = r.text(&Text::key(format!("zeyn.ui.document.{key}")));
        out.push_str(&format!(
            r#"<button type="button" class="z-button z-ghost z-doc-button" data-doc="{what}" aria-label="{label}" title="{label}" data-testid="{}.doc.{what}">{}</button>"#,
            r.scope,
            icon.html(),
        ));
    }
}

impl Component for DocumentViewer {
    fn render(&self, out: &mut String, r: &Render) {
        if !r.can(&self.pages.meta.access) {
            return;
        }
        let id = dom_id(r, "doc");
        let t = |k: &str| r.text(&Text::key(format!("zeyn.ui.document.{k}")));
        let title = self
            .title
            .as_ref()
            .map_or_else(|| t("label"), |x| r.text(x));
        let (w, h) = self.size.css();
        let count = self.pages.count;
        out.push_str(&format!(
            r#"<section class="z-doc" data-zeyn-doc data-pages="{count}" data-width="{w:.2}" data-height="{h:.2}" data-texts="{texts}" aria-label="{title}" data-testid="{scope}.doc">"#,
            texts = attr(&self.texts(r)),
            scope = r.scope,
        ));
        out.push_str(&format!(
            r#"<div class="z-doc-bar" role="group" aria-label="{}">"#,
            t("tools")
        ));
        self.button(out, r, "prev", Icon::ChevronLeft, "previous");
        out.push_str(&format!(
            r#"<span class="z-doc-where"><label for="{id}-page">{page}</label><input id="{id}-page" class="z-doc-input" type="text" inputmode="numeric" autocomplete="off" value="1" size="4" maxlength="5" data-doc="page" aria-describedby="{id}-count"><span id="{id}-count">{of}</span></span>"#,
            page = t("page"),
            of = escape(
                &r.i18n
                    .text(r.locale, &Text::key("zeyn.ui.document.of"))
                    .replace("{count}", &count.to_string())
            ),
        ));
        self.button(out, r, "next", Icon::ChevronRight, "next");
        out.push_str(r#"<span class="z-doc-sep" aria-hidden="true"></span>"#);
        self.button(out, r, "out", Icon::Minus, "zoom_out");
        out.push_str(&format!(
            r#"<label class="z-sr-only" for="{id}-zoom">{}</label><select id="{id}-zoom" class="z-doc-zoom" data-doc="zoom"><option value="width" selected>{}</option><option value="page">{}</option>"#,
            t("zoom"),
            t("fit_width"),
            t("fit_page"),
        ));
        for z in ZOOMS {
            out.push_str(&format!(r#"<option value="{z}">{z}%</option>"#));
        }
        out.push_str("</select>");
        self.button(out, r, "in", Icon::Plus, "zoom_in");
        if !self.downloads.is_empty() {
            out.push_str(r#"<span class="z-doc-sep" aria-hidden="true"></span>"#);
        }
        for (label, href) in &self.downloads {
            out.push_str(&format!(
                r#"<a class="z-button z-ghost z-doc-download" href="{}" download>{}<span>{}</span></a>"#,
                escape(href),
                Icon::Download.html(),
                r.text(label),
            ));
        }
        out.push_str("</div>");
        let search = self
            .search
            .as_ref()
            .filter(|s| r.can(&s.meta.access));
        let side = !self.outline.is_empty() || search.is_some();
        out.push_str(r#"<div class="z-doc-body">"#);
        if side {
            out.push_str(r#"<div class="z-doc-side">"#);
            if search.is_some() {
                out.push_str(&format!(
                    r#"<div class="z-doc-search" role="search"><label for="{id}-q">{label}</label><input id="{id}-q" class="z-doc-input" type="search" autocomplete="off" maxlength="{max}" data-doc="search" aria-describedby="{id}-hits-status"><p class="z-doc-status" id="{id}-hits-status" role="status" aria-live="polite"></p><ul class="z-doc-hits" role="list" data-doc="hits"></ul></div>"#,
                    label = t("search"),
                    max = MAX_DOCUMENT_QUERY,
                ));
            }
            if !self.outline.is_empty() {
                out.push_str(&format!(
                    r#"<nav class="z-doc-outline" aria-label="{}">"#,
                    t("outline")
                ));
                let mut left = MAX_OUTLINE;
                self.outline_into(out, &self.outline, 0, &mut left);
                out.push_str("</nav>");
            }
            out.push_str("</div>");
        }
        out.push_str(&format!(
            r#"<div class="z-doc-view" id="{id}-view" tabindex="0" role="region" aria-label="{pages}" aria-describedby="{id}-keys" data-doc="view"><div class="z-doc-pages" data-doc="pages"></div></div></div><p class="z-sr-only" id="{id}-keys">{keys}</p><p class="z-sr-only" role="status" aria-live="polite" data-doc="status"></p></section>"#,
            pages = t("pages"),
            keys = t("keys"),
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "document_viewer",
            "action": self.pages.meta,
            "document": self.pages.document,
            "pages": self.pages.count,
            "page_size": { "width": self.size.width, "height": self.size.height },
            "trusted": self.pages.trusted,
            "search": self.search.as_ref().map(|s| s.meta),
            "outline": self.outline.len(),
            "downloads": self.downloads.len(),
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        match key {
            "page" => Some(&self.pages),
            "search" => self
                .search
                .as_ref()
                .map(|s| s as &dyn DataSource),
            _ => None,
        }
    }
}

impl From<DocumentViewer> for Node {
    fn from(v: DocumentViewer) -> Self {
        Node::new(v)
    }
}
