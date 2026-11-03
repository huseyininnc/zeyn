//! [`Share`]: a button that shares a link through the system share sheet or copies it.

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
        Share { target: None, title: None, label: None }
    }

    /// Shares the address of a screen. Params of the wrong type do not compile.
    pub fn screen<S: ScreenDef>(_screen: S, params: S::Params) -> Self {
        Share {
            target: Some((S::META.name, serde_json::to_value(params).unwrap_or(Json::Null))),
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
