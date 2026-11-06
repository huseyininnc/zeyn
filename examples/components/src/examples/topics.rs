//! Topics examples: filters that drive a grid, a chart and a KPI through the screen params, a
//! date range and a search box that drive a list, and a button that refreshes a node.

use crate::data::{Kind, Product, count_products, products, search_products, week, week_events};
use crate::{Demo, Entry, exd};
use zeyn::prelude::*;
use zeyn::ui::ListItem;

/// The kind a filter chip stands for.
fn kind_of(value: &str) -> Option<Kind> {
    match value {
        "Book" => Some(Kind::Book),
        "Tool" => Some(Kind::Tool),
        "Plant" => Some(Kind::Plant),
        _ => None,
    }
}

/// The grid, chart and KPI that listen to `shop` and show the kind in the params.
fn shop(demo: &Demo) -> Node {
    let shown: Vec<&Product> = products()
        .iter()
        .filter(|p| demo.kind.is_none_or(|k| p.kind == k))
        .collect();
    let stock: i64 = shown.iter().map(|p| p.stock).sum();
    let mut grid = DataGrid::of(search_products);
    if let Some(kind) = demo.kind {
        grid = grid.scope(Product::kind, kind);
    }
    let grid = grid
        .column(Product::name)
        .column(Product::kind)
        .column(Product::stock)
        .paging(Paging::pages(5));
    Stack::new()
        .add(
            Row::new()
                .col(Kpi::new("Units in stock", stock as f64).listens("shop"))
                .col(
                    Chart::bar("Stock of the first six")
                        .labels(
                            shown
                                .iter()
                                .take(6)
                                .map(|p| p.code.clone()),
                        )
                        .series(
                            "Units",
                            shown
                                .iter()
                                .take(6)
                                .map(|p| p.stock as f64),
                        )
                        .listens("shop"),
                ),
        )
        .add(grid.listens("shop"))
        .into()
}

pub fn overview(_ctx: &Ctx, demo: &Demo) -> Node {
    let keep = demo.clone();
    Stack::new()
        .add(
            Chips::filter("Kind", crate::topics, move |v| Demo {
                kind: v.first().and_then(|v| kind_of(v)),
                ..keep.clone()
            })
            .current(demo.kind.map(|k| format!("{k:?}")))
            .option("Book", "Books")
            .option("Tool", "Tools")
            .option("Plant", "Plants")
            .drives("shop"),
        )
        .add(shop(demo))
        .into()
}

/// The fields of the filter form: screen params of the same names.
#[model]
pub struct ShopFilter {
    pub kind: Option<Kind>,
}

pub fn select(_ctx: &Ctx, demo: &Demo) -> Node {
    Stack::new()
        .add(
            Form::of(ShopFilter { kind: demo.kind })
                .field(ShopFilter::kind)
                .drives("shop"),
        )
        .add(shop(demo))
        .into()
}

/// The date range of the week filter.
#[model]
pub struct WeekFilter {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

pub fn date_range_and_search(ctx: &Ctx, demo: &Demo) -> Node {
    let q = demo
        .q
        .clone()
        .unwrap_or_default()
        .to_lowercase();
    let list = week(ctx.today())
        .into_iter()
        .filter(|e| {
            e.day
                .is_some_and(|d| demo.from.is_none_or(|f| d >= f) && demo.to.is_none_or(|t| d <= t))
                && e.title.to_lowercase().contains(&q)
        })
        .fold(List::new().title("This week"), |list, e| {
            let day = e
                .day
                .map(|d| d.to_string())
                .unwrap_or_default();
            list.item(
                ListItem::new(e.title)
                    .subtitle(day)
                    .meta(e.starts),
            )
        });
    Stack::new()
        .add(
            Form::of(WeekFilter {
                from: demo.from,
                to: demo.to,
            })
            .range(WeekFilter::from, WeekFilter::to)
            .drives("week"),
        )
        .add(
            SearchBox::of(week_events)
                .label("Search events")
                .value(demo.q.clone().unwrap_or_default())
                .drives("week"),
        )
        .add(list.listens("week"))
        .into()
}

pub fn refresh_after_an_action(ctx: &Ctx, _demo: &Demo) -> Node {
    let now = ctx.now().local(ctx.time_zone());
    Stack::new()
        .add(
            ActionButton::new("Count products", count_products, ())
                .on_success(|_, ui| ui.refresh("clock")),
        )
        .add(Paragraph::new(format!("Rendered at {now}")).listens("clock"))
        .into()
}

/// What the catalog shows of topics.
pub const ENTRY: Entry = Entry {
    name: "Topics",
    docs: "effects#topics",
    api: &["Effect", "Chips", "Form", "SearchBox"],
    overview: &[exd!(overview)],
    examples: &[
        exd!(select),
        exd!(date_range_and_search),
        exd!(refresh_after_an_action),
    ],
};
