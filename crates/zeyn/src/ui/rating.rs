//! The rating control of whole number form fields ([`super::Control::Rating`]): a group of radio
//! buttons from 1 to N drawn as stars. It needs no script: native radio buttons already follow
//! the W3C APG radio group pattern (one tab stop, arrow keys move and check).

use super::Slot;
use crate::i18n::Text;
use crate::render::Render;
use crate::value::Value;

/// Most stars a rating shows.
pub const MAX_RATING: i64 = 10;

/// Stars a rating shows when its field has no `max` rule.
pub const DEFAULT_RATING: i64 = 5;

/// How many stars a field gets: its `max` rule, from 1 to [`MAX_RATING`], or [`DEFAULT_RATING`].
pub(crate) fn stars(max: Option<i64>) -> i64 {
    max.unwrap_or(DEFAULT_RATING)
        .clamp(1, MAX_RATING)
}

/// Writes a rating: a fieldset with the field label as its legend and one radio button per
/// star. Each star is named "3 of 5" for screen readers; the first radio carries the field id, so
/// validation messages land under the group.
pub(crate) fn render_rating(out: &mut String, r: &Render, s: &Slot) {
    let max = stars(s.m.max());
    let current = match s.value {
        Value::Int(n) => *n,
        _ => 0,
    };
    let required = s.m.is_required();
    let star = if required { r#"<span class="z-required" aria-hidden="true">*</span>"# } else { "" };
    out.push_str(&format!(
        r#"<fieldset class="z-rating" aria-describedby="{}-error"><legend>{}{star}</legend><div class="z-rating-list">"#,
        s.id,
        r.label(s.m)
    ));
    let required = if required { " required" } else { "" };
    let disabled = if s.m.readonly { " disabled" } else { "" };
    for n in 1..=max {
        let id = if n == 1 { s.id.to_string() } else { format!("{}-{n}", s.id) };
        let checked = if n == current { " checked" } else { "" };
        let name = r.text(
            &Text::key("zeyn.ui.rating.value")
                .arg("n", n.to_string())
                .arg("max", max.to_string()),
        );
        out.push_str(&format!(
            r#"<label class="z-rating-star" for="{id}"><input type="radio" class="z-rating-input" id="{id}" name="{}" value="{n}" data-testid="{}.field.{}.{n}"{checked}{required}{disabled}><span class="z-rating-mark" aria-hidden="true">&#9733;</span><span class="z-sr-only">{name}</span></label>"#,
            s.m.name,
            r.scope,
            s.m.name
        ));
    }
    out.push_str("</div></fieldset>");
}
