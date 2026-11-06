//! [`Countdown`]: the time left until a moment the server knows, such as the end of a sale.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use crate::time::DateTime;
use serde_json::{Value as Json, json};

into_node!(Countdown);

/// The time left until `end`: "2d 03:14:09", for the end of a sale or an auction. The page
/// carries `end` as an absolute moment (`data-end`, milliseconds since 1970), never the time left,
/// so a page served from a cache is never stale: the browser counts down to that moment. The
/// server writes the time left as it renders, for visitors without script, and, once `end` has
/// passed, the end state the screen gave ([`Countdown::ended`]) instead of the clock.
///
/// The small `countdown.js` module, loaded only on pages with a countdown, keeps one timer for
/// every countdown of the page, ticking once a second and stopped while the tab is hidden. The
/// digits are hidden from screen readers; a polite live region says the time left in words
/// ("2 d 3 h 14 min left") only when the minute changes, so it never talks every second. When the
/// time is up the end state replaces the clock, and a screen that needs fresh data refreshes it
/// with `.listens(topic)` on its own topics.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Countdown;
///
/// let ends = DateTime::from_unix_ms(1_790_000_000_000);
/// let timer = Countdown::until(ends)
///     .label("Sale ends in")
///     .ended(Badge::new("The sale has ended"));
/// ```
pub struct Countdown {
    end: DateTime,
    label: Option<Text>,
    ended: Vec<Node>,
}

impl Countdown {
    /// A countdown to `end`.
    pub fn until(end: DateTime) -> Self {
        Countdown {
            end,
            label: None,
            ended: Vec::new(),
        }
    }

    /// A text before the clock, such as "Sale ends in".
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// What shows once `end` has passed, rendered by the server, such as "The sale has ended".
    pub fn ended(mut self, node: impl Into<Node>) -> Self {
        self.ended = vec![node.into()];
        self
    }
}

/// Days, hours, minutes and seconds in `ms`.
fn parts(ms: i64) -> (i64, i64, i64, i64) {
    let s = ms.max(0) / 1000;
    (s / 86_400, s / 3_600 % 24, s / 60 % 60, s % 60)
}

impl Component for Countdown {
    fn render(&self, out: &mut String, r: &Render) {
        let now = r
            .ctx
            .map(|c| c.now())
            .unwrap_or_else(DateTime::now);
        let left = self.end.unix_ms() - now.unix_ms();
        let t = |key: &str| Text::key(format!("zeyn.ui.shop.countdown.{key}"));
        if left <= 0 {
            out.push_str(r#"<div class="z-countdown z-countdown-over">"#);
            for node in &self.ended {
                node.render(out, r);
            }
            out.push_str("</div>");
            return;
        }
        let (d, h, m, s) = parts(left);
        let label = self
            .label
            .as_ref()
            .map(|l| format!(r#"<span class="z-countdown-label">{}</span>"#, r.text(l)))
            .unwrap_or_default();
        let day = r.text(&t("day"));
        let days = if d > 0 {
            format!(r#"<span class="z-countdown-d">{d}</span>{day} "#)
        } else {
            String::from(r#"<span class="z-countdown-d" hidden>0</span>"#)
        };
        let words = |d: i64, h: i64, m: i64| {
            r.text(
                &t("left")
                    .arg("d", d.to_string())
                    .arg("h", h.to_string())
                    .arg("m", m.to_string()),
            )
        };
        out.push_str(&format!(
            r#"<div class="z-countdown" data-zeyn-countdown data-end="{}" data-day="{day}" data-text-left="{}">{label}<span class="z-countdown-time" aria-hidden="true">{days}<span class="z-countdown-h">{h:02}</span>:<span class="z-countdown-m">{m:02}</span>:<span class="z-countdown-s">{s:02}</span></span><span class="z-sr-only z-countdown-words" aria-live="polite" aria-atomic="true">{}</span><template class="z-countdown-end">"#,
            self.end.unix_ms(),
            r.text(&t("left")),
            words(d, h, m),
        ));
        for node in &self.ended {
            node.render(out, r);
        }
        out.push_str("</template></div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "countdown",
            "end": self.end.to_string(),
            "label": self.label,
            "ended": self.ended.first().map(Node::schema),
        })
    }
}
