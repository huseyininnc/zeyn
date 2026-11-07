//! VariantPicker examples: colors and sizes that change the price, the photo and the stock.

use crate::{Demo, Entry, exd};
use zeyn::prelude::*;

/// The colors of the sample shirt: value, name, swatch and picture.
const COLORS: [(&str, &str, &str, i64); 3] = [
    ("sand", "Sand", "#d8c3a5", 2),
    ("navy", "Navy", "#1f3a68", 6),
    ("sage", "Sage", "#8fa98b", 9),
];

/// Units in stock of a color and a size; navy has no XL.
fn stock(color: &str, size: &str) -> i64 {
    match (color, size) {
        ("navy", "xl") => 0,
        ("sage", "s") => 2,
        (_, "xl") => 4,
        _ => 12,
    }
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let color = demo.color.as_deref().unwrap_or("sand");
    let size = demo.size.as_deref().unwrap_or("m");
    let sizes = ["s", "m", "l", "xl"]
        .into_iter()
        .fold(VariantGroup::chips("size", "Size"), |g, s| {
            let g = g.option(s, s.to_uppercase());
            match stock(color, s) {
                0 => g.unavailable(s, "Not in this color"),
                _ => g,
            }
        })
        .current(Some(size));
    let colors = COLORS
        .iter()
        .fold(
            VariantGroup::swatches("color", "Color"),
            |g, (v, n, c, _)| g.swatch(*v, *n, *c),
        )
        .current(Some(color));
    let picture = COLORS
        .iter()
        .find(|c| c.0 == color)
        .map_or(1, |c| c.3);
    let price = if size == "xl" { 34_900 } else { 29_900 };
    let left = stock(color, size);
    Row::new()
        .col(
            Image::new(crate::data::picture(picture))
                .alt(format!("Linen shirt in {color}"))
                .size(240, 240)
                .listens("shirt"),
        )
        .col(
            Stack::new()
                .add(
                    VariantPicker::new()
                        .group(colors)
                        .group(sizes)
                        .drives("shirt"),
                )
                .add(
                    Stack::new()
                        .add(Price::new(Money::tl(price)).large())
                        .add(Paragraph::new(match left {
                            0 => "Out of stock".to_string(),
                            n => format!("{n} in stock"),
                        }))
                        .listens("shirt"),
                ),
        )
        .into()
}

/// What the catalog shows of `VariantPicker`.
pub const ENTRY: Entry = Entry {
    name: "VariantPicker",
    docs: "components/commerce#variant-picker",
    api: &["VariantPicker", "VariantGroup"],
    overview: &[exd!(overview)],
    examples: &[],
};
