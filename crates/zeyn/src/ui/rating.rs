//! The rating control of number form fields ([`super::Control::Rating`]): a group of radio
//! buttons from 1 to N drawn as stars, in whole stars for whole number fields and in half stars
//! for `Decimal` fields. It needs no script: native radio buttons already follow the W3C APG
//! radio group pattern (one tab stop, arrow keys move and check).

use super::Slot;
use crate::i18n::{NumberFormat, Text};
use crate::render::Render;
use crate::value::{FieldKind, Value};

/// Most stars a rating shows.
pub const MAX_RATING: i64 = 10;

/// Stars a rating shows when its field has no `max` rule.
pub const DEFAULT_RATING: i64 = 5;

/// How many stars a field gets: its `max` rule, from 1 to [`MAX_RATING`], or [`DEFAULT_RATING`].
pub(crate) fn stars(max: Option<i64>) -> i64 {
    max.unwrap_or(DEFAULT_RATING)
        .clamp(1, MAX_RATING)
}

/// A value in half stars (`9` for 4.5), when it is a whole or half number.
pub(crate) fn halves(value: &Value) -> Option<i64> {
    let v = match value {
        Value::Int(n) => *n as f64,
        Value::Number(d) => d.to_string().parse::<f64>().ok()?,
        _ => return None,
    };
    let h = v * 2.0;
    (h.fract() == 0.0).then_some(h as i64)
}

/// `n` half stars as the page writes a number: `4.5` in English, `4,5` in Turkish.
fn half_text(n: i64, locale: &str) -> String {
    match n % 2 {
        0 => (n / 2).to_string(),
        _ => format!("{}{}5", n / 2, NumberFormat::for_locale(locale).decimal),
    }
}

/// Writes a rating: a fieldset with the field label as its legend and one radio button per
/// star, or per half star for a `Decimal` field. Each is named "3 of 5" ("4.5 of 5") for screen
/// readers; the first radio carries the field id, so validation messages land under the group.
pub(crate) fn render_rating(out: &mut String, r: &Render, s: &Slot) {
    let max = stars(s.m.max());
    let half = s.m.kind == FieldKind::Decimal;
    let step = if half { 1 } else { 2 };
    let current = halves(s.value).unwrap_or(0);
    let required = s.m.is_required();
    let star = if required {
        r#"<span class="z-required" aria-hidden="true">*</span>"#
    } else {
        ""
    };
    let class = if half {
        "z-rating z-rating-halves"
    } else {
        "z-rating"
    };
    out.push_str(&format!(
        r#"<fieldset class="{class}" aria-describedby="{}-error"><legend>{}{star}</legend><div class="z-rating-list">"#,
        s.id,
        r.label(s.m)
    ));
    let required = if required { " required" } else { "" };
    let disabled = if s.m.readonly { " disabled" } else { "" };
    for n in (step..=max * 2).step_by(step as usize) {
        let value = half_text(n, r.locale);
        let key = value.replace([',', '.'], "_");
        let id = if n == step {
            s.id.to_string()
        } else {
            format!("{}-{key}", s.id)
        };
        let checked = if n == current { " checked" } else { "" };
        let name = r.text(
            &Text::key("zeyn.ui.rating.value")
                .arg("n", value.clone())
                .arg("max", max.to_string()),
        );
        let class = match (half, n % 2) {
            (false, _) => "z-rating-star",
            (true, 1) => "z-rating-star z-rating-half",
            (true, _) => "z-rating-star z-rating-half z-rating-half-end",
        };
        out.push_str(&format!(
            r#"<label class="{class}" for="{id}"><input type="radio" class="z-rating-input" id="{id}" name="{}" value="{value}" data-testid="{}.field.{}.{key}"{checked}{required}{disabled}><span class="z-rating-mark" aria-hidden="true">&#9733;</span><span class="z-sr-only">{name}</span></label>"#,
            s.m.name,
            r.scope,
            s.m.name
        ));
    }
    out.push_str("</div></fieldset>");
}
