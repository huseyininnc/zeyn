//! Content from other sites: framed pages ([`Embed`]), images ([`Image`]) and links
//! ([`ExternalLink`]).
//!
//! Every outside address is checked against the app's content security policy (`App::csp`,
//! read from the request context) before anything is rendered, so a host the browser would block
//! never reaches the page. The browser side lives in `media.js` and `media.css`.

use super::Alert;
use crate::csp::{Csp, EMPTY, embed_url};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

/// An [`Embed`] without its title yet. Call `.title(t!(...))`.
pub struct NoTitle;

/// An [`Image`] without its alt text yet. Call `.alt(t!(...))`.
pub struct NoAlt;

/// An [`Image`] without its size yet. Call `.size(width, height)`.
pub struct NoSize;

/// A required part of a component is set.
pub struct Given;

/// A component built with all its required parts. Only finished components can be placed.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is missing a required part and cannot be placed on a screen",
    label = "not finished",
    note = "an `Embed` needs `.title(t!(...))`; an `Image` needs `.alt(t!(...))` and `.size(width, height)`",
    note = "titles and alt texts are read by screen readers, and the size keeps the page from jumping while it loads"
)]
pub trait Complete: Component {}

/// The policy of the request, or the strict default when rendering outside one.
pub(super) fn policy<'a>(r: &Render<'a>) -> &'a Csp {
    r.ctx.map(|c| c.csp()).unwrap_or(&EMPTY)
}

/// Host of an `https` URL, for messages.
fn host_of(url: &str) -> &str {
    let rest = url.get(8..).unwrap_or("");
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or("")
}

/// What a framed page may use besides running scripts, written to the `allow` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedPermission {
    Fullscreen,
    PictureInPicture,
    Autoplay,
    EncryptedMedia,
}

impl EmbedPermission {
    fn feature(self) -> &'static str {
        match self {
            EmbedPermission::Fullscreen => "fullscreen",
            EmbedPermission::PictureInPicture => "picture-in-picture",
            EmbedPermission::Autoplay => "autoplay",
            EmbedPermission::EncryptedMedia => "encrypted-media",
        }
    }
}

/// The fixed sandbox of every [`Embed`]. Popups, top navigation, forms and downloads are never
/// allowed, so a framed page cannot leave the frame or open other windows.
pub const EMBED_SANDBOX: &str = "allow-scripts allow-same-origin allow-presentation";

/// A page from another site in a locked frame, such as a video player or a map.
///
/// ```
/// # use zeyn::ui::{Embed, EmbedPermission};
/// # use zeyn::Text;
/// let video = Embed::url("https://www.youtube-nocookie.com/embed/abc")
///     .title(Text::from("Product tour"))
///     .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
///     .aspect(16, 9)
///     .click_to_load("/static/tour.jpg");
/// ```
///
/// The frame gets `sandbox="allow-scripts allow-same-origin allow-presentation"`,
/// `referrerpolicy="no-referrer"` and `loading="lazy"`. When the address is not `https` or its
/// host is not in the app's `frame-src` (`App::csp(|c| c.frame_src([...]))`), no frame is
/// rendered at all; a warning says why. With [`Embed::click_to_load`] the other site is not
/// contacted until the user presses the poster button.
///
/// A title is required, because screen readers announce frames by it:
///
/// ```compile_fail
/// # use zeyn::ui::{Embed, Stack};
/// let stack = Stack::new().add(Embed::url("https://player.example.com/v/1"));
/// ```
pub struct Embed<T = Given> {
    url: String,
    title: Text,
    allow: Vec<EmbedPermission>,
    aspect: (u32, u32),
    poster: Option<String>,
    notice: Option<Text>,
    remember: Option<crate::prefs::ClientPref<crate::prefs::Consent>>,
    _title: PhantomData<T>,
}

