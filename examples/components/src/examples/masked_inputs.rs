//! Masked, decimal, percent and password inputs.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// Terms of an account.
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(min_len = 10, max_len = 128)]
    pub password: String,
}

/// Accepts valid terms. Nothing is stored.
#[action(public)]
pub async fn save_terms(terms: Valid<Terms>) -> Result<String> {
    Ok(terms.phone.clone())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Terms {
        rate: Decimal::parse("1.0825").unwrap_or_default(),
        ..Terms::default()
    })
    .row(|r| {
        r.field(Terms::rate)
            .field(Terms::discount)
    })
    .control(Terms::discount, Control::Percent)
    .field(Terms::phone)
    .field(Terms::password)
    .control(Terms::password, Control::PasswordMeter)
    .submit("Save", save_terms)
    .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
    .into()
}

/// What the catalog shows of `Control`.
pub const ENTRY: Entry = Entry {
    name: "Masked inputs",
    docs: "components/controls",
    api: &["Form"],
    overview: &[ex!(overview)],
    examples: &[],
};
