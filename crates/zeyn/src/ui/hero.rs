//! [`Hero`]: a large image section with a title, text and actions, and optional slides.

use super::layout::into_node;
use super::render_child;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Hero);

/// Most slides a hero holds; more are left out.
pub const MAX_HERO_SLIDES: usize = 10;

/// Shortest and longest time a slide shows when slides rotate, in seconds.
pub const HERO_SECONDS: (u8, u8) = (4, 30);

struct Slide {
    title: Text,
    text: Option<Text>,
    image: Option<String>,
    /// How many of the hero's actions, in order, belong to this slide.
    actions: usize,
}

/// A large section at the top of a page: a background image with a title, a line of text and
/// actions such as buttons or links. It is static unless the app asks for more.
///
/// `.slide(title)` adds another slide; `.text`, `.image` and `.action` always set the last one.
/// With several slides the hero follows the W3C APG carousel pattern: slide picker buttons, and
/// each slide a group named "2 of 3". Slides change only when the user picks one, unless the app
/// calls `.rotate(seconds)`. Then a pause and play button comes first (WCAG 2.2.2), rotation
/// stops while the pointer is over the hero or focus is inside it, and it never starts when the
/// user prefers reduced motion. Without script the first slide shows.
///
/// The image follows the [`super::Image`] rules (a path of the app or an `https` address on a
/// host of the app's `img-src`; anything else is left out). It is decoration behind the text,
/// so its alternative text is empty; put what matters into the title and text. The first slide's
/// image loads with high priority, the others lazily.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Hero;
/// let welcome = Hero::new("Fresh every morning")
///     .text("Order before nine and it is at your door by noon.")
///     .image("/img/hero/kitchen.jpg")
///     .action(Paragraph::new("Open daily"));
///
/// let promos = Hero::new("Spring sale")
///     .image("/img/hero/spring.jpg")
///     .slide("New in store")
///     .text("Twelve new teas from four countries.")
///     .rotate(8);
/// ```
pub struct Hero {
    slides: Vec<Slide>,
    actions: Vec<Node>,
    rotate: Option<u8>,
}

impl Hero {
    pub fn new(title: impl Into<Text>) -> Self {
        Hero {
            slides: vec![Slide { title: title.into(), text: None, image: None, actions: 0 }],
            actions: Vec::new(),
            rotate: None,
        }
    }

    /// Starts another slide with its title. The calls after it set this slide.
    pub fn slide(mut self, title: impl Into<Text>) -> Self {
        if self.slides.len() < MAX_HERO_SLIDES {
            self.slides
                .push(Slide { title: title.into(), text: None, image: None, actions: 0 });
        }
        self
    }

    fn last(&mut self) -> &mut Slide {
        let at = self.slides.len() - 1;
        &mut self.slides[at]
    }

    /// The line of text under the title.
    pub fn text(mut self, text: impl Into<Text>) -> Self {
        self.last().text = Some(text.into());
        self
    }

    /// The background picture.
    pub fn image(mut self, src: impl Into<String>) -> Self {
        self.last().image = Some(src.into());
        self
    }

    /// A button, link or other node under the text.
    pub fn action(mut self, node: impl Into<Node>) -> Self {
        self.last().actions += 1;
        self.actions.push(node.into());
        self
    }

    /// Moves to the next slide every `seconds` ([`HERO_SECONDS`] bounds it), with a pause
    /// button. Does nothing with one slide.
    pub fn rotate(mut self, seconds: u8) -> Self {
        self.rotate = Some(seconds.clamp(HERO_SECONDS.0, HERO_SECONDS.1));
        self
    }
}

impl Component for Hero {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.hero.{key}")));
        let id = super::dom_id(r, "hero");
        let count = self.slides.len();
        let many = count > 1;
        let policy = super::embed::policy(r);
        let mut slides = String::new();
        let mut index = 0;
        for (i, slide) in self.slides.iter().enumerate() {
            let image = slide
                .image
                .as_deref()
                .filter(|src| policy.allows_image(src))
                .map(|src| {
                    let load = if i == 0 { r#" fetchpriority="high""# } else { r#" loading="lazy""# };
                    format!(r#"<img class="z-hero-img" src="{}" alt=""{load} decoding="async">"#, escape(src))
                })
                .unwrap_or_default();
            let group = if many {
                let name = r.text(
                    &Text::key("zeyn.ui.hero.slide_of")
                        .arg("n", (i + 1).to_string())
                        .arg("count", count.to_string()),
                );
                format!(r#" role="group" aria-roledescription="{}" aria-label="{name}""#, t("slide"))
            } else {
                String::new()
            };
            let hidden = if i > 0 { " hidden" } else { "" };
            slides.push_str(&format!(
                r#"<div class="z-hero-slide" id="{id}-{i}"{group}{hidden}>{image}<div class="z-hero-body"><h2 class="z-hero-title">{}</h2>"#,
                r.text(&slide.title)
            ));
            if let Some(text) = &slide.text {
                slides.push_str(&format!(r#"<p class="z-hero-text">{}</p>"#, r.text(text)));
            }
            if slide.actions > 0 {
                slides.push_str(r#"<div class="z-hero-actions">"#);
                for node in &self.actions[index..index + slide.actions] {
                    render_child(&mut slides, r, index, node);
                    index += 1;
                }
                slides.push_str("</div>");
            }
            slides.push_str("</div></div>");
        }
        if !many {
            out.push_str(&format!(r#"<section class="z-hero">{slides}</section>"#));
            return;
        }
        let rotate = self
            .rotate
            .map(|s| format!(r#" data-rotate="{s}""#))
            .unwrap_or_default();
        out.push_str(&format!(
            r#"<section class="z-hero z-hero-slides" aria-roledescription="{}" aria-label="{}" data-zeyn-hero{rotate}><div class="z-hero-controls" hidden>"#,
            t("carousel"),
            r.text(&self.slides[0].title)
        ));
        if self.rotate.is_some() {
            out.push_str(&format!(
                r#"<button type="button" class="z-hero-pause" data-zeyn-hero-pause data-text-pause="{0}" data-text-play="{1}" aria-label="{1}"><span class="z-hero-pause-icon" aria-hidden="true"></span></button>"#,
                t("pause"),
                t("play")
            ));
        }
        out.push_str(r#"<div class="z-hero-pick">"#);
        for i in 0..count {
            let name = r.text(
                &Text::key("zeyn.ui.hero.show")
                    .arg("n", (i + 1).to_string())
                    .arg("count", count.to_string()),
            );
            let current = if i == 0 { r#" aria-current="true""# } else { "" };
            out.push_str(&format!(
                r#"<button type="button" class="z-hero-dot" aria-controls="{id}-{i}" aria-label="{name}" data-zeyn-hero-show="{i}"{current}></button>"#
            ));
        }
        out.push_str(&format!(
            r#"</div></div><div class="z-hero-track" aria-live="{}">{slides}</div></section>"#,
            if self.rotate.is_some() { "off" } else { "polite" }
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "hero",
            "rotate": self.rotate,
            "slides": self
                .slides
                .iter()
                .map(|s| json!({ "title": s.title, "text": s.text, "image": s.image, "actions": s.actions }))
                .collect::<Vec<_>>(),
            "children": self
                .actions
                .iter()
                .map(Node::schema)
                .collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.actions
    }
}