impl Embed<NoTitle> {
    /// The address to frame: an `https` URL or pasted embed code, of which only `src` is used.
    pub fn url(url: impl Into<String>) -> Self {
        Embed {
            url: url.into(),
            title: Text::from(""),
            allow: Vec::new(),
            aspect: (16, 9),
            poster: None,
            notice: None,
            remember: None,
            _title: PhantomData,
        }
    }

    /// What the framed page is, for screen readers. Required.
    pub fn title(self, title: impl Into<Text>) -> Embed<Given> {
        Embed {
            url: self.url,
            title: title.into(),
            allow: self.allow,
            aspect: self.aspect,
            poster: self.poster,
            notice: self.notice,
            remember: self.remember,
            _title: PhantomData,
        }
    }
}

impl<T> Embed<T> {
    /// Browser features the framed page may use. None unless set.
    pub fn allow(mut self, permissions: impl IntoIterator<Item = EmbedPermission>) -> Self {
        for p in permissions {
            if !self.allow.contains(&p) {
                self.allow.push(p);
            }
        }
        self
    }

    /// Width to height ratio of the frame. 16:9 unless set; zeros are ignored.
    pub fn aspect(mut self, width: u32, height: u32) -> Self {
        if width > 0 && height > 0 {
            self.aspect = (width, height);
        }
        self
    }

    /// Shows a button first and loads the frame only when it is pressed, so the other site
    /// learns nothing about visitors who do not use it. `poster` is an image shown on the button:
    /// a path on the app or an `https` address on a host of `img-src`; pass `""` for none.
    pub fn click_to_load(mut self, poster: impl Into<String>) -> Self {
        self.poster = Some(poster.into());
        self
    }

    /// A text of the app shown on the click to load button, such as a content warning. It is part
    /// of the button's accessible name, so screen readers read it before the button is pressed.
    /// Without [`Embed::click_to_load`] it turns the button on with no poster.
    ///
    /// ```
    /// # use zeyn::ui::Embed;
    /// # use zeyn::Text;
    /// let tour = Embed::url("https://video.example.com/embed/tour")
    ///     .title(Text::from("Product tour"))
    ///     .click_to_load("")
    ///     .notice(Text::from("This video is served by another site."));
    /// ```
    pub fn notice(mut self, notice: impl Into<Text>) -> Self {
        self.notice = Some(notice.into());
        self.poster
            .get_or_insert_with(String::new);
        self
    }

    /// Remembers that the user pressed the click to load button, under a consent key declared
    /// with [`crate::ClientPref::consent`] and registered with `App::client_prefs`, so embeds
    /// bound to the same key load at once on later pages and in other tabs for the key's
    /// lifetime. Use it for a notice that should be confirmed once in a while, not on every page
    /// that shows such an embed.
    ///
    /// ```
    /// # use zeyn::ui::Embed;
    /// # use zeyn::{ClientPref, Consent, Purpose, Text};
    /// # use std::time::Duration;
    /// pub const EXTERNAL_VIDEO: ClientPref<Consent> =
    ///     ClientPref::consent("external-video", Duration::from_secs(24 * 60 * 60), Purpose::Necessary);
    ///
    /// let tour = Embed::url("https://video.example.com/embed/tour")
    ///     .title(Text::from("Product tour"))
    ///     .notice(Text::from("This video is served by another site."))
    ///     .remember(EXTERNAL_VIDEO);
    /// ```
    ///
    /// The browser keeps the record in `zeyn.state` (see [`crate::ClientPref`]); no cookie is set
    /// and the server never sees it, so cached public pages stay the same for everyone. A
    /// remembered frame loads without moving focus (WCAG 2.2 3.2.1 On Focus and 2.4.3 Focus
    /// Order); focus moves to the frame only after a real press. The button stays when the
    /// record has ended or does not pass its checks, when the app did not register the key, and
    /// without JavaScript. Without [`Embed::click_to_load`] it turns the button on with no poster.
    pub fn remember(mut self, key: crate::prefs::ClientPref<crate::prefs::Consent>) -> Self {
        self.remember = Some(key);
        self.poster
            .get_or_insert_with(String::new);
        self
    }
}

impl Complete for Embed<Given> {}

