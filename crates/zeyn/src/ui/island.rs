//! WASM islands: Rust components compiled to WebAssembly and placed on a server rendered page.
//!
//! The server renders a placeholder with the props as JSON and fallback content (any nodes, so a
//! read only grid or a text summary works without the island). `/_zeyn/islands.js` loads the
//! island when the placeholder comes into view, on first interaction or when the browser is
//! idle, and never blocks the page. See `docs/islands.md`.

use super::{ActionSource, DataSource, dom_id, layout::into_node, render_child};
use crate::action::{Action, ActionMeta, Result};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::Serialize;
use serde_json::{Value as Json, json};
pub use zeyn_island::Remote;

into_node!(Island);

/// Attribute that marks a placeholder. The quote makes it impossible to produce from escaped
/// user data, so the server can look for it in rendered HTML.
pub(crate) const MARKER: &str = "data-zeyn-island=\"";

/// When an island's code loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IslandLoad {
    /// When the placeholder comes within 200 px of the viewport (the default).
    Visible,
    /// On the first pointer, touch or focus on the placeholder.
    Interaction,
    /// When the browser is idle after the page loaded.
    Idle,
}

/// A drawing surface: an island whose code draws on a `<canvas>` with
/// `zeyn_island::canvas::Surface` (2D, or WebGL 2 when available).
///
/// A canvas is invisible to assistive technology, so the description is required: say what the
/// picture shows and which keys operate it. The fallback should be the keyboard and screen reader
/// alternative, such as a text summary or a form over the same data; it is what users without
/// WebAssembly get too.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::Canvas;
/// use zeyn::{Component, I18n, Render};
///
/// let canvas = Canvas::new("diagram", "Arrow keys move the selected shape.").label("Diagram");
/// let mut html = String::new();
/// canvas.render(&mut html, &Render::new("d", "en", I18n::builtin()));
/// assert!(html.contains(r#"aria-describedby="d-island-description-""#));
/// assert!(html.contains("Arrow keys move the selected shape."));
/// ```
pub struct Canvas;

impl Canvas {
    /// A canvas island named `name` (its crate's library name) with its description.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: &'static str, description: impl Into<Text>) -> Island {
        Island::new(name).description(description)
    }
}

/// A Rust component compiled to WebAssembly, built with `zeyn build-islands`.
///
/// `name` is the island crate's library name. Props are any serde type, usually from a crate the
/// server and the island share. The island calls only the actions bound here with
/// [`Island::action`], through `POST /_zeyn/data` with the usual checks.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Island, Remote};
/// use zeyn::{Component, I18n, Render};
///
/// #[derive(serde::Serialize, serde::Deserialize)]
/// pub struct Count { pub value: i64 }
///
/// pub struct SaveCount;
/// impl Remote for SaveCount {
///     const NAME: &'static str = "save";
///     type Input = Count;
///     type Output = Count;
/// }
///
/// #[action(permission = "demo.count.write")]
/// pub async fn save_count(count: Count) -> Result<Count> { Ok(count) }
///
/// let island = Island::new("counter")
///     .props(&Count { value: 3 })
///     .label("Counter")
///     .fallback(Alert::info("The counter needs JavaScript."))
///     .action::<SaveCount, _>(save_count);
/// let mut html = String::new();
/// island.render(&mut html, &Render::new("demo", "en", I18n::builtin()));
/// assert!(html.contains(r#"data-zeyn-island="counter""#));
/// assert!(html.contains(r#"data-zeyn-props="{&quot;value&quot;:3}""#));
/// assert_eq!(island.actions()[0].0, "save");
/// ```
pub struct Island {
    name: &'static str,
    props: String,
    label: Text,
    description: Option<Text>,
    load: IslandLoad,
    fallback: Vec<Node>,
    remotes: Vec<(&'static str, Box<dyn DataSource>)>,
}

impl Island {
    /// An island by its library name: lower case letters, digits and `_`.
    pub fn new(name: &'static str) -> Self {
        Island {
            name,
            props: "null".into(),
            label: Text::from(""),
            description: None,
            load: IslandLoad::Visible,
            fallback: Vec::new(),
            remotes: Vec::new(),
        }
    }

