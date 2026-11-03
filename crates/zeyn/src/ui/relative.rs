//! [`RelativeTime`]: a moment shown as "3 minutes ago" in the reader's language.

use super::layout::into_node;
use crate::render::Render;
use crate::screen::{Component, Node};
use crate::time::DateTime;
use serde_json::{Value as Json, json};

into_node!(RelativeTime);

/// A moment shown as relative time, such as "3 minutes ago" or "in 2 days", in the page's
/// language. The server writes the exact date and time in the user's time zone inside a
/// `<time datetime="...">`, so the HTML does not change from minute to minute and pages can be
/// cached. The browser runtime rewrites the text with `Intl.RelativeTimeFormat` when the page
/// loads and once a minute after that; the exact time stays in the tooltip. Without script, or in
/// a browser without `Intl.RelativeTimeFormat`, the exact time stays as the text.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::RelativeTime;
/// let posted = RelativeTime::new(DateTime::from_unix_ms(1_778_580_000_000));
/// ```
pub struct RelativeTime {
    at: DateTime,
}

impl RelativeTime {
    pub fn new(at: DateTime) -> Self {
        RelativeTime { at }
    }
}

impl Component for RelativeTime {
    fn render(&self, out: &mut String, r: &Render) {
        let exact = crate::render::escape(&self.at.format(r.tz, r.locale));
        out.push_str(&format!(
            r#"<time class="z-relative" datetime="{}" title="{exact}" data-zeyn-relative>{exact}</time>"#,
            self.at
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "relative_time", "at": self.at.to_string() })
    }
}