impl<T> From<Embed<T>> for Node
where
    Embed<T>: Complete + 'static,
{
    fn from(e: Embed<T>) -> Self {
        Node::new(e)
    }
}

impl Component for Embed<Given> {
    fn render(&self, out: &mut String, r: &Render) {
        let title = r.text(&self.title);
        let csp = policy(r);
        let Some(url) = embed_url(&self.url).filter(|u| csp.allows_frame(u)) else {
            let text = Text::key("zeyn.ui.embed.blocked").arg("title", self.title.clone());
            Alert::warning(text).render(out, r);
            return;
        };
        let allow: Vec<&str> = self
            .allow
            .iter()
            .map(|p| p.feature())
            .collect();
        let allow = if allow.is_empty() {
            String::new()
        } else {
            format!(r#" allow="{}""#, allow.join("; "))
        };
        let frame = format!(
            r#"<iframe src="{}" title="{title}" sandbox="{EMBED_SANDBOX}"{allow} referrerpolicy="no-referrer" loading="lazy"></iframe>"#,
            escape(&url)
        );
        let (w, h) = self.aspect;
        let remember = self
            .remember
            .map(|k| k.attrs(r))
            .unwrap_or_default();
        out.push_str(&format!(
            r#"<div class="z-embed" style="--z-aspect: {w} / {h}"{remember} data-testid="{}.embed">"#,
            r.scope
        ));
        match &self.poster {
            None => out.push_str(&frame),
            Some(poster) => {
                let image = if !poster.is_empty() && csp.allows_image(poster) {
                    format!(
                        r#"<img src="{}" alt="" decoding="async" loading="lazy">"#,
                        escape(poster)
                    )
                } else {
                    String::new()
                };
                let notice = match &self.notice {
                    Some(n) => format!(r#"<span class="z-embed-notice">{}</span>"#, r.text(n)),
                    None => String::new(),
                };
                out.push_str(&format!(
                    r#"<button type="button" class="z-embed-load" data-zeyn-embed-load>{image}<span class="z-embed-play" aria-hidden="true"></span><span class="z-embed-label">{notice}<strong>{}</strong><small>{}</small></span></button><template data-zeyn-embed>{frame}</template>"#,
                    r.text(&Text::key("zeyn.ui.embed.load").arg("title", self.title.clone())),
                    r.text(&Text::key("zeyn.ui.embed.from").arg("host", host_of(&url).to_string())),
                ));
            }
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "embed",
            "url": embed_url(&self.url),
            "title": self.title,
            "allow": self.allow,
            "aspect": [self.aspect.0, self.aspect.1],
            "click_to_load": self.poster.is_some(),
            "notice": self.notice,
            "remember": self.remember.map(|k| k.info()),
        })
    }
}

/// An image with its alt text and size, so screen readers can read it and the page does not
/// move while it loads.
///
/// ```
/// # use zeyn::ui::Image;
/// # use zeyn::Text;
/// let logo = Image::new("https://images.example.com/logo.png")
///     .alt(Text::from("Northwind logo"))
///     .size(240, 80)
///     .lazy()
///     .srcset([("https://images.example.com/logo@2x.png", "2x")]);
/// ```
///
/// `src` is a path on the app (such as a public media source: `/_zeyn/media/brand/logo.png`, see
/// `Feature::public_media`) or an `https` address on a host of the
/// app's `img-src` (`App::csp(|c| c.img_src([...]))`). Any other image is not rendered; its alt
/// text is shown instead. `srcset` entries on other hosts, or with a descriptor that is not a
/// density (`2x`) or a width (`800w`), are left out. Every image gets `decoding="async"`;
/// `.lazy()` adds `loading="lazy"` for images below the fold. Pass `Text::from("")` as the alt
/// text for a purely decorative image.
///
/// For long lists of photos, such as a product catalog, the image can show something while it
/// loads and recover when it fails:
///
/// ```
/// # use zeyn::ui::{Fit, Image, Placeholder};
/// # use zeyn::Text;
/// let photo = Image::new("/img/p/42-800.webp")
///     .alt(Text::from("Blue linen shirt"))
///     .size(400, 500)
///     .lazy()
///     .srcset([("/img/p/42-400.webp", "400w"), ("/img/p/42-800.webp", "800w")])
///     .sizes("(min-width: 800px) 25vw, 50vw")
///     .placeholder(Placeholder::color("#d9dde3"))
///     .low("/img/p/42-40.webp")
///     .fallback("/img/no-photo.webp")
///     .on_error(Text::from("No photo yet"))
///     .fit(Fit::Cover);
/// ```
///
/// The image then sits in a box of its size that shows the [`Placeholder`] and the
/// [`Image::low`] file under it, both without script. The browser loads the full file natively
/// (`loading`, `srcset`, `sizes`); one listener of the media module, shared by every image on the
/// page, fades it in, tries the [`Image::fallback`] once and finally shows the
/// [`Image::on_error`] content. [`Image::priority`] marks the first image a visitor sees.
///
/// Both the alt text and the size are required; an image without them cannot be placed:
///
/// ```compile_fail
/// # use zeyn::ui::{Image, Stack};
/// # use zeyn::Text;
/// let stack = Stack::new().add(Image::new("/logo.png").alt(Text::from("Logo")));
/// ```
///
/// ```compile_fail
/// # use zeyn::ui::{Image, Stack};
/// let stack = Stack::new().add(Image::new("/logo.png").size(240, 80));
/// ```
pub struct Image<A = Given, S = Given> {
    src: String,
    alt: Text,
    size: (u32, u32),
    lazy: bool,
    srcset: Vec<(String, String)>,
    sizes: Option<String>,
    loading: Loading,
    _parts: PhantomData<(A, S)>,
}

impl Image<NoAlt, NoSize> {
    pub fn new(src: impl Into<String>) -> Self {
        Image {
            src: src.into(),
            alt: Text::from(""),
            size: (0, 0),
            lazy: false,
            srcset: Vec::new(),
            sizes: None,
            loading: Loading::default(),
            _parts: PhantomData,
        }
    }
}

impl<A, S> Image<A, S> {
    fn into_parts<B, T>(self) -> Image<B, T> {
        Image {
            src: self.src,
            alt: self.alt,
            size: self.size,
            lazy: self.lazy,
            srcset: self.srcset,
            sizes: self.sizes,
            loading: self.loading,
            _parts: PhantomData,
        }
    }

