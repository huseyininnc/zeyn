//! A product listing of 2,000 products and a product page, built from the shop components:
//! facets with counts, a price range, a card grid with prices and progressive photos, and a
//! product page with a gallery, variants, a quantity stepper and reviews.

use crate::data::{BRANDS, COLORS, Item, items, picture, search_items};
use zeyn::prelude::*;
use zeyn::ui::{QuantityChange, QuantityState};

/// The filters of the listing, kept in the address.
#[params]
#[derive(Default)]
pub struct ShopParams {
    #[serde(default)]
    pub color: Vec<String>,
    #[serde(default)]
    pub brand: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

/// The highest price of the range, in whole lira.
const TOP_PRICE: i64 = 1_000;

/// Whether an item passes every filter but `skip`, the facet being counted.
fn passes(i: &Item, p: &ShopParams, skip: &str) -> bool {
    let low = p.min_price.unwrap_or(0) * 100;
    let high = p.max_price.unwrap_or(TOP_PRICE) * 100;
    (low..=high).contains(&i.price.minor)
        && (skip == "color" || p.color.is_empty() || p.color.contains(&i.color))
        && (skip == "brand" || p.brand.is_empty() || p.brand.contains(&i.brand))
}

/// How many items have each value of a facet with the other filters applied: one pass over the
/// items per facet, at most as many counts as the facet has options.
fn counts<'a>(
    p: &ShopParams,
    facet: &str,
    values: impl Iterator<Item = &'a str>,
    of: fn(&Item) -> &str,
) -> Vec<(&'a str, u64)> {
    let mut out: Vec<(&str, u64)> = values
        .take(zeyn::ui::MAX_FACET_OPTIONS)
        .map(|v| (v, 0))
        .collect();
    for i in items()
        .iter()
        .filter(|i| passes(i, p, facet))
    {
        if let Some(c) = out
            .iter_mut()
            .find(|(v, _)| *v == of(i))
        {
            c.1 += 1;
        }
    }
    out
}

/// The facet panel of the listing.
fn facets(p: &ShopParams) -> FacetPanel {
    let colors = counts(p, "color", COLORS.iter().map(|c| c.0), |i| &i.color);
    let brands = counts(p, "brand", BRANDS.iter().copied(), |i| &i.brand);
    let color = COLORS
        .iter()
        .zip(colors)
        .fold(Facet::new("color", "Color"), |f, ((v, name), (_, n))| {
            f.option(*v, *name, n)
        })
        .chosen(p.color.clone());
    let brand = brands
        .into_iter()
        .fold(Facet::new("brand", "Brand"), |f, (v, n)| f.option(v, v, n))
        .chosen(p.brand.clone());
    FacetPanel::of(shop_listing, p)
        .facet(color)
        .facet(brand)
        .range(
            RangeSlider::new("Price", ["min_price", "max_price"], 0, TOP_PRICE)
                .step(10)
                .money(Currency::Try)
                .value(p.min_price, p.max_price),
        )
        .drives("shop")
}

/// The price of an item, with its old price when it is on sale.
fn price(i: &Item) -> Price {
    match i.old_price.minor {
        0 => Price::new(i.price),
        _ => Price::new(i.price).was(i.old_price),
    }
}

/// The cards of the listing, filtered on the server by the same params.
fn cards(p: &ShopParams) -> CardGrid<Item> {
    CardGrid::of(search_items)
        .card(|i: &Item| {
            Card::image(i.photo(), i.name.clone())
                .subtitle(i.brand.clone())
                .price(price(i))
                .low(i.low())
                .fallback(picture(1))
                .link(shop_product, ProductPage::of(i.id.0))
        })
        .placeholder(Placeholder::color("--z-surface-2"))
        .priority(4)
        .scope_any(Item::color, p.color.clone())
        .scope_any(Item::brand, p.brand.clone())
        .scope_range(
            Item::price,
            p.min_price.map(|m| Money::tl(m * 100)),
            p.max_price.map(|m| Money::tl(m * 100)),
        )
        .image_size(300, 300)
        .paging(Paging::infinite(24))
}

/// 2,000 products with facets, a price range and a card grid that follow each other in place.
#[screen(route = "/shop", public)]
pub fn shop_listing(p: ShopParams) -> Screen {
    let shown = items()
        .iter()
        .filter(|i| passes(i, &p, ""))
        .count();
    Screen::new("Shop")
        .subtitle("2,000 products")
        .body(
            Row::new()
                .col(facets(&p))
                .col(
                    Stack::new()
                        .add(Paragraph::new(format!("{shown} products")).listens("shop"))
                        .add(cards(&p).listens("shop")),
                )
                .weight(3),
        )
}

/// The product of the product page and its chosen variant.
#[params]
pub struct ProductPage {
    pub id: i64,
    pub color: Option<String>,
    pub size: Option<String>,
}

impl ProductPage {
    /// The page of item `id` with no variant chosen.
    pub fn of(id: i64) -> Self {
        ProductPage {
            id,
            color: None,
            size: None,
        }
    }
}

