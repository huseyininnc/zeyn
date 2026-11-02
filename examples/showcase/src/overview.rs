//! Overview: key numbers, drawn by a component written in the app (an extension point).

use crate::sales::todays_orders;
use crate::stock::ProductStore;
use zeyn::prelude::*;
use zeyn::{Component, Render};

/// A row of key numbers.
pub struct Stats(Vec<(Text, String)>);

impl Component for Stats {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<div class="z-stats" data-testid="{}.stats">"#, r.scope));
        for (label, value) in &self.0 {
            out.push_str(&format!(
                r#"<div class="z-stat"><p class="z-stat-label">{}</p><p class="z-stat-value">{}</p></div>"#,
                r.text(label),
                zeyn::render::escape(value)
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "stats", "items": self.0.iter().map(|(l, v)| serde_json::json!({ "label": l, "value": v })).collect::<Vec<_>>() })
    }
}

impl From<Stats> for Node {
    fn from(s: Stats) -> Self {
        Node::new(s)
    }
}

#[screen(route = "/dashboard", public)]
pub fn overview(_: (), store: ProductStore, ctx: Ctx) -> Result<Screen> {
    let products = store.all()?;
    let low = products.iter().filter(|p| p.stock < 10).count();
    let orders = todays_orders(&ctx);
    let revenue = Money::tl(orders.iter().map(|o| o.amount.minor).sum());
    Ok(Screen::new(t!("dashboard.title")).body(Stats(vec![
        (t!("dashboard.products"), products.len().to_string()),
        (t!("dashboard.low_stock"), low.to_string()),
        (t!("dashboard.orders_today"), orders.len().to_string()),
        (t!("dashboard.revenue_today"), revenue.format(ctx.locale())),
    ])))
}

/// A public page for visitors: sent without cookies, kept by caches for five minutes, and
/// described for search engines and link previews.
#[screen(route = "/about", public, cache = 300)]
pub fn about() -> Screen {
    Screen::new(t!("about.title"))
        .description(t!("about.description"))
        .canonical("/about")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({
            "@context": "https://schema.org",
            "@type": "Organization",
            "name": "Northwind Operations",
        }))
        .body(Card::new().title(t!("about.lead")).add(Alert::info(t!("about.cache"))))
}

/// The public pages of the showcase for `/sitemap.xml`.
#[action(public)]
pub async fn public_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
    Ok(part.of(&[SitemapUrl::new("/about"), SitemapUrl::new("/dashboard")]))
}

pub fn feature() -> Feature {
    Feature::new("overview").screen(overview).screen(about).sitemap(public_pages).menu(|m| {
        m.group(t!("menu.overview"), Icon::Chart).item(t!("dashboard.title"), overview).item(t!("about.title"), about)
    })
}
