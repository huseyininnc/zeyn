//! Story rings: [`StoryRing`], an avatar inside a ring that tells whether its stories are new,
//! and [`StoryRow`], a row of rings that scrolls sideways.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde::Serialize;
use serde_json::{Value as Json, json};

into_node!(StoryRing, StoryRow);

/// Most rings one [`StoryRow`] shows; more are left out.
pub const MAX_STORY_RINGS: usize = 100;

/// Whether the viewer has seen every story behind a [`StoryRing`]. The server decides it, from
/// what it stored about the viewer; the browser never changes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryState {
    /// At least one story is new to the viewer: a thick ring in the brand colors.
    Unseen,
    /// The viewer has seen them all: a thin neutral ring.
    Seen,
}

/// An avatar inside a ring, the entry to someone's stories. The ring is thick and in the brand
/// colors while there are stories the viewer has not seen ([`StoryState::Unseen`]), thin and
/// neutral once they are all seen; the two differ in width too, so color is not the only sign,
/// and in forced colors mode the seen ring is dashed. Drawn with CSS only: no script, no image.
///
/// The ring is a link to the screen that shows the stories when [`StoryRing::link`] is set. Its
/// accessible name is the person's name and the state, such as "Ada Lovelace, unseen stories";
/// the visible name under the ring starts the same way. The picture follows the
/// [`super::Avatar`] rules; without one the initials are shown.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{StoryRing, StoryState};
///
/// #[params]
/// pub struct StoryParams {
///     pub person: i64,
/// }
///
/// #[screen(route = "/stories/{person}", public)]
/// pub fn stories(p: StoryParams) -> Screen {
///     Screen::new("Stories")
/// }
///
/// let ring = StoryRing::new("Ada Lovelace", StoryState::Unseen)
///     .image("/img/people/ada.jpg")
///     .link(stories, StoryParams { person: 1 });
/// ```
pub struct StoryRing {
    name: Text,
    state: StoryState,
    image: Option<String>,
    link: Option<(&'static str, Json)>,
}

impl StoryRing {
    /// A ring for `name` in the given state.
    pub fn new(name: impl Into<Text>, state: StoryState) -> Self {
        StoryRing {
            name: name.into(),
            state,
            image: None,
            link: None,
        }
    }

    /// The picture inside the ring. Without one, or when it is not allowed, the initials are
    /// shown.
    pub fn image(mut self, src: impl Into<String>) -> Self {
        self.image = Some(src.into());
        self
    }

    /// The screen that shows the stories, opened without a page reload. Params of the wrong type
    /// do not compile.
    pub fn link<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.link = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }
}

impl Component for StoryRing {
    fn render(&self, out: &mut String, r: &Render) {
        let name = r.i18n.text(r.locale, &self.name);
        let (class, key) = match self.state {
            StoryState::Unseen => ("z-story z-story-unseen", "zeyn.ui.story.unseen"),
            StoryState::Seen => ("z-story z-story-seen", "zeyn.ui.story.seen"),
        };
        let label = escape(
            &r.i18n
                .text(r.locale, &Text::key(key))
                .replace("{name}", &name),
        );
        let face = match self
            .image
            .as_deref()
            .filter(|src| super::embed::policy(r).allows_image(src))
        {
            Some(src) => format!(
                r#"<img class="z-story-face" src="{}" alt="" width="64" height="64" loading="lazy" decoding="async">"#,
                escape(src)
            ),
            None => format!(
                r#"<span class="z-story-face z-story-initials">{}</span>"#,
                escape(&super::content::initials(&name, r.locale))
            ),
        };
        let inner = format!(
            r#"<span class="z-story-ring" aria-hidden="true">{face}</span><span class="z-story-name" aria-hidden="true">{}</span>"#,
            escape(&name)
        );
        match &self.link {
            Some((screen, params)) => out.push_str(&format!(
                r#"<a class="{class}" href="{}" data-zeyn-base-href aria-label="{label}">{inner}</a>"#,
                screen_href(r, screen, params)
            )),
            None => out.push_str(&format!(
                r#"<span class="{class}" role="img" aria-label="{label}">{inner}</span>"#
            )),
        }
    }

    fn schema(&self) -> Json {
        json!({
            "type": "story_ring",
            "name": self.name,
            "state": self.state,
            "image": self.image,
            "screen": self.link.as_ref().map(|(s, _)| s),
        })
    }
}

/// A row of [`StoryRing`]s that scrolls sideways, like a [`super::Rail`] but without script:
/// touch, trackpad and Shift+wheel scroll it, the rings snap into place, and a ring that gets
/// keyboard focus scrolls into view. At most [`MAX_STORY_RINGS`] rings; more are left out. The
/// row is a list named "Stories" unless [`StoryRow::label`] names it.
///
/// ```
/// use zeyn::ui::{StoryRing, StoryRow, StoryState};
/// let row = StoryRow::new()
///     .add(StoryRing::new("Ada Lovelace", StoryState::Unseen))
///     .add(StoryRing::new("Alan Turing", StoryState::Seen));
/// ```
#[derive(Default)]
pub struct StoryRow {
    label: Option<Text>,
    rings: Vec<StoryRing>,
}

impl StoryRow {
    pub fn new() -> Self {
        StoryRow::default()
    }

    /// The name of the row for screen readers, "Stories" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a ring at the end of the row.
    #[allow(
        clippy::should_implement_trait,
        reason = "a builder method that appends a ring; it is not arithmetic and `add` is the public name"
    )]
    pub fn add(mut self, ring: StoryRing) -> Self {
        if self.rings.len() < MAX_STORY_RINGS {
            self.rings.push(ring);
        }
        self
    }
}

impl Component for StoryRow {
    fn render(&self, out: &mut String, r: &Render) {
        let label = match &self.label {
            Some(l) => r.text(l),
            None => r.text(&Text::key("zeyn.ui.story.row")),
        };
        let focusable = if self
            .rings
            .iter()
            .any(|ring| ring.link.is_some())
        {
            ""
        } else {
            r#" tabindex="0""#
        };
        out.push_str(&format!(
            r#"<ul class="z-stories" aria-label="{label}"{focusable} data-testid="{}.stories">"#,
            r.scope
        ));
        for ring in &self.rings {
            out.push_str("<li>");
            ring.render(out, r);
            out.push_str("</li>");
        }
        out.push_str("</ul>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "story_row",
            "label": self.label,
            "rings": self.rings.iter().map(StoryRing::schema).collect::<Vec<_>>(),
        })
    }
}
