//! Overview: key numbers as Kpi cards.

use crate::sales::todays_orders;
use crate::stock::ProductStore;
use zeyn::prelude::*;

#[screen(route = "/dashboard", public)]
pub fn overview(_: (), store: ProductStore, ctx: Ctx) -> Result<Screen> {
    let products = store.all()?;
    let low = products
        .iter()
        .filter(|p| p.stock < 10)
        .count();
    let orders = todays_orders(&ctx);
    let revenue = Money::tl(
        orders
            .iter()
            .map(|o| o.amount.minor)
            .sum(),
    );
    Ok(Screen::new(t!("dashboard.title"))
        .body(
            Row::new()
                .col(Kpi::new(t!("dashboard.products"), products.len() as f64))
                .col(
                    Kpi::new(t!("dashboard.low_stock"), low as f64)
                        .lower_is_better()
                        .delta(-12.5),
                )
                .col(
                    Kpi::new(t!("dashboard.orders_today"), orders.len() as f64)
                        .delta(6.2)
                        .trend([42.0, 51.0, 48.0, 57.0, 61.0, orders.len() as f64]),
                )
                .col(
                    Kpi::money(t!("dashboard.revenue_today"), revenue)
                        .delta(-2.4)
                        .trend([9.1, 8.4, 8.8, 7.9, 8.2, 8.0]),
                ),
        )
        .body(zeyn::ui::Alert::info(t!("dashboard.shop_text")).title(t!("dashboard.shop")))
        .body(crate::shop::ShellLink(
            t!("dashboard.open_shop"),
            "shop_home",
        )))
}

/// The app's 404 page (`App::not_found`): shown for unknown paths and missing records.
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found"))
        .body(EmptyState::new(t!("errors.not_found_text")))
        .body(
            Breadcrumb::new()
                .link(t!("dashboard.title"), overview, ())
                .current(t!("errors.not_found")),
        )
}

/// The app's server error page (`App::server_error`). It never sees what went wrong.
#[screen(public)]
pub fn server_error_page() -> Screen {
    Screen::new(t!("errors.server"))
        .body(EmptyState::new(t!("errors.server_text")))
        .body(
            Breadcrumb::new()
                .link(t!("dashboard.title"), overview, ())
                .current(t!("errors.server")),
        )
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
        .body(
            Card::new()
                .title(t!("about.lead"))
                .add(Alert::info(t!("about.cache"))),
        )
}

/// The public pages of the showcase for `/sitemap.xml`.
#[action(public)]
pub async fn public_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
    Ok(part.of(&[SitemapUrl::new("/about"), SitemapUrl::new("/dashboard")]))
}

pub fn feature() -> Feature {
    Feature::new("overview")
        .screen(overview)
        .screen(about)
        .sitemap(public_pages)
        .menu(|m| {
            m.group(t!("menu.overview"), Icon::ChartBar)
                .item(t!("dashboard.title"), overview)
                .icon(Icon::Home)
                .item(t!("about.title"), about)
                .icon(Icon::Tag)
        })
}
