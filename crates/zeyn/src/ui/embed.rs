//! Content from other sites: framed pages ([`Embed`]).
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
