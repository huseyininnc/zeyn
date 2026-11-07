//! [`ReviewSummary`]: the average rating, the number of reviews and a histogram from 5 to 1.

use super::layout::into_node;
use crate::i18n::{NumberFormat, Text};
use crate::render::{Render, push_grouped};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(ReviewSummary);

/// The reviews of a product at a glance: the average as a number and as stars, how many reviews
/// there are, and one bar per star from 5 to 1 with its share. The average and the shares are
/// computed from the counts, so they always agree. Screen readers hear "4.6 out of 5 stars" for
/// the stars and "5 stars, 68%" for each bar; the bars themselves are hidden from them. It is
/// server rendered HTML with no script.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::ReviewSummary;
/// use zeyn::{Component, Render};
///
/// let summary = ReviewSummary::new()
///     .count(5, 120)
///     .count(4, 40)
///     .count(3, 10)
///     .count(1, 6);
/// let mut html = String::new();
/// summary.render(&mut html, &Render::english("s"));
/// assert!(html.contains("4.5 out of 5 stars"));
/// assert!(html.contains("176 reviews"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewSummary {
    counts: [u64; 5],
}

impl ReviewSummary {
    /// A summary with no reviews yet.
    pub fn new() -> Self {
        ReviewSummary::default()
    }

    /// How many reviews gave `stars` (1 to 5) stars; other star counts are ignored.
    pub fn count(mut self, stars: u8, reviews: u64) -> Self {
        if (1..=5).contains(&stars) {
            self.counts[usize::from(stars - 1)] = reviews;
        }
        self
    }

    fn total(&self) -> u64 {
        self.counts
            .iter()
            .fold(0u64, |a, b| a.saturating_add(*b))
    }

    /// The average in tenths of a star, rounded: `46` for 4.6.
    fn tenths(&self) -> u64 {
        let total = self.total();
        if total == 0 {
            return 0;
        }
        let sum: u128 = self
            .counts
            .iter()
            .enumerate()
            .map(|(i, n)| u128::from(*n) * (i as u128 + 1))
            .sum();
        ((sum * 10 + u128::from(total) / 2) / u128::from(total)) as u64
    }
}

impl Component for ReviewSummary {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| Text::key(format!("zeyn.ui.shop.reviews.{key}"));
        let total = self.total();
        let tenths = self.tenths();
        let average = format!(
            "{}{}{}",
            tenths / 10,
            NumberFormat::for_locale(r.locale).decimal,
            tenths % 10
        );
        let mut count = String::new();
        push_grouped(&mut count, total as i64, r.locale);
        out.push_str(&format!(
            r#"<section class="z-reviews" aria-label="{}"><div class="z-reviews-head"><span class="z-reviews-average" aria-hidden="true">{average}</span><span class="z-stars" role="img" aria-label="{}" style="--z-stars:{}%"></span><span class="z-reviews-count">{}</span></div><ul class="z-reviews-bars">"#,
            r.text(&t("label")),
            r.text(&t("average").arg("average", average.clone())),
            tenths * 2,
            r.text(&t("count").arg("count", count)),
        ));
        for stars in (1..=5u8).rev() {
            let n = self.counts[usize::from(stars - 1)];
            let share = match total {
                0 => 0,
                _ => (u128::from(n) * 100 + u128::from(total) / 2) / u128::from(total),
            };
            let pct = if NumberFormat::percent_first(r.locale) {
                format!("%{share}")
            } else {
                format!("{share}%")
            };
            out.push_str(&format!(
                r#"<li class="z-reviews-row"><span class="z-reviews-star">{}</span><span class="z-reviews-bar" aria-hidden="true"><span class="z-reviews-fill" style="--z-share:{share}%"></span></span><span class="z-reviews-share">{pct}</span></li>"#,
                r.text(&t("stars").arg("n", stars.to_string())),
            ));
        }
        out.push_str("</ul></section>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "review_summary",
            "counts": self.counts,
            "total": self.total(),
            "average": self.tenths() as f64 / 10.0,
        })
    }
}
