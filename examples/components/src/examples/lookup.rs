//! Lookup examples: pick a record from a server side search.

use crate::data::{ProductId, find_products};
use crate::{Entry, ex};
use zeyn::prelude::*;

/// An order line that refers to a product.
#[model]
pub struct Pick {
    #[rule(required)]
    pub product: Option<ProductId>,
}

/// Accepts the pick. Nothing is stored.
#[action(public)]
pub async fn save_pick(pick: Valid<Pick>) -> Result<bool> {
    Ok(pick.product.is_some())
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Pick::default())
        .lookup(Pick::product, find_products)
        .field(Pick::product)
        .submit("Save", save_pick)
        .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
        .into()
}

/// What the catalog shows of `LookupItem`.
pub const ENTRY: Entry = Entry {
    name: "Lookup",
    docs: "components/controls",
    api: &["LookupItem", "LookupQuery"],
    overview: &[ex!(overview)],
    examples: &[],
};
