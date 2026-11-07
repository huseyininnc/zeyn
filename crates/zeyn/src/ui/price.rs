//! [`Price`]: a price with an optional struck old price, the percent off and a unit price.

use super::layout::into_node;
use crate::i18n::{NumberFormat, Text};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use crate::value::Money;
use serde::Serialize;
use serde_json::{Value as Json, json};

into_node!(Price);

/// What a unit price is per, for [`Price::unit`]: "₺25.98 per kg".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceUnit {
    /// Per piece.
    Item,
    /// Per kilogram.
    Kg,
    /// Per 100 grams.
    Grams100,
    /// Per liter.
    Liter,
    /// Per 100 milliliters.
    Ml100,
    /// Per meter.
    Meter,
}

impl PriceUnit {
    fn key(self) -> &'static str {
        match self {
            PriceUnit::Item => "zeyn.ui.shop.price.per.item",
            PriceUnit::Kg => "zeyn.ui.shop.price.per.kg",
            PriceUnit::Grams100 => "zeyn.ui.shop.price.per.g100",
            PriceUnit::Liter => "zeyn.ui.shop.price.per.l",
            PriceUnit::Ml100 => "zeyn.ui.shop.price.per.ml100",
            PriceUnit::Meter => "zeyn.ui.shop.price.per.m",
        }
    }
}

/// A price as shops show it: the current price, the old price struck through with the percent
/// off when it was higher, and a unit price such as "₺25.98 per kg". Amounts are [`Money`] in the
/// page's number format (`₺1,299.00` in English, `1.299,00 ₺` in Turkish). It is server rendered
/// HTML with no script.
///
/// Screen readers hear one sentence, "Was ₺1,599.00, now ₺1,299.00, 19% off", instead of two
/// numbers whose strike through they would not read; the struck price and the badge are hidden
/// from them. An old price that is not higher than the current one, or in another currency, is
/// left out, so a stale discount never shows a negative percent.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Price, PriceUnit};
/// use zeyn::{Component, Render};
///
/// let price = Price::new(Money::tl(129_900))
///     .was(Money::tl(159_900))
///     .unit(Money::tl(25_980), PriceUnit::Kg);
/// let mut html = String::new();
/// price.render(&mut html, &Render::english("s"));
/// assert!(html.contains("Was ₺1,599.00, now ₺1,299.00, 19% off"));
/// assert!(html.contains("₺259.80 per kg"));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Price {
    now: Money,
    was: Option<Money>,
    unit: Option<(Money, PriceUnit)>,
    large: bool,
}

impl Price {
    /// The price the buyer pays now.
    pub fn new(now: Money) -> Self {
        Price {
            now,
            was: None,
            unit: None,
            large: false,
        }
    }

    /// The price before the discount, struck through next to the current one with the percent
    /// off. Left out unless it is higher than the current price and in the same currency.
    pub fn was(mut self, was: Money) -> Self {
        self.was = Some(was);
        self
    }

    /// A unit price under the price, such as the price per kilogram of a packed food.
    pub fn unit(mut self, price: Money, per: PriceUnit) -> Self {
        self.unit = Some((price, per));
        self
    }

    /// A larger current price, for the price on a product page.
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }

    /// The old price and the whole percent off, when the old price counts.
    fn discount(&self) -> Option<(Money, i64)> {
        let was = self.was?;
        if was.currency != self.now.currency || was.minor <= self.now.minor || was.minor <= 0 {
            return None;
        }
        let off = (i128::from(was.minor - self.now.minor) * 100 + i128::from(was.minor) / 2)
            / i128::from(was.minor);
        Some((was, off as i64))
    }

    /// The price as HTML, also used inside cards.
    pub(crate) fn html(&self, r: &Render) -> String {
        let raw_now = self.now.format(r.locale);
        let now = escape(&raw_now);
        let class = if self.large {
            "z-price z-price-lg"
        } else {
            "z-price"
        };
        let main = match self.discount() {
            Some((was, off)) => {
                let raw_was = was.format(r.locale);
                let pct = percent(off, r.locale);
                let said = r.text(
                    &Text::key("zeyn.ui.shop.price.sale")
                        .arg("was", raw_was.clone())
                        .arg("now", raw_now)
                        .arg("off", pct.clone()),
                );
                let was = escape(&raw_was);
                let badge = if off > 0 {
                    format!(r#"<span class="z-price-off">-{pct}</span>"#)
                } else {
                    String::new()
                };
                format!(
                    r#"<span class="z-sr-only">{said}</span><span class="z-price-row" aria-hidden="true"><span class="z-price-now z-price-sale">{now}</span><s class="z-price-was">{was}</s>{badge}</span>"#
                )
            }
            None => format!(
                r#"<span class="z-price-row"><span class="z-price-now">{now}</span></span>"#
            ),
        };
        let unit = self
            .unit
            .map(|(price, per)| {
                format!(
                    r#"<span class="z-price-unit">{}</span>"#,
                    r.text(&Text::key(per.key()).arg("price", price.format(r.locale)))
                )
            })
            .unwrap_or_default();
        format!(r#"<span class="{class}">{main}{unit}</span>"#)
    }
}

/// A whole percent in the locale's order: `19%`, `%19` in Turkish.
fn percent(n: i64, locale: &str) -> String {
    if NumberFormat::percent_first(locale) {
        format!("%{n}")
    } else {
        format!("{n}%")
    }
}

impl Component for Price {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&self.html(r));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "price",
            "now": self.now,
            "was": self.discount().map(|(was, _)| was),
            "percent_off": self.discount().map(|(_, off)| off),
            "unit": self.unit.map(|(price, per)| json!({ "price": price, "per": per })),
        })
    }
}