    /// Loads the image only when it comes near the viewport.
    pub fn lazy(mut self) -> Self {
        self.lazy = true;
        self
    }

    /// Other files of the same image, with a density (`2x`) or width (`800w`) descriptor.
    pub fn srcset<'a>(mut self, candidates: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        self.srcset.extend(
            candidates
                .into_iter()
                .map(|(u, d)| (u.to_string(), d.to_string())),
        );
        self
    }

    /// The width the image takes in the layout, for the browser to pick a `srcset` width
    /// (`800w`) file, such as `"(min-width: 800px) 25vw, 50vw"`. Without it a lazy image lets
    /// the browser measure the slot (`sizes="auto"`) and falls back to the image's own width.
    /// Values longer than 200 characters or with `<`, `>`, quotes or backslashes are ignored.
    pub fn sizes(mut self, sizes: impl Into<String>) -> Self {
        let sizes = sizes.into();
        self.sizes = Some(sizes).filter(|s| {
            !s.is_empty() && s.len() <= 200 && !s.contains(['<', '>', '"', '\'', '`', '\\'])
        });
        self
    }

    /// What shows in the image's box until the file has loaded: a color, a tiny blurred image
    /// or a shared image. See [`Placeholder`].
    pub fn placeholder(mut self, placeholder: Placeholder) -> Self {
        self.loading.placeholder = Some(placeholder);
        self
    }

    /// A small, low resolution file of the same image, shown at once under the full one. The
    /// full file loads at the same time (lazily when the image is lazy) and covers it with a
    /// short fade when it is ready. Same address rules as the image.
    pub fn low(mut self, src: impl Into<String>) -> Self {
        self.loading.low = Some(src.into());
        self
    }

    /// Another file tried once when the image fails to load, such as a generic product photo.
    /// Same address rules as the image.
    pub fn fallback(mut self, src: impl Into<String>) -> Self {
        self.loading.fallback = Some(src.into());
        self
    }

    /// What shows when the image and its fallback both fail: a text or a component. Without it
    /// the alt text is shown with an icon on the placeholder background.
    pub fn on_error(mut self, content: impl Into<Node>) -> Self {
        self.loading.on_error = Some(content.into());
        self
    }

    /// Marks the first image the visitor sees (the largest paint): it loads at once with
    /// `fetchpriority="high"` and is never lazy. Use it for one image per page.
    pub fn priority(mut self) -> Self {
        self.loading.priority = true;
        self
    }

    /// Drops the low resolution file, so nothing competes with an image that must load first.
    pub(crate) fn without_low(mut self) -> Self {
        self.loading.low = None;
        self
    }

    /// How the file fills the box when its shape differs from the size: cut to fill
    /// ([`Fit::Cover`]) or shown whole ([`Fit::Contain`]).
    pub fn fit(mut self, fit: Fit) -> Self {
        self.loading.fit = Some(fit);
        self
    }
}

