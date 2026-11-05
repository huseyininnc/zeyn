//! Shop: a small public catalog in the site shell (top menu, footer) next to the work screens,
//! which keep the side menu. Moving between the two goes through the runtime without a reload.

use crate::stock::{Product, ProductId, ProductStore};
use zeyn::prelude::*;
use zeyn::ui::{Alert, ImageCard};
use zeyn::{Component, Render};

/// Active products for the public catalog: the same rows as the stock list, without the
/// permission, filtered, sorted, searched and paged by the query.
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(
        store
            .all()?
            .into_iter()
            .filter(|p| p.active)
            .collect(),
    ))
}

/// A placeholder picture for a product: a tinted square with the first letter of its name.
fn picture(p: &Product) -> String {
    let hue = (p.id.0 * 47) % 360;
    let letter = p
        .name
        .chars()
        .next()
        .map(String::from)
        .unwrap_or_default();
    format!(
        "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 300 300'%3E%3Crect width='300' height='300' fill='hsl({hue} 45%25 62%25)'/%3E%3Ctext x='150' y='190' font-family='sans-serif' font-size='140' text-anchor='middle' fill='white'%3E{letter}%3C/text%3E%3C/svg%3E"
    )
}

/// The card of a product, with its price in the visitor's language.
fn card(locale: String) -> impl Fn(&Product) -> ImageCard + Send + Sync + 'static {
    move |p: &Product| {
        let card = Card::image(picture(p), p.name.clone())
            .text(p.price.format(&locale))
            .link(shop_product, ProductPage { id: p.id });
        if p.id.0 > 228 {
            card.badge(Badge::new(t!("shop.new")).tone(Tone::Success))
        } else {
            card
        }
    }
}

/// Which product the product page shows.
#[params]
pub struct ProductPage {
    pub id: ProductId,
}

fn search_box(text: String) -> SearchBox<Product> {
    SearchBox::of(catalog_products)
        .label(t!("shop.search.label"))
        .instant(150)
        .value(text)
        .suggestion(|p: &Product| p.name.clone())
        .results(shop_search)
}

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
pub fn shop_home(_: (), ctx: Ctx) -> Screen {
    Screen::new(t!("shop.home.title"))
        .body(Alert::info(t!("shop.home.text")).title(t!("shop.home.lead")))
        .body(
            Rail::of(catalog_products)
                .title(t!("shop.new_arrivals"))
                .card(card(ctx.locale().to_string()))
                .sort_option(t!("shop.sort.newest"), Product::id, true)
                .image_size(300, 300),
        )
        .body(Alert::info(t!("shop.home.panel_text")).title(t!("shop.home.panel")))
        .body(ShellLink(t!("shop.home.open_panel"), "overview"))
}

#[screen(route = "/shop/products", public)]
pub fn shop_catalog(_: (), ctx: Ctx) -> Screen {
    Screen::new(t!("shop.products")).body(
        CardGrid::of(catalog_products)
            .card(card(ctx.locale().to_string()))
            .sort_option(t!("shop.sort.name"), Product::name, false)
            .sort_option(t!("shop.sort.cheapest"), Product::price, false)
            .sort_option(t!("shop.sort.dearest"), Product::price, true)
            .image_size(300, 300)
            .paging(Paging::infinite(24)),
    )
}

#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams, ctx: Ctx) -> Screen {
    let title = if p.q.trim().is_empty() {
        t!("shop.search.all")
    } else {
        t!("shop.search.title", q = p.q.clone())
    };
    Screen::new(title)
        .body(search_box(p.q.clone()))
        .body(
            CardGrid::of(catalog_products)
                .card(card(ctx.locale().to_string()))
                .search(p.q)
                .image_size(300, 300)
                .paging(Paging::pages(12)),
        )
}

#[screen(route = "/shop/product", public)]
pub fn shop_product(p: ProductPage, store: ProductStore, ctx: Ctx) -> Result<Screen> {
    let product = store.get(p.id)?;
    if !product.active {
        return Err(Error::NotFound);
    }
    Ok(Screen::new(product.name.clone()).body(Alert::info(t!(
        "shop.product.text",
        code = product.code.clone(),
        price = product.price.format(ctx.locale())
    ))))
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
        .screen(shop_catalog)
        .screen(shop_search)
        .screen(shop_product)
        .screen(shop_delivery)
        .screen(shop_returns)
        .screen(shop_about)
        .menu(|m| {
            m.group(t!("shop.menu.home"), Icon::Home)
                .item(t!("shop.menu.home"), shop_home)
                .group(t!("shop.products"), Icon::Cart)
                .item(t!("shop.products"), shop_catalog)
                .item(t!("shop.search.all"), shop_search)
                .group(t!("shop.menu.help"), Icon::Box)
                .item(t!("shop.delivery.title"), shop_delivery)
                .item(t!("shop.returns.title"), shop_returns)
                .group(t!("shop.about.title"), Icon::Home)
                .item(t!("shop.about.title"), shop_about)
        })
        .shell(
            Shell::site()
                .header(|h| {
                    h.center(search_box(String::new()))
                        .center_on_phone(zeyn::CenterOnPhone::Icon)
                        .sign_in()
                        .language()
                        .theme()
                })
                .footer(|f| {
                    f.column(t!("shop.menu.help"), |c| {
                        c.link(t!("shop.delivery.title"), shop_delivery)
                            .link(t!("shop.returns.title"), shop_returns)
                    })
                    .column(t!("shop.footer.company"), |c| {
                        c.link(t!("shop.about.title"), shop_about)
                            .link(t!("shop.footer.panel"), crate::overview::overview)
                    })
                    .text(t!("shop.footer.note"))
                })
                .bottom_nav([
                    zeyn::BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
                    zeyn::BottomNavItem::new(t!("shop.products"), shop_catalog).icon(Icon::Cart),
                    zeyn::BottomNavItem::new(t!("shop.search.all"), shop_search).icon(Icon::Search),
                ]),
        )
}
