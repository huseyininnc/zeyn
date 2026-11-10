//! DocumentViewer examples: a 500 page report preview whose pages are SVG drawn on the server,
//! with an outline and a search.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::{DocPage, DocSearch, DocumentViewer, OutlineItem, PageHit, PageSize};

/// Pages of the sample report.
const PAGES: u32 = 500;

/// Rows of the sample table on every page.
const ROWS: u32 = 28;

/// Regions the sample orders come from.
const REGIONS: [&str; 5] = ["North", "South", "East", "West", "Central"];

/// The region and amount of order `n` of the sample report, the same on every request.
fn order(n: u32) -> (&'static str, u32) {
    let mix = n
        .wrapping_mul(2_654_435_761)
        .rotate_left(7);
    (REGIONS[(mix % 5) as usize], 100 + mix % 9_900)
}

/// Draws one page of the sample report as SVG in points of an A4 page: a header, a table of
/// orders and a footer with the page number.
#[action(public)]
pub async fn report_page(p: DocPage) -> Result<String> {
    let mut svg = String::from(
        r##"<svg viewBox="0 0 595 842" font-family="Helvetica, Arial, sans-serif"><rect width="595" height="842" fill="#fff"/><text x="40" y="56" font-size="18" font-weight="bold" fill="#1d2939">Sales by order</text><text x="555" y="56" font-size="10" text-anchor="end" fill="#667085">Quarterly report</text><line x1="40" y1="70" x2="555" y2="70" stroke="#d0d5dd"/><rect x="40" y="84" width="515" height="22" fill="#f2f4f7"/><text x="48" y="99" font-size="10" font-weight="bold" fill="#344054">Order</text><text x="200" y="99" font-size="10" font-weight="bold" fill="#344054">Region</text><text x="547" y="99" font-size="10" font-weight="bold" text-anchor="end" fill="#344054">Amount</text>"##,
    );
    for row in 0..ROWS {
        let n = (p.page - 1) * ROWS + row + 1;
        let (region, amount) = order(n);
        let y = 124 + row * 24;
        svg.push_str(&format!(
            r##"<text x="48" y="{y}" font-size="10" fill="#1d2939">#{n:05}</text><text x="200" y="{y}" font-size="10" fill="#1d2939">{region}</text><text x="547" y="{y}" font-size="10" text-anchor="end" fill="#1d2939">{amount}.00</text><line x1="40" y1="{l}" x2="555" y2="{l}" stroke="#eaecf0"/>"##,
            l = y + 8,
        ));
    }
    svg.push_str(&format!(
        r##"<text x="297" y="812" font-size="9" text-anchor="middle" fill="#667085">Page {} of {PAGES}</text></svg>"##,
        p.page
    ));
    Ok(svg)
}

/// Finds the pages with orders from a region, by its name.
#[action(public)]
pub async fn search_report(q: DocSearch) -> Result<Vec<PageHit>> {
    let text = q.text.to_lowercase();
    Ok((1..=PAGES)
        .filter_map(|page| {
            let rows = (0..ROWS).map(|r| (page - 1) * ROWS + r + 1);
            let found = rows
                .map(|n| (n, order(n).0))
                .find(|(_, region)| region.to_lowercase().contains(&text))?;
            Some(PageHit::new(
                page,
                format!("Order #{:05}, {}", found.0, found.1),
            ))
        })
        .collect())
}

/// The outline of the sample report: a part per 100 pages with a chapter per 25.
fn outline() -> Vec<OutlineItem> {
    (0..PAGES / 100)
        .map(|part| {
            (0..4).fold(
                OutlineItem::new(format!("Part {}", part + 1), part * 100 + 1),
                |item, ch| {
                    let page = part * 100 + ch * 25 + 1;
                    item.child(OutlineItem::new(format!("Orders from page {page}"), page))
                },
            )
        })
        .collect()
}

/// A 500 page report: only the pages near the view are fetched.
pub fn overview(_ctx: &Ctx) -> Node {
    DocumentViewer::of(report_page, PAGES)
        .document("sales")
        .title("Sales by order")
        .page_size(PageSize::A4)
        .outline(outline())
        .search(search_report)
        .into()
}

/// The same report without side panels, in US Letter.
pub fn plain(_ctx: &Ctx) -> Node {
    DocumentViewer::of(report_page, 12)
        .document("short")
        .page_size(PageSize::LETTER)
        .into()
}

/// What the catalog shows of `DocumentViewer`.
pub const ENTRY: Entry = Entry {
    name: "DocumentViewer",
    docs: "components/display#documentviewer",
    api: &["DocumentViewer"],
    overview: &[ex!(overview)],
    examples: &[ex!(plain)],
};