impl<S> Image<NoAlt, S> {
    /// What the image shows, for screen readers and when it cannot load. Required.
    pub fn alt(mut self, alt: impl Into<Text>) -> Image<Given, S> {
        self.alt = alt.into();
        self.into_parts()
    }
}

impl<A> Image<A, NoSize> {
    /// Width and height in CSS pixels. Required, so the browser keeps the space while it loads.
    pub fn size(mut self, width: u32, height: u32) -> Image<A, Given> {
        self.size = (width, height);
        self.into_parts()
    }
}

/// Whether a `srcset` descriptor is a density (`2x`, `1.5x`) or a width (`800w`).
fn valid_descriptor(d: &str) -> bool {
    match d.strip_suffix('x') {
        Some(n) => {
            !n.starts_with('.')
                && n.bytes()
                    .all(|b| b.is_ascii_digit() || b == b'.')
                && n.parse::<f32>()
                    .is_ok_and(|v| v > 0.0 && v <= 8.0)
        }
        None => d.strip_suffix('w').is_some_and(|n| {
            !n.is_empty() && n.len() <= 5 && n.bytes().all(|b| b.is_ascii_digit())
        }),
    }
}

impl Complete for Image<Given, Given> {}

impl<A, S> From<Image<A, S>> for Node
where
    Image<A, S>: Complete + 'static,
{
    fn from(i: Image<A, S>) -> Self {
        Node::new(i)
    }
}

