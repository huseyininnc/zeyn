//! Accordion examples: sections that open and close under their titles.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Accordion::new()
        .section(
            "How do I reset my password?",
            Paragraph::new("Use the link on the sign in page. The email arrives in a minute."),
        )
        .open()
        .section(
            "Where are my invoices?",
            Paragraph::new("Under Billing, then Invoices. Each one downloads as a PDF."),
        )
        .section(
            "Can I change my plan?",
            Paragraph::new("Yes, at any time. The new price starts with the next invoice."),
        )
        .into()
}

pub fn one_at_a_time(_ctx: &Ctx) -> Node {
    Accordion::new()
        .section(
            "Shipping",
            Paragraph::new("Orders placed before 14:00 leave the same day."),
        )
        .open()
        .section(
            "Returns",
            Paragraph::new("Send it back within 30 days for a full refund."),
        )
        .section(
            "Warranty",
            Stack::new()
                .add(Paragraph::new("Two years on every tool."))
                .add(Badge::new("Registered products get a third year").tone(Tone::Success)),
        )
        .single()
        .into()
}

/// What the catalog shows of `Accordion`.
pub const ENTRY: Entry = Entry {
    name: "Accordion",
    docs: "components/layouts#accordion",
    api: &["Accordion"],
    overview: &[ex!(overview)],
    examples: &[ex!(one_at_a_time)],
};
