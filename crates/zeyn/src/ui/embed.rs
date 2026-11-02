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
fn policy<'a>(r: &Render<'a>) -> &'a Csp {
    r.ctx.map(|c| c.csp()).unwrap_or(&EMPTY)
}

/// Host of an `https` URL, for messages.
fn host_of(url: &str) -> &str {
    let rest = url.get(8..).unwrap_or("");
    rest.split(['/', '?', '#']).next().unwrap_or("")
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
        let allow: Vec<&str> = self.allow.iter().map(|p| p.feature()).collect();
        let allow = if allow.is_empty() { String::new() } else { format!(r#" allow="{}""#, allow.join("; ")) };
        let frame = format!(
            r#"<iframe src="{}" title="{title}" sandbox="{EMBED_SANDBOX}"{allow} referrerpolicy="no-referrer" loading="lazy"></iframe>"#,
            escape(&url)
        );
        let (w, h) = self.aspect;
        out.push_str(&format!(
            r#"<div class="z-embed" style="--z-aspect: {w} / {h}" data-testid="{}.embed">"#,
            r.scope
        ));
        match &self.poster {
            None => out.push_str(&frame),
            Some(poster) => {
                let image = if !poster.is_empty() && csp.allows_image(poster) {
                    format!(r#"<img src="{}" alt="" decoding="async" loading="lazy">"#, escape(poster))
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    r#"<button type="button" class="z-embed-load" data-zeyn-embed-load>{image}<span class="z-embed-play" aria-hidden="true"></span><span class="z-embed-label"><strong>{}</strong><small>{}</small></span></button><template data-zeyn-embed>{frame}</template>"#,
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
/// `src` is a path on the app (`/static/logo.png`) or an `https` address on a host of the
/// app's `img-src` (`App::csp(|c| c.img_src([...]))`). Any other image is not rendered; its alt
/// text is shown instead. `srcset` entries on other hosts, or with a descriptor that is not a
/// density (`2x`) or a width (`800w`), are left out. Every image gets `decoding="async"`;
/// `.lazy()` adds `loading="lazy"` for images below the fold. Pass `Text::from("")` as the alt
/// text for a purely decorative image.
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
        self.srcset.extend(candidates.into_iter().map(|(u, d)| (u.to_string(), d.to_string())));
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
                && n.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                && n.parse::<f32>().is_ok_and(|v| v > 0.0 && v <= 8.0)
        }
        None => {
            d.strip_suffix('w').is_some_and(|n| !n.is_empty() && n.len() <= 5 && n.bytes().all(|b| b.is_ascii_digit()))
        }
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
            out.push_str(&format!(r#"<span class="z-image-blocked" data-testid="{}.image">{alt}</span>"#, r.scope));
            return;
        }
        let candidates: Vec<String> = self
            .srcset
            .iter()
            .filter(|(u, d)| !u.contains([' ', ',']) && csp.allows_image(u) && valid_descriptor(d))
            .map(|(u, d)| format!("{} {d}", escape(u)))
            .collect();
        let srcset = if candidates.is_empty() {
            String::new()
        } else {
            let sizes = if candidates.iter().any(|c| c.ends_with('w')) {
                format!(r#" sizes="(max-width: {w}px) 100vw, {w}px""#)
            } else {
                String::new()
            };
            format!(r#" srcset="{}"{sizes}"#, candidates.join(", "))
        };
        let lazy = if self.lazy { r#" loading="lazy""# } else { "" };
        out.push_str(&format!(
            r#"<img class="z-image" src="{}" alt="{alt}" width="{w}" height="{h}" decoding="async"{lazy}{srcset} data-testid="{}.image">"#,
            escape(&self.src),
            r.scope
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "image", "src": self.src, "alt": self.alt, "width": self.size.0, "height": self.size.1, "lazy": self.lazy })
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
        ExternalLink { url: url.into(), text: text.into(), user_content: false, new_tab: false }
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
        let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
        let web = (lower.starts_with("https://") || lower.starts_with("http://"))
            && url.len() > lower.find("//").unwrap_or(0) + 2
            && !url.chars().any(|c| c.is_whitespace() || c.is_control());
        if !web {
            return format!(r#"<span class="z-link-text">{text}</span>"#);
        }
        let rel = if self.user_content { "noopener noreferrer nofollow ugc" } else { "noopener noreferrer" };
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