impl Component for Image<Given, Given> {
    fn render(&self, out: &mut String, r: &Render) {
        let csp = policy(r);
        let alt = r.text(&self.alt);
        let (w, h) = self.size;
        if !csp.allows_image(&self.src) {
            out.push_str(&format!(
                r#"<span class="z-image-blocked" data-testid="{}.image">{alt}</span>"#,
                r.scope
            ));
            return;
        }
        let candidates: Vec<String> = self
            .srcset
            .iter()
            .filter(|(u, d)| !u.contains([' ', ',']) && csp.allows_image(u) && valid_descriptor(d))
            .map(|(u, d)| format!("{} {d}", escape(u)))
            .collect();
        let lazy = self.lazy && !self.loading.priority;
        let srcset = if candidates.is_empty() {
            String::new()
        } else {
            let sizes = match &self.sizes {
                Some(s) => format!(r#" sizes="{}""#, escape(s)),
                None if candidates
                    .iter()
                    .any(|c| c.ends_with('w')) =>
                {
                    let auto = if lazy { "auto, " } else { "" };
                    format!(r#" sizes="{auto}(max-width: {w}px) 100vw, {w}px""#)
                }
                None => String::new(),
            };
            format!(r#" srcset="{}"{sizes}"#, candidates.join(", "))
        };
        let wrap = self.loading.wraps();
        if wrap {
            out.push_str(&format!(
                r#"<span class="z-img{}" style="width:{w}px;{}">{}"#,
                self.loading.host_class(),
                self.loading.host_style(self.size),
                self.loading.layers(r, lazy)
            ));
        }
        out.push_str(&format!(
            r#"<img class="z-image" src="{}" alt="{alt}" width="{w}" height="{h}" decoding="async"{}{srcset} data-testid="{}.image">"#,
            escape(&self.src),
            self.loading.img_attrs(r, lazy),
            r.scope
        ));
        if wrap {
            out.push_str(&self.loading.error(r));
            out.push_str("</span>");
        }
    }

    fn schema(&self) -> Json {
        json!({
            "type": "image",
            "src": self.src,
            "alt": self.alt,
            "width": self.size.0,
            "height": self.size.1,
            "lazy": self.lazy,
            "sizes": self.sizes,
            "loading": self.loading.schema(),
        })
    }
}

/// The longest [`Placeholder::blur`] data URI, in bytes. A 16 by 16 pixel WebP or AVIF fits
/// easily; anything longer belongs in a file.
pub const PLACEHOLDER_BLUR_MAX: usize = 600;

/// How an image fills its box when the file's shape differs from the size, set with
/// [`Image::fit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Fills the box and cuts what overflows, as product photos in a grid.
    Cover,
    /// Shows the whole file inside the box, leaving the rest to the placeholder.
    Contain,
}

/// What an image's box shows until the file has loaded. It is a CSS background or a layer under
/// the image, so it needs no script, and the box keeps its size either way.
///
/// ```
/// # use zeyn::ui::Placeholder;
/// let gray = Placeholder::color("#d9dde3");
/// let themed = Placeholder::color("--z-surface-2");
/// let tiny = Placeholder::blur("data:image/webp;base64,UklGRhoAAABXRUJQVlA4TA0AAAAvAAAAEAcQERGIiP4HAA==");
/// let shared = Placeholder::image("/static/product-outline.png");
/// ```
///
/// A color is a hex color (`#rgb`, `#rrggbb`, with or without alpha) or a theme token
/// (`--z-surface-2`, see `docs/theming.md`), so it can follow the light and the dark theme. A
/// blur is a tiny image the app made, as a base64 `data:image/webp`, `avif`, `jpeg` or `png` URI
/// of at most [`PLACEHOLDER_BLUR_MAX`] bytes; it is stretched and blurred. SVG is never accepted.
/// An image follows the address rules of [`Image`]. A placeholder that breaks a rule is left
/// out, and the box shows the theme's surface color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder(Fill);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Fill {
    Color(String),
    Blur(String),
    Image(String),
    Invalid,
}

impl Placeholder {
    /// A plain color: `#d9dde3` or a theme token such as `--z-surface-2`.
    pub fn color(color: &str) -> Self {
        let hex = color
            .strip_prefix('#')
            .is_some_and(|h| {
                matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
            });
        let token = color
            .strip_prefix("--z-")
            .is_some_and(|t| {
                !t.is_empty()
                    && t.len() <= 40
                    && t.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            });
        match (hex, token) {
            (true, _) => Placeholder(Fill::Color(color.to_string())),
            (_, true) => Placeholder(Fill::Color(format!("var({color})"))),
            _ => Placeholder(Fill::Invalid),
        }
    }

    /// A tiny image, blurred over the box: a base64 `data:image/webp`, `avif`, `jpeg` or `png`
    /// URI of at most [`PLACEHOLDER_BLUR_MAX`] bytes.
    pub fn blur(data_uri: &str) -> Self {
        let body = ["webp", "avif", "jpeg", "png"]
            .iter()
            .find_map(|t| data_uri.strip_prefix(&format!("data:image/{t};base64,")));
        let ok = data_uri.len() <= PLACEHOLDER_BLUR_MAX
            && body.is_some_and(|b| {
                !b.is_empty()
                    && b.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
            });
        match ok {
            true => Placeholder(Fill::Blur(data_uri.to_string())),
            false => Placeholder(Fill::Invalid),
        }
    }

