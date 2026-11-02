//! Shop: a small public catalog in the site shell (top menu, footer) next to the work screens,
//! which keep the side menu. Moving between the two goes through the runtime without a reload.

use zeyn::prelude::*;
use zeyn::ui::Alert;
use zeyn::{Component, Render};

/// A link to a screen of the other shell, as a button. The runtime swaps the whole frame without a
/// page reload.
pub struct ShellLink(pub Text, pub &'static str);

impl Component for ShellLink {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<p><a class="z-button z-primary" href="/_zeyn/route/{}" data-zeyn-base-href data-testid="{}.shell-link">{}</a></p>"#,
            self.1,
            r.scope,
            r.text(&self.0)
        ));
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "shell_link", "screen": self.1 })
    }
}

impl From<ShellLink> for Node {
    fn from(l: ShellLink) -> Self {
        Node::new(l)
    }
}

#[screen(route = "/shop", public)]
pub fn shop_home() -> Screen {
    Screen::new(t!("shop.home.title"))
        .body(Alert::info(t!("shop.home.text")).title(t!("shop.home.lead")))
        .body(Alert::info(t!("shop.home.panel_text")).title(t!("shop.home.panel")))
        .body(ShellLink(t!("shop.home.open_panel"), "overview"))
}

#[screen(route = "/shop/delivery", public)]
pub fn shop_delivery() -> Screen {
    Screen::new(t!("shop.delivery.title")).body(Alert::info(t!("shop.delivery.text")))
}

#[screen(route = "/shop/returns", public)]
pub fn shop_returns() -> Screen {
    Screen::new(t!("shop.returns.title")).body(Alert::info(t!("shop.returns.text")))
}

#[screen(route = "/shop/about", public)]
pub fn shop_about() -> Screen {
    Screen::new(t!("shop.about.title")).body(Alert::info(t!("shop.about.text")))
}

pub fn feature() -> Feature {
    Feature::new("shop")
        .screen(shop_home)
        .screen(shop_delivery)
        .screen(shop_returns)
        .screen(shop_about)
        .menu(|m| {
            m.group(t!("shop.menu.home"), Icon::Home)
                .item(t!("shop.menu.home"), shop_home)
                .group(t!("shop.menu.help"), Icon::Box)
                .item(t!("shop.delivery.title"), shop_delivery)
                .item(t!("shop.returns.title"), shop_returns)
                .group(t!("shop.about.title"), Icon::Home)
                .item(t!("shop.about.title"), shop_about)
        })
        .shell(Shell::site().header(|h| h.sign_in().language().theme()).footer(|f| {
            f.column(t!("shop.menu.help"), |c| {
                c.link(t!("shop.delivery.title"), shop_delivery).link(t!("shop.returns.title"), shop_returns)
            })
            .column(t!("shop.footer.company"), |c| {
                c.link(t!("shop.about.title"), shop_about).link(t!("shop.footer.panel"), crate::overview::overview)
            })
            .text(t!("shop.footer.note"))
        }))
}
