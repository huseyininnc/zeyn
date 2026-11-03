//! Line editor examples: rows of a model inside a form.

use crate::{Entry, ex};
use zeyn::prelude::*;

/// One invoice line.
#[model]
pub struct Line {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

/// An invoice with lines.
#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<Line>,
}

/// Adds up a valid invoice. Nothing is stored.
#[action(public)]
pub async fn total_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}

pub fn overview(_ctx: &Ctx) -> Node {
    Form::of(Invoice {
        customer: "Northwind".into(),
        lines: vec![
            Line { item: "Notebook".into(), quantity: 12, unit_price: Money::tl(8_369) },
            Line { item: "Pencil".into(), quantity: 40, unit_price: Money::tl(1_250) },
        ],
    })
    .field(Invoice::customer)
    .lines(Invoice::lines, |l| {
        l.column(Line::item).column(Line::quantity).column(Line::unit_price).total(Line::quantity, Line::unit_price)
    })
    .submit("Save", total_invoice)
    .on_success(|_, ui| ui.toast("Saved. Nothing was stored."))
    .into()
}

/// What the catalog shows of `Lines`.
pub const ENTRY: Entry =
    Entry { name: "Line editor", docs: "components/lines", api: &["Lines"], overview: &[ex!(overview)], examples: &[] };