    /// A file shown under the image while it loads, such as one outline shared by every product
    /// so the browser fetches it once. Same address rules as the image.
    pub fn image(src: impl Into<String>) -> Self {
        Placeholder(Fill::Image(src.into()))
    }

    fn kind(&self) -> &'static str {
        match self.0 {
            Fill::Color(_) => "color",
            Fill::Blur(_) => "blur",
            Fill::Image(_) => "image",
            Fill::Invalid => "none",
        }
    }
}

/// The loading options an [`Image`], a card image and a video poster share: placeholder, low
/// resolution file, fallback, error content, priority and fit.
#[derive(Default)]
pub(crate) struct Loading {
    pub(crate) placeholder: Option<Placeholder>,
    pub(crate) low: Option<String>,
    pub(crate) fallback: Option<String>,
    pub(crate) on_error: Option<Node>,
    pub(crate) priority: bool,
    pub(crate) fit: Option<Fit>,
}

impl Loading {
    /// Whether the image needs its box: every option but the priority does.
    pub(crate) fn wraps(&self) -> bool {
        self.placeholder.is_some()
            || self.low.is_some()
            || self.fallback.is_some()
            || self.on_error.is_some()
            || self.fit.is_some()
    }

    /// Classes of the box, each with a leading space.
    pub(crate) fn host_class(&self) -> String {
        let mut c = String::from(" z-img-host");
        if let Some(Placeholder(Fill::Blur(_))) = &self.placeholder {
            c.push_str(" z-img-blur");
        }
        match self.fit {
            Some(Fit::Cover) => c.push_str(" z-img-cover"),
            Some(Fit::Contain) => c.push_str(" z-img-contain"),
            None => {}
        }
        c
    }

    /// The `style` of the box: its ratio and the placeholder color or blur image. Every value
    /// is a number, a checked color or a checked base64 URI, so none can leave the attribute.
    pub(crate) fn host_style(&self, (w, h): (u32, u32)) -> String {
        format!(
            "aspect-ratio:{} / {}{}",
            w.max(1),
            h.max(1),
            self.fill_style()
        )
    }

    /// The placeholder color or blur image as custom properties, each with a leading `;`.
    pub(crate) fn fill_style(&self) -> String {
        match &self.placeholder {
            Some(Placeholder(Fill::Color(c))) => format!(";--z-ph:{c}"),
            Some(Placeholder(Fill::Blur(uri))) => format!(";--z-ph-img:url({uri})"),
            _ => String::new(),
        }
    }

    /// The layers under the image: the placeholder image and the low resolution file, those
    /// the policy allows.
    pub(crate) fn layers(&self, r: &Render, lazy: bool) -> String {
        let csp = policy(r);
        let loading = match (self.priority, lazy) {
            (true, _) => r#" fetchpriority="high""#,
            (false, true) => r#" loading="lazy""#,
            (false, false) => "",
        };
        let shared = match &self.placeholder {
            Some(Placeholder(Fill::Image(src))) => Some(src),
            _ => None,
        };
        shared
            .into_iter()
            .chain(self.low.as_ref())
            .filter(|src| csp.allows_image(src))
            .map(|src| {
                format!(
                    r#"<img class="z-img-under" src="{}" alt="" decoding="async"{loading}>"#,
                    escape(src)
                )
            })
            .collect()
    }

