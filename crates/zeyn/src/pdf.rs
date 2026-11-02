//! PDF documents from Typst templates, behind the `pdf` Cargo feature.
//!
//! A template is Typst source. Data from an action reaches it only as a Typst value: the data is
//! serialized to JSON and the template reads it with `data`, which the renderer binds as
//! `#let data = json("/data.json")` before the template. User data is never spliced into the
//! source, so a value such as `#panic()` or `]` stays text.
//!
//! ```ignore
//! const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("invoice.typ"));
//!
//! #[action(permission = "billing.invoice.read")]
//! pub async fn invoice_data(p: InvoiceParams, store: Invoices) -> Result<InvoiceData> { ... }
//!
//! Screen::new("Invoice").body(PdfLink::new("Download PDF", INVOICE, invoice_data, params))
//! ```
//!
//! Rendering runs on a blocking thread, at most [`Pdf::max_concurrent`] documents at a time.
//! Libertinus Serif and DejaVu Sans Mono are embedded in the binary (1.6 MB), and `pdf.fonts` in the configuration names a
//! directory with more `.ttf`, `.otf` or `.ttc` files.

use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome, Service};
use crate::i18n::Text;
use crate::platform::Config;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst_layout::PagedDocument;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// Largest data a document may get, in bytes of JSON.
pub const MAX_PDF_DATA: usize = 16 * 1024 * 1024;
/// Most rows a grid's PDF list report holds; bigger results are cut there.
pub const MAX_PDF_ROWS: usize = 2_000;

/// A Typst template with a name used for file names and errors.
#[derive(Clone, Copy, Debug)]
pub struct PdfTemplate {
    pub name: &'static str,
    pub source: &'static str,
}

impl PdfTemplate {
    pub const fn new(name: &'static str, source: &'static str) -> Self {
        PdfTemplate { name, source }
    }

    /// The list report grids use for `Export::Pdf`. Its data is `{ title, lang, columns: [{ label,
    /// numeric }], rows: [[text]], note }`.
    pub const LIST: PdfTemplate = PdfTemplate::new("list", include_str!("pdf/list.typ"));
}

/// Libertinus Serif in four styles (SIL Open Font License, see `assets/fonts/OFL.txt`), which
/// covers Latin, Greek and Cyrillic, Turkish included, and DejaVu Sans Mono (see
/// `assets/fonts/DEJAVU-LICENSE.txt`) as the fallback for signs Libertinus lacks, such as `₺`.
const EMBEDDED: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/LibertinusSerif-Regular.otf"),
    include_bytes!("../assets/fonts/LibertinusSerif-Bold.otf"),
    include_bytes!("../assets/fonts/LibertinusSerif-Italic.otf"),
    include_bytes!("../assets/fonts/LibertinusSerif-BoldItalic.otf"),
    include_bytes!("../assets/fonts/DejaVuSansMono.ttf"),
];

struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    permits: tokio::sync::Semaphore,
    max: usize,
}

/// Renders Typst templates to PDF. A service: register one built from the configuration with
/// `App::service(Pdf::from_config(&config)?)`, or the first use builds a default one with the
/// embedded fonts.
#[derive(Clone)]
pub struct Pdf(Arc<Shared>);

impl Service for Pdf {}

impl Default for Pdf {
    fn default() -> Self {
        Pdf::build(Vec::new(), default_limit())
    }
}

fn default_limit() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).clamp(1, 2))
}

impl Pdf {
    /// The embedded fonts and a limit of two documents at a time (one on a single CPU).
    pub fn new() -> Pdf {
        Pdf::default()
    }

    /// Reads `pdf.fonts` (a directory of extra fonts) and `pdf.max_concurrent` (documents
    /// rendered at once).
    pub fn from_config(config: &Config) -> Result<Pdf> {
        let mut extra = Vec::new();
        if let Some(dir) = config.get("pdf.fonts") {
            let entries = std::fs::read_dir(dir).map_err(|e| Error::Internal(format!("pdf.fonts: {e}")))?;
            for entry in entries.flatten() {
                let path = entry.path();
                let font = path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"));
                if font && let Ok(data) = std::fs::read(&path) {
                    extra.extend(Font::iter(Bytes::new(data)));
                }
            }
        }
        let max = match config.get("pdf.max_concurrent") {
            Some(n) => n.parse::<usize>().ok().filter(|n| (1..=64).contains(n)).ok_or_else(|| Error::Internal("pdf.max_concurrent must be 1 to 64".into()))?,
            None => default_limit(),
        };
        Ok(Pdf::build(extra, max))
    }