    /// A text shown above the island and referenced by `aria-describedby`: what it shows and how
    /// to use it with a keyboard. The island can point its own controls at the same id (the
    /// placeholder's `aria-describedby`).
    pub fn description(mut self, text: impl Into<Text>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Props the island reads on mount. Serialized once, here.
    pub fn props<P: Serialize>(mut self, props: &P) -> Self {
        self.props = serde_json::to_string(props).unwrap_or_else(|_| "null".into());
        self
    }

    /// The accessible name of the island's region.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = label.into();
        self
    }

    /// Content shown before the island loads and when it cannot load (no JavaScript, an old
    /// browser, a failed download). The loader hides it once the island has mounted.
    pub fn fallback(mut self, node: impl Into<Node>) -> Self {
        self.fallback.push(node.into());
        self
    }

    /// Loads the code on the first pointer, touch or focus instead of when it comes into view.
    pub fn on_interaction(mut self) -> Self {
        self.load = IslandLoad::Interaction;
        self
    }

    /// Loads the code when the browser is idle after the page loaded.
    pub fn on_idle(mut self) -> Self {
        self.load = IslandLoad::Idle;
        self
    }

    /// Lets the island call `action` as `R`. The action's input and output must be the remote's
    /// types, so the server and the island cannot drift apart.
    pub fn action<R, A>(mut self, _action: A) -> Self
    where
        R: Remote,
        A: Action<Input = R::Input, Output = Result<R::Output>>,
    {
        self.remotes
            .retain(|(k, _)| *k != R::NAME);
        self.remotes
            .push((R::NAME, Box::new(ActionSource::<A>::new())));
        self
    }

    /// Actions the island may call, by remote name.
    pub fn actions(&self) -> Vec<(&'static str, ActionMeta)> {
        self.remotes
            .iter()
            .map(|(k, s)| (*k, s.action()))
            .collect()
    }
}

impl Component for Island {
    fn render(&self, out: &mut String, r: &Render) {
        let described = self
            .description
            .as_ref()
            .map(|_| dom_id(r, "island-description"));
        out.push_str(&format!(
            r#"<div class="z-island" {MARKER}{}" data-zeyn-load="{}" data-zeyn-props="{}" role="region" aria-label="{}"{} data-testid="{}.island.{}">"#,
            escape(self.name),
            match self.load {
                IslandLoad::Visible => "visible",
                IslandLoad::Interaction => "interaction",
                IslandLoad::Idle => "idle",
            },
            escape(&self.props),
            r.text(&self.label),
            described.as_ref().map(|id| format!(r#" aria-describedby="{id}""#)).unwrap_or_default(),
            r.scope,
            escape(self.name),
        ));
        if let (Some(id), Some(text)) = (&described, &self.description) {
            out.push_str(&format!(
                r#"<p class="z-island-description" id="{id}">{}</p>"#,
                r.text(text)
            ));
        }
        out.push_str(r#"<div class="z-island-fallback" data-zeyn-island-fallback>"#);
        for (i, node) in self.fallback.iter().enumerate() {
            render_child(out, r, i, node);
        }
        out.push_str("</div></div>");
    }

    fn schema(&self) -> Json {
        let props: Json = serde_json::from_str(&self.props).unwrap_or(Json::Null);
        json!({
            "type": "island",
            "name": self.name,
            "label": self.label,
            "description": self.description,
            "load": self.load,
            "props": props,
            "actions": self.remotes.iter().map(|(k, s)| json!({ "name": k, "action": s.action() })).collect::<Vec<_>>(),
            "fallback": self.fallback.iter().map(Node::schema).collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.fallback
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        self.remotes
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, s)| s.as_ref())
    }
}
