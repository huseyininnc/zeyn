//! Home: the first screen after sign in.

use zeyn::prelude::*;

/// Welcome screen.
#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new(t!("home.title")).body(Intro)
}

/// One paragraph of text.
struct Intro;

impl zeyn::Component for Intro {
    fn render(&self, out: &mut String, r: &zeyn::Render<'_>) {
        out.push_str(&format!("<p>{}</p>", r.text(&t!("home.intro"))));
    }

    fn schema(&self) -> zeyn::__serde_json::Value {
        zeyn::__serde_json::json!({ "type": "text" })
    }
}

impl From<Intro> for Node {
    fn from(i: Intro) -> Self {
        Node::new(i)
    }
}

pub fn feature() -> Feature {
    Feature::new("home")
        .screen(home)
        .menu(|m| {
            m.group(t!("home.menu"), Icon::Home)
                .item(t!("home.title"), home)
        })
}