    /// Attributes of the image itself: how it loads, and the marker and fallback the media
    /// module reads when it has a box.
    pub(crate) fn img_attrs(&self, r: &Render, lazy: bool) -> String {
        let mut a = match (self.priority, lazy) {
            (true, _) => String::from(r#" loading="eager" fetchpriority="high""#),
            (false, true) => String::from(r#" loading="lazy""#),
            (false, false) => String::new(),
        };
        if self.wraps() {
            a.push_str(" data-zeyn-img");
            if let Some(src) = self
                .fallback
                .as_ref()
                .filter(|s| policy(r).allows_image(s))
            {
                a.push_str(&format!(r#" data-zeyn-fallback="{}""#, escape(src)));
            }
        }
        a
    }

    /// The app's error content as an inert template, shown only when the image fails.
    pub(crate) fn error(&self, r: &Render) -> String {
        match &self.on_error {
            Some(node) => {
                let mut out = String::from("<template data-zeyn-img-error>");
                node.render(&mut out, r);
                out.push_str("</template>");
                out
            }
            None => String::new(),
        }
    }

    pub(crate) fn schema(&self) -> Json {
        json!({
            "placeholder": self.placeholder.as_ref().map(Placeholder::kind),
            "low": self.low,
            "fallback": self.fallback,
            "on_error": self.on_error.as_ref().map(Node::schema),
            "priority": self.priority,
            "fit": self.fit,
        })
    }
}

/// A link to another site. It always has `rel="noopener noreferrer"`, so the other site learns
/// neither the page it came from nor gets a handle on this window, and screen readers hear that
/// it leaves the app.
///
/// ```
/// # use zeyn::ui::ExternalLink;
/// # use zeyn::Text;
/// let help = ExternalLink::new("https://example.com/help", Text::from("Help center")).new_tab();
/// let site = ExternalLink::new("https://example.org", Text::from("Website")).user_content();
/// ```
///
/// `.user_content()` adds `nofollow ugc` for addresses users typed, so search engines do not
/// credit them. `.new_tab()` opens it in a new tab and says so; it stays in the same tab unless
/// set. Only `https` and `http` addresses become links; anything else, such as `javascript:`,
/// is shown as plain text.
pub struct ExternalLink {
    url: String,
    text: Text,
    user_content: bool,
    new_tab: bool,
}

impl ExternalLink {
    pub fn new(url: impl Into<String>, text: impl Into<Text>) -> Self {
        ExternalLink {
            url: url.into(),
            text: text.into(),
            user_content: false,
            new_tab: false,
        }
    }

    /// Marks an address a user entered: adds `nofollow ugc`.
    pub fn user_content(mut self) -> Self {
        self.user_content = true;
        self
    }

    /// Opens the link in a new tab.
    pub fn new_tab(mut self) -> Self {
        self.new_tab = true;
        self
    }

    /// Link markup, for use inside other components.
    pub fn html(&self, r: &Render) -> String {
        let text = r.text(&self.text);
        let url = self.url.trim();
        let lower = url
            .get(..8)
            .unwrap_or(url)
            .to_ascii_lowercase();
        let web = (lower.starts_with("https://") || lower.starts_with("http://"))
            && url.len() > lower.find("//").unwrap_or(0) + 2
            && !url
                .chars()
                .any(|c| c.is_whitespace() || c.is_control());
        if !web {
            return format!(r#"<span class="z-link-text">{text}</span>"#);
        }
        let rel = if self.user_content {
            "noopener noreferrer nofollow ugc"
        } else {
            "noopener noreferrer"
        };
        let (target, spoken) = if self.new_tab {
            (r#" target="_blank""#, "zeyn.ui.link.external_new_tab")
        } else {
            ("", "zeyn.ui.link.external")
        };
        format!(
            r#"<a class="z-link-external" href="{}" rel="{rel}"{target} data-testid="{}.link">{text}<span class="z-sr-only"> ({})</span><span class="z-link-external-icon" aria-hidden="true">&#8599;</span></a>"#,
            escape(url),
            r.scope,
            r.text(&Text::key(spoken))
        )
    }
}

impl Component for ExternalLink {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&self.html(r));
    }

    fn schema(&self) -> Json {
        json!({ "type": "external_link", "url": self.url, "text": self.text, "user_content": self.user_content, "new_tab": self.new_tab })
    }
}

super::layout::into_node!(ExternalLink);
