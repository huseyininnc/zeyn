//! {{title}}: a Zeyn component crate with a browser script, a stylesheet and texts of its own.
//!
//! The crate gives an app two things: the component ([`{{type}}`]) and a [`Plugin`] that
//! registers its browser module ([`{{type}}Plugin`]). A page links the module only when it holds
//! the component, and `zeyn.js` loads it when the component arrives through client navigation.
//!
//! ```
//! use zeyn::prelude::*;
//! use {{crate}}::{{{type}}, {{type}}Plugin};
//!
//! let app = App::new("Shop").plugin({{type}}Plugin);
//! let screen = Screen::new("Item").body({{type}}::new("Favorite"));
//! # let _ = (app, screen);
//! ```

use serde_json::json;
use zeyn::prelude::*;
use zeyn::{Component, Module, Plugin, Render};

/// The module name: the crate `{{name}}` registers `{{component}}`.
pub const NAME: &str = "{{component}}";

/// The texts of the component, by locale.
pub const CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../assets/en.json")),
    ("tr", include_str!("../assets/tr.json")),
];

/// The browser module of the crate: script, stylesheet, markers and texts.
pub fn module() -> Module {
    Module::new(NAME)
        .js(include_str!("../assets/{{component}}.js"))
        .css(include_str!("../assets/{{component}}.css"))
        .marker("data-zeyn-{{component}}-toggle")
        .texts(CATALOGS)
}

/// Registers the crate's browser module: `App::new(..).plugin({{type}}Plugin)`.
pub struct {{type}}Plugin;

impl Plugin for {{type}}Plugin {
    fn install(self, app: App) -> App {
        app.module(module())
    }
}

/// A toggle button (W3C APG button pattern with `aria-pressed`): pressing it, or Space and
/// Enter while it has focus, switches it on and off, and a polite live region says the new
/// state. The state lives in the browser only. Replace it with your own component.
pub struct {{type}} {
    label: Text,
    on: bool,
}

impl {{type}} {
    /// A switched off toggle with a label: `{{type}}::new("Favorite")`.
    pub fn new(label: impl Into<Text>) -> Self {
        {{type}} {
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

impl Component for {{type}} {
    fn render(&self, out: &mut String, r: &Render<'_>) {
        let state = |key: &str| r.text(&Text::key(format!("{{component}}.{key}")));
        out.push_str(&format!(
            r#"<span class="z-{{component}}"><button type="button" class="z-{{component}}-toggle" data-zeyn-{{component}}-toggle aria-pressed="{pressed}" data-text-on="{on}" data-text-off="{off}" data-testid="{scope}.toggle">{label}</button><span class="z-{{component}}-status" role="status" aria-live="polite"></span></span>"#,
            pressed = self.on,
            on = state("on"),
            off = state("off"),
            scope = zeyn::render::escape(r.scope),
            label = r.text(&self.label),
        ));
    }

    fn schema(&self) -> serde_json::Value {
        json!({ "type": "{{component}}.toggle", "label": self.label, "on": self.on })
    }
}

impl From<{{type}}> for Node {
    fn from(component: {{type}}) -> Node {
        Node::new(component)
    }
}
