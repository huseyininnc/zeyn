//! The chat panel: markup rendered on the server, behavior in `assistant.js`.

use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::Component;
use serde_json::{Value as Json, json};

/// The assistant's chat as a screen component: a full page (`AssistantChat::page()`) or a panel
/// inside a layout (`AssistantChat::panel()`). Every page also gets a floating launcher unless the
/// app turns it off with `Assistant::launcher(false)`.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::assistant::AssistantChat;
///
/// #[screen(route = "/assistant", permission = "app.assistant.use")]
/// pub fn assistant_page() -> Screen {
///     Screen::new("Assistant").body(AssistantChat::page())
/// }
/// ```
pub struct AssistantChat {
    full: bool,
}

impl AssistantChat {
    /// Fills the screen's main area.
    pub fn page() -> Self {
        AssistantChat { full: true }
    }

    /// A panel that fits next to other content.
    pub fn panel() -> Self {
        AssistantChat { full: false }
    }
}

impl From<AssistantChat> for crate::screen::Node {
    fn from(c: AssistantChat) -> Self {
        crate::screen::Node::new(c)
    }
}

const ICON_SEND: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M3.4 20.4 21 12 3.4 3.6 3.4 10l12.6 2-12.6 2z"/></svg>"#;
const ICON_MIC: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1A7 7 0 0 0 19 11z"/></svg>"#;
const ICON_STOP: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>"#;
const ICON_CLOSE: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M18.3 5.7 12 12l6.3 6.3-1.4 1.4L10.6 13.4 4.3 19.7 2.9 18.3 9.2 12 2.9 5.7l1.4-1.4 6.3 6.3 6.3-6.3z"/></svg>"#;
pub(crate) const ICON_SPARK: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 2l1.9 5.6L19.5 9.5 13.9 11.4 12 17l-1.9-5.6L4.5 9.5l5.6-1.9zM19 15l.9 2.1L22 18l-2.1.9L19 21l-.9-2.1L16 18l2.1-.9z"/></svg>"#;

/// Texts the browser script needs, localized on the server.
fn texts(r: &Render) -> Json {
    let t = |k: &'static str| r.i18n.text(r.locale, &Text::key(k));
    json!({
        "you": t("zeyn.assistant.you"),
        "assistant": t("zeyn.assistant.title"),
        "thinking": t("zeyn.assistant.thinking"),
        "stopped": t("zeyn.assistant.stopped"),
        "retry": t("zeyn.assistant.retry"),
        "read": t("zeyn.assistant.read_aloud"),
        "copy": t("zeyn.assistant.copy"),
        "copied": t("zeyn.assistant.copied"),
        "confirm": t("zeyn.assistant.confirm"),
        "decline": t("zeyn.assistant.decline"),
        "confirm_title": t("zeyn.assistant.confirm_title"),
        "declined": t("zeyn.assistant.declined"),
        "listening": t("zeyn.assistant.listening"),
        "mic": t("zeyn.assistant.mic"),
        "mic_stop": t("zeyn.assistant.mic_stop"),
        "error": t("zeyn.assistant.error"),
        "unavailable": t("zeyn.assistant.unavailable"),
        "working": t("zeyn.assistant.working"),
        "empty": t("zeyn.assistant.empty"),
    })
}

/// Markup of a chat panel. `mode` is `page`, `panel` or `dock` (inside the launcher's panel).
pub(crate) fn markup(r: &Render, mode: &str) -> String {
    let t = |k: &'static str| r.text(&Text::key(k));
    let id = format!("z-assistant-{mode}");
    let close = if mode == "dock" {
        format!(
            r#"<button type="button" class="z-icon-button" data-zeyn-assistant-close aria-label="{0}" title="{0}">{ICON_CLOSE}</button>"#,
            t("zeyn.assistant.close")
        )
    } else {
        String::new()
    };
    let heading = if mode == "page" { "h2" } else { "h2" };
    format!(
        r#"<section class="z-assistant" data-zeyn-assistant="{mode}" aria-labelledby="{id}-title" data-zeyn-texts="{texts}"><header class="z-assistant-head"><span class="z-assistant-mark">{ICON_SPARK}</span><{heading} id="{id}-title">{title}</{heading}><button type="button" class="z-button z-assistant-new" data-zeyn-assistant-clear>{new}</button>{close}</header><div class="z-assistant-log" role="log" aria-live="polite" aria-labelledby="{id}-title" tabindex="0" data-zeyn-assistant-log><p class="z-assistant-empty">{empty}</p></div><div class="z-assistant-chips" role="group" aria-label="{suggestions}" data-zeyn-assistant-chips></div><form class="z-assistant-form" data-zeyn-assistant-form><label class="z-sr-only" for="{id}-input">{message}</label><textarea id="{id}-input" rows="1" maxlength="2000" placeholder="{placeholder}" data-zeyn-assistant-input></textarea><button type="button" class="z-icon-button" data-zeyn-assistant-mic aria-pressed="false" aria-label="{mic}" title="{mic}" hidden>{ICON_MIC}</button><button type="button" class="z-icon-button z-assistant-stop" data-zeyn-assistant-stop aria-label="{stop}" title="{stop}" hidden>{ICON_STOP}</button><button type="submit" class="z-icon-button z-assistant-send" aria-label="{send}" title="{send}">{ICON_SEND}</button></form><p class="z-assistant-note">{note}</p><p class="z-sr-only" role="status" data-zeyn-assistant-status></p></section>"#,
        texts = attr(&texts(r)),
        title = t("zeyn.assistant.title"),
        new = t("zeyn.assistant.new_chat"),
        empty = t("zeyn.assistant.empty"),
        suggestions = t("zeyn.assistant.suggestions"),
        message = t("zeyn.assistant.message"),
        placeholder = t("zeyn.assistant.placeholder"),
        mic = t("zeyn.assistant.mic"),
        stop = t("zeyn.assistant.stop"),
        send = t("zeyn.assistant.send"),
        note = t("zeyn.assistant.note"),
    )
}

/// The floating launcher and its panel, placed once in the page shell.
pub(crate) fn launcher(r: &Render) -> String {
    let label = r.text(&Text::key("zeyn.assistant.open"));
    format!(
        r#"<button type="button" class="z-assistant-launch" data-zeyn-assistant-launch aria-expanded="false" aria-controls="z-assistant-dock" aria-label="{label}" title="{label}">{ICON_SPARK}</button><aside id="z-assistant-dock" class="z-assistant-dock" aria-label="{title}" hidden>{panel}</aside>"#,
        title = escape(
            &r.i18n
                .text(r.locale, &Text::key("zeyn.assistant.title"))
        ),
        panel = markup(r, "dock"),
    )
}

impl Component for AssistantChat {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&markup(r, if self.full { "page" } else { "panel" }));
    }

    fn schema(&self) -> Json {
        json!({ "type": "assistant_chat", "mode": if self.full { "page" } else { "panel" } })
    }
}
