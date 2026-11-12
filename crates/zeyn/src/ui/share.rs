//! [`Share`]: a button that shares a link through the system share sheet or copies it, and
//! [`CopyButton`]: a button that copies a text.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(Share);

/// A button that shares a link. Where the browser has the Web Share API (most phones), it opens
/// the system share sheet; elsewhere it copies the link to the clipboard and says so in a toast.
/// It shares the page being shown unless [`Share::screen`] names another screen. The button
/// stays hidden until its script runs, so a page without script shows no button that does
/// nothing. Its label is "Share" in the user's language unless the app gives one.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Share;
///
/// #[params]
/// pub struct ArticleParams {
///     pub id: i64,
/// }
///
/// #[screen(route = "/articles/{id}", public)]
/// pub fn article(p: ArticleParams) -> Screen {
///     Screen::new("Article").body(Share::page().title("Getting started"))
/// }
///
/// let other = Share::screen(article, ArticleParams { id: 7 }).label("Share this article");
/// ```
pub struct Share {
    target: Option<(&'static str, Json)>,
    title: Option<Text>,
    label: Option<Text>,
}

impl Share {
    /// Shares the address of the page being shown.
    pub fn page() -> Self {
        Share {
            target: None,
            title: None,
            label: None,
        }
    }

    /// Shares the address of a screen. Params of the wrong type do not compile.
    pub fn screen<S: ScreenDef>(_screen: S, params: S::Params) -> Self {
        Share {
            target: Some((
                S::META.name,
                serde_json::to_value(params).unwrap_or(Json::Null),
            )),
            title: None,
            label: None,
        }
    }

    /// Shares the address of a screen given by name and params, for components that keep them
    /// that way.
    pub(crate) fn to(screen: &'static str, params: Json) -> Self {
        Share {
            target: Some((screen, params)),
            title: None,
            label: None,
        }
    }

    /// The title the share sheet shows with the link. The page title unless set.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The button's text, "Share" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Component for Share {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.share.{key}")));
        let url = self
            .target
            .as_ref()
            .map(|(screen, params)| format!(r#" data-url="{}""#, screen_href(r, screen, params)))
            .unwrap_or_default();
        let title = self
            .title
            .as_ref()
            .map(|t| format!(r#" data-title="{}""#, r.text(t)))
            .unwrap_or_default();
        let label = match &self.label {
            Some(l) => r.text(l),
            None => t("label"),
        };
        out.push_str(&format!(
            r#"<button type="button" class="z-button z-share" data-zeyn-share{url}{title} data-copied="{}" data-failed="{}" data-testid="{}.share" hidden>{label}</button>"#,
            t("copied"),
            t("failed"),
            r.scope
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "share",
            "screen": self.target.as_ref().map(|(s, _)| s),
            "params": self.target.as_ref().map(|(_, p)| p),
            "title": self.title,
            "label": self.label,
        })
    }
}

/// Longest text a [`CopyButton`] carries, in bytes. A longer text is cut at a character boundary.
pub const MAX_COPY_BYTES: usize = 4096;

/// A button that copies a text to the clipboard: an API address, a tracking number, an
/// invitation link. `.shown()` writes the text next to the button, in a fixed width font, so it
/// can also be read and selected; otherwise the text is in the page but hidden.
///
/// The button uses the `share.js` module, loaded only on pages that hold one. It is rendered
/// hidden and shown by the script, so a page without script has no button that does nothing; a
/// shown text stays readable there. The result ("Copied to the clipboard") is said in a toast.
/// The text is part of the page, so it must be one the user may see: never a masked value.
///
/// ```
/// use zeyn::ui::CopyButton;
/// let tracking = CopyButton::new("1Z999AA10123456784").shown();
/// let invite = CopyButton::new("https://example.com/join/abc").label("Copy the invitation");
/// ```
pub struct CopyButton {
    text: String,
    label: Option<Text>,
    shown: bool,
}

into_node!(CopyButton);

impl CopyButton {
    /// A button that copies `text`, at most [`MAX_COPY_BYTES`] of it.
    pub fn new(text: impl Into<String>) -> Self {
        let mut text = text.into();
        if text.len() > MAX_COPY_BYTES {
            let mut end = MAX_COPY_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        CopyButton {
            text,
            label: None,
            shown: false,
        }
    }

    /// The button's text, "Copy" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Writes the text next to the button.
    pub fn shown(mut self) -> Self {
        self.shown = true;
        self
    }
}

impl Component for CopyButton {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.code.{key}")));
        let id = super::dom_id(r, "copy");
        let label = match &self.label {
            Some(l) => r.text(l),
            None => t("copy"),
        };
        out.push_str(&format!(
            r#"<span class="z-copy"><code class="z-copy-text" id="{id}"{}>{}</code><button type="button" class="z-button z-copy-button" data-zeyn-copy="{id}" data-copied="{}" data-failed="{}" data-testid="{}.copy" hidden>{label}</button></span>"#,
            if self.shown { "" } else { " hidden" },
            crate::render::escape(&self.text),
            t("copied"),
            t("failed"),
            r.scope
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "copy_button", "text": self.text, "label": self.label, "shown": self.shown })
    }
}
