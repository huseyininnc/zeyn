//! Barcode and QrCode examples: Code 128 and QR codes drawn as SVG on the server.

use crate::{Entry, ex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .col(Barcode::code128("SO-2026-1042"))
        .col(QrCode::new("https://example.com/orders/1042").label("Order SO-2026-1042"))
        .into()
}

pub fn levels(_ctx: &Ctx) -> Node {
    Row::new()
        .col(
            QrCode::new("https://example.com/p/TEA-01")
                .level(QrLevel::Low)
                .size(120)
                .label("Low"),
        )
        .col(
            QrCode::new("https://example.com/p/TEA-01")
                .level(QrLevel::High)
                .size(120)
                .label("High"),
        )
        .into()
}

pub fn digits(_ctx: &Ctx) -> Node {
    Barcode::code128("4006381333931")
        .height(48)
        .label("Tea, 500 g · 4006381333931")
        .into()
}

/// What the catalog shows of `Barcode`.
pub const ENTRY: Entry = Entry {
    name: "Barcode",
    docs: "components/admin#barcode-and-qrcode",
    api: &["Barcode", "QrCode"],
    overview: &[ex!(overview)],
    examples: &[ex!(levels), ex!(digits)],
};
