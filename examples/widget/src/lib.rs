//! A component crate outside the framework: one toggle button with its own script, stylesheet
//! and texts, shipped the way a `zeyn-<name>` crate ships a component.
//!
//! The crate gives the app two things: the component ([`Toggle`]) and a [`Plugin`] that
//! registers its browser module ([`WidgetPlugin`]). A page links the module only when it holds
//! a toggle, and `zeyn.js` loads it when a toggle arrives through client navigation.
//!
//! ```
//! use zeyn::prelude::*;
//! use zeyn_example_widget::{Toggle, WidgetPlugin};
//!
//! let app = App::new("Shop").plugin(WidgetPlugin);
//! let screen = Screen::new("Item").body(Toggle::new("Favorite"));
//! # let _ = (app, screen);
//! ```

use serde_json::json;
use zeyn::prelude::*;
use zeyn::{Component, Module, Plugin, Render};

/// The module name: the crate `zeyn-widget` would register `widget`.
pub const NAME: &str = "widget";

/// The texts of the component, by locale.
pub const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../assets/en.json")),
    ("tr", include_str!("../assets/tr.json")),
];

/// The browser module of the crate: script, stylesheet, markers and texts.
pub fn module() -> Module {
    Module::new(NAME)
        .js(include_str!("../assets/widget.js"))
        .css(include_str!("../assets/widget.css"))
        .marker("data-zeyn-widget-toggle")
        .texts(CATALOGS)
}

/// Registers the crate's browser module: `App::new(..).plugin(WidgetPlugin)`.
pub struct WidgetPlugin;

impl Plugin for WidgetPlugin {
    fn install(self, app: App) -> App {
        app.module(module())
    }
}

/// A toggle button (W3C APG button pattern with `aria-pressed`): pressing it, or Space and
/// Enter while it has focus, switches it on and off, and a polite live region says the new
/// state. The state lives in the browser only.
pub struct Toggle {
    label: Text,
    on: bool,
}

impl Toggle {
    pub fn new(label: impl Into<Text>) -> Self {
        Toggle {
            label: label.into(),
            on: false,
        }
    }

    /// Starts switched on.
    pub fn on(mut self) -> Self {
        self.on = true;
        self
    }
}

impl Component for Toggle {
    fn render(&self, out: &mut String, r: &Render) {
        let state = |key: &str| r.text(&Text::key(format!("widget.{key}")));
        out.push_str(&format!(
            r#"<span class="z-widget"><button type="button" class="z-widget-toggle" data-zeyn-widget-toggle aria-pressed="{pressed}" data-text-on="{on}" data-text-off="{off}" data-testid="{scope}.toggle">{label}</button><span class="z-widget-status" role="status" aria-live="polite"></span></span>"#,
            pressed = self.on,
            on = state("on"),
            off = state("off"),
            scope = zeyn::render::escape(r.scope),
            label = r.text(&self.label),
        ));
    }

    fn schema(&self) -> serde_json::Value {
        json!({ "type": "widget.toggle", "label": self.label, "on": self.on })
    }
}

impl From<Toggle> for Node {
    fn from(toggle: Toggle) -> Node {
        Node::new(toggle)
    }
}