/// Units in stock of an item in a size; some sizes are out.
fn stock(id: i64, size: &str) -> i64 {
    match (id % 4, size) {
        (0, "xl") => 0,
        (1, "s") => 0,
        (_, "xl") => 3,
        _ => 5 + id % 9,
    }
}

/// Keeps a cart quantity up to the stock of `item-{id}-{size}`. Nothing is stored.
#[action(public)]
pub async fn set_shop_quantity(change: QuantityChange) -> Result<QuantityState> {
    let mut parts = change
        .target
        .strip_prefix("item-")
        .ok_or(Error::NotFound)?
        .split('-');
    let id: i64 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(Error::NotFound)?;
    let size = parts.next().ok_or(Error::NotFound)?;
    let left = stock(id, size);
    Ok(QuantityState::new(change.quantity.min(left)).max(left))
}

/// Pretends to add the item to a cart. Nothing is stored.
#[action(public)]
pub async fn add_to_cart(_id: i64) -> Result<()> {
    Ok(())
}

/// The photos of an item: five views, the first its listing photo.
fn gallery(id: i64, color: &str) -> ProductGallery {
    let shift = COLORS
        .iter()
        .position(|c| c.0 == color)
        .unwrap_or(0) as i64;
    (0..5).fold(ProductGallery::new(format!("Item {id}")), |g, k| {
        let n = id + k * 97 + shift * 11;
        g.image(
            Image::new(format!("/site/photo/{n}.svg"))
                .alt(format!("Item {id}, view {}", k + 1))
                .size(600, 600)
                .low(format!("/site/photo/{n}-low.svg"))
                .placeholder(Placeholder::color("--z-surface-2")),
        )
        .thumb(format!("/site/photo/{n}-low.svg"))
    })
}

/// A product page: gallery, price, variants, quantity and the add to cart button. Choosing a
/// color or a size refreshes the photos, the price and the stock in place.
#[screen(route = "/shop/product/{id}", public)]
pub fn shop_product(p: ProductPage, ctx: Ctx) -> Screen {
    let Some(item) = items().iter().find(|i| i.id.0 == p.id) else {
        return Screen::new("Product").body(EmptyState::new("No such product"));
    };
    let color = p
        .color
        .clone()
        .unwrap_or_else(|| item.color.clone());
    let size = p
        .size
        .clone()
        .unwrap_or_else(|| "m".into());
    let sizes = ["s", "m", "l", "xl"]
        .into_iter()
        .fold(VariantGroup::chips("size", "Size"), |g, s| {
            let g = g.option(s, s.to_uppercase());
            match stock(item.id.0, s) {
                0 => g.unavailable(s, "Sold out"),
                _ => g,
            }
        })
        .current(Some(&size));
    let colors = COLORS
        .iter()
        .zip([
            "#1d1f22", "#f2f0eb", "#c0392b", "#2f5d9a", "#3f7f5a", "#d8c3a5",
        ])
        .fold(
            VariantGroup::swatches("color", "Color"),
            |g, ((v, name), hex)| g.swatch(*v, *name, hex),
        )
        .current(Some(&color));
    let left = stock(item.id.0, &size);
    let extra = if size == "xl" { 2_000 } else { 0 };
    let every = 6 * 3_600_000;
    let sale_ends = DateTime::from_unix_ms((ctx.now().unix_ms() / every + 1) * every);
    let buy = Stack::new()
        .add(
            Countdown::until(sale_ends)
                .label("Sale ends in")
                .ended(Badge::new("The sale has ended")),
        )
        .add(
            price(item)
                .large()
                .unit(Money::tl((item.price.minor + extra) * 2), PriceUnit::Kg),
        )
        .add(Paragraph::new(match left {
            0 => "Sold out in this size".to_string(),
            n => format!("{n} in stock"),
        }))
        .add(
            QuantityStepper::of(set_shop_quantity, format!("item-{}-{size}", item.id.0))
                .range(1, 10)
                .value(1),
        )
        .add(
            ActionButton::new("Add to cart", add_to_cart, item.id.0)
                .tone(Tone::Brand)
                .on_success(|_, ui| ui.toast("Added. Nothing was stored.")),
        );
    let reviews = (1..=5u8).fold(ReviewSummary::new(), |s, n| {
        s.count(
            n,
            (item.id.0 as u64 * u64::from(n) * 7) % 97 + u64::from(n) * 20,
        )
    });
    Screen::new(item.name.clone())
        .subtitle(item.brand.clone())
        .body(
            Row::new()
                .col(gallery(item.id.0, &color).listens("variant"))
                .col(
                    Stack::new()
                        .add(
                            VariantPicker::new()
                                .group(colors)
                                .group(sizes)
                                .drives("variant"),
                        )
                        .add(buy.listens("variant")),
                ),
        )
        .body(
            Stack::new()
                .add(Heading::h2("Reviews"))
                .add(reviews),
        )
}