    fn build(extra: Vec<Font>, max: usize) -> Pdf {
        let mut fonts: Vec<Font> = EMBEDDED.iter().flat_map(|data| Font::iter(Bytes::new(*data))).collect();
        fonts.extend(extra);
        let book = FontBook::from_fonts(&fonts);
        Pdf(Arc::new(Shared {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(book),
            fonts,
            permits: tokio::sync::Semaphore::new(max),
            max,
        }))
    }

    /// The service registered for this request, or a shared default one.
    pub fn of(ctx: &Ctx) -> Pdf {
        static DEFAULT: OnceLock<Pdf> = OnceLock::new();
        ctx.service::<Pdf>().unwrap_or_else(|| DEFAULT.get_or_init(Pdf::default).clone())
    }

    /// Most documents rendered at the same time.
    pub fn max_concurrent(&self) -> usize {
        self.0.max
    }

    /// Names of the font families templates may use.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.0.book.families().map(|(name, _)| name.to_string()).collect();
        names.dedup();
        names
    }

    /// Renders `template` with `data` on a blocking thread, waiting for a free slot first.
    pub async fn render(&self, template: PdfTemplate, data: &impl Serialize) -> Result<Vec<u8>> {
        let data = serde_json::to_vec(data).map_err(|e| Error::Internal(format!("pdf data: {e}")))?;
        if data.len() > MAX_PDF_DATA {
            return Err(Error::Internal(format!("pdf data for `{}` is larger than {MAX_PDF_DATA} bytes", template.name)));
        }
        let _permit = self.0.permits.acquire().await.map_err(|_| Error::Internal("pdf renderer closed".into()))?;
        let pdf = self.clone();
        tokio::task::spawn_blocking(move || pdf.render_now(template, data)).await.map_err(|e| Error::Internal(format!("pdf: {e}")))?
    }

    /// Renders on the calling thread without the concurrency limit. `data` is JSON.
    pub fn render_now(&self, template: PdfTemplate, data: Vec<u8>) -> Result<Vec<u8>> {
        let world = DocWorld::new(&self.0, template, data);
        let compiled = typst::compile::<PagedDocument>(&world).output;
        let result = compiled
            .map_err(|errors| Error::Internal(format!("pdf `{}`: {}", template.name, errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; "))))
            .and_then(|doc| {
                typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default())
                    .map_err(|errors| Error::Internal(format!("pdf `{}`: {}", template.name, errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; "))))
            });
        typst::comemo::evict(0);
        result
    }
}

/// Fixed paths of the two files a document sees.
fn ids() -> (FileId, FileId) {
    static IDS: OnceLock<(FileId, FileId)> = OnceLock::new();
    *IDS.get_or_init(|| {
        let id = |p: &str| RootedPath::new(VirtualRoot::Project, VirtualPath::new(p).expect("valid path")).intern();
        (id("/main.typ"), id("/data.json"))
    })
}

/// The world of one document: the template, its data and the fonts. Nothing else can be read,
/// no file system and no packages.
struct DocWorld<'a> {
    shared: &'a Shared,
    main: Source,
    data: Bytes,
}

impl<'a> DocWorld<'a> {
    fn new(shared: &'a Shared, template: PdfTemplate, data: Vec<u8>) -> Self {
        let (main, _) = ids();
        let source = format!("#let data = json(\"/data.json\")\n{}", template.source);
        DocWorld { shared, main: Source::new(main, source), data: Bytes::new(data) }
    }
}

impl World for DocWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.shared.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.shared.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() { Ok(self.main.clone()) } else { Err(FileError::AccessDenied) }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == ids().1 { Ok(self.data.clone()) } else { Err(FileError::AccessDenied) }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.shared.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        let d = crate::time::DateTime::now().local(crate::time::TimeZone::UTC).date;
        Datetime::from_ymd(d.year(), d.month(), d.day())
    }
}

/// A PDF document on a screen, as the server sees it.
pub trait PdfSource: Send + Sync {
    /// The action that loads the document's data.
    fn action(&self) -> ActionMeta;

    /// The template and the data, loaded through the action for the user of `ctx`.
    fn load(&self, ctx: Ctx) -> BoxFuture<Result<(PdfTemplate, Json)>>;
}

/// A download button for a PDF document rendered from `template` with the data an action
/// returns for `input`. The download needs the action's access and goes through interceptors.
pub struct PdfLink<A: Action> {
    label: Text,
    template: PdfTemplate,
    input: A::Input,
    _action: PhantomData<fn() -> A>,
}

impl<A, T> PdfLink<A>
where
    A: Action<Output = Result<T>>,
    A::Input: Clone + Send + Sync + Serialize,
    T: Serialize + Send + 'static,
{
    pub fn new(label: impl Into<Text>, template: PdfTemplate, _action: A, input: A::Input) -> Self {
        PdfLink { label: label.into(), template, input, _action: PhantomData }
    }
}

impl<A, T> PdfSource for PdfLink<A>
where
    A: Action<Output = Result<T>>,
    A::Input: Clone + Send + Sync + Serialize,
    T: Serialize + Send + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn load(&self, ctx: Ctx) -> BoxFuture<Result<(PdfTemplate, Json)>> {
        let (input, template) = (self.input.clone(), self.template);
        Box::pin(async move {
            let data = A::call(ctx, input).await??;
            let data = serde_json::to_value(data).map_err(|e| Error::Internal(format!("pdf data: {e}")))?;
            Ok((template, data))
        })
    }
}

impl<A, T> Component for PdfLink<A>
where
    A: Action<Output = Result<T>>,
    A::Input: Clone + Send + Sync + Serialize,
    T: Serialize + Send + 'static,
    Result<T>: Outcome,
{
    fn render(&self, out: &mut String, r: &Render) {
        if r.identity.is_some_and(|i| !i.can(&A::META.access)) {
            return;
        }
        out.push_str(&format!(
            r#"<button type="button" class="z-button" data-zeyn-pdf data-testid="{}.pdf.{}">{}</button>"#,
            r.scope,
            escape(self.template.name),
            r.text(&self.label)
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "pdf", "template": self.template.name, "action": A::META, "input": self.input })
    }

    fn pdf(&self) -> Option<&dyn PdfSource> {
        Some(self)
    }
}

impl<A, T> From<PdfLink<A>> for Node
where
    A: Action<Output = Result<T>> + 'static,
    A::Input: Clone + Send + Sync + Serialize,
    T: Serialize + Send + 'static,
    Result<T>: Outcome,
{
    fn from(link: PdfLink<A>) -> Self {
        Node::new(link)
    }
}

/// Language code Typst understands for a locale such as `tr-TR`; `en` when it has none.
pub(crate) fn lang(locale: &str) -> String {
    let code: String = locale.chars().take_while(char::is_ascii_alphabetic).collect::<String>().to_ascii_lowercase();
    if (2..=3).contains(&code.len()) { code } else { "en".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typst::layout::{Frame, FrameItem};

    fn text_of(frame: &Frame, out: &mut String) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Group(g) => text_of(&g.frame, out),
                FrameItem::Text(t) => out.push_str(&t.text),
                _ => {}
            }
        }
    }

    /// The text a template prints for `data`, without white space.
    fn printed(template: &'static str, data: Json) -> std::result::Result<String, String> {
        let pdf = Pdf::new();
        let world = DocWorld::new(&pdf.0, PdfTemplate::new("t", template), serde_json::to_vec(&data).unwrap());
        let doc = typst::compile::<PagedDocument>(&world)
            .output
            .map_err(|e| e.iter().map(|e| e.message.to_string()).collect::<Vec<_>>().join("; "))?;
        let mut out = String::new();
        for page in doc.pages() {
            text_of(&page.frame, &mut out);
        }
        Ok(out.replace(char::is_whitespace, ""))
    }

    #[test]
    fn user_data_is_printed_as_text_and_never_runs_as_typst() {
        let evil = "#panic(\"pwned\") ] #read(\"/etc/passwd\") $x$ *bold* \" \\ <x> @y // /* #let";
        let plain = evil.replace(char::is_whitespace, "");
        assert_eq!(printed("#data.name", json!({ "name": evil })).unwrap(), plain);
        let list = json!({ "title": evil, "lang": "tr", "note": evil, "columns": [{ "label": evil, "numeric": true }], "rows": [[evil], [evil]] });
        let text = printed(PdfTemplate::LIST.source, list).unwrap();
        assert!(text.matches(plain.as_str()).count() >= 5, "{text}");
    }

    #[test]
    fn templates_see_only_their_data() {
        let err = printed("#read(\"/main.typ\")", json!({})).unwrap_err();
        assert!(err.contains("denied"), "{err}");
        assert!(printed("#read(\"../../Cargo.toml\")", json!({})).is_err());
        assert!(printed("#import \"@preview/cetz:0.3.0\": *", json!({})).is_err());
    }

    #[test]
    fn embedded_fonts_cover_turkish_text() {
        assert!(Pdf::new().families().iter().any(|f| f.eq_ignore_ascii_case("libertinus serif")), "{:?}", Pdf::new().families());
        let text = printed("#set text(font: \"Libertinus Serif\")\n#data.t", json!({ "t": "Çağrı Şükrü İĞÖÜ ıi 1.234,56 ₺" })).unwrap();
        assert_eq!(text, "ÇağrıŞükrüİĞÖÜıi1.234,56₺");
        assert_eq!((lang("tr-TR"), lang("en"), lang("*")), ("tr".to_string(), "en".to_string(), "en".to_string()));
    }
}
