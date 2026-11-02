//! Component library tour: layout containers, editors and display components.

use crate::stock::{Product, search_products};
use zeyn::prelude::*;

fn products() -> Node {
    Grid::of(search_products)
        .paging(Paging::pages(8))
        .column(Product::code)
        .width(110)
        .filter()
        .column(Product::name)
        .flex(1)
        .column(Product::price)
        .width(130)
        .into()
}

#[screen(route = "/layout", permission = "stock.product.read")]
pub fn layout() -> Screen {
    Screen::new(t!("ui.layout")).body(
        Stack::new()
            .add(
                Row::new()
                    .col(Card::new().title(t!("ui.card_orders")).add(Card::new().title(t!("ui.card_hint"))))
                    .col(Card::new().title(t!("ui.card_stock")))
                    .col(Card::new().title(t!("ui.card_people"))),
            )
            .add(
                Panel::new(t!("ui.panel_products"))
                    .collapsible()
                    .toolbar(|t| t.export(Export::Csv))
                    .add(
                        Tabs::new(t!("ui.tabs_label"))
                            .tab(t!("ui.tab_all"), products())
                            .tab(t!("ui.tab_split"), Split::new(products(), Card::new().title(t!("ui.card_detail"))).ratio(55))
                            .tab(t!("ui.tab_columns"), Row::new().col(products()).weight(2).col(Card::new().title(t!("ui.card_hint")))),
                    ),
            )
            .add(Panel::new(t!("ui.panel_collapsed")).collapsed().add(Card::new().title(t!("ui.card_hint")))),
    )
}

/// A visit booked with the date and time editors.
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
    pub slot: LocalDateTime,
    pub confirmed_at: DateTime,
    pub stay_from: Option<Date>,
    pub stay_to: Option<Date>,
}

#[action(public)]
pub async fn book_visit(visit: Valid<Visit>) -> Result<Visit> {
    Ok(visit.into_inner())
}

#[screen(route = "/editors", public)]
pub fn editors(_: (), ctx: Ctx) -> Screen {
    let today = ctx.today();
    let visit = Visit {
        day: Some(today),
        arrives: Time::new(9, 30, 0).unwrap_or_default(),
        slot: LocalDateTime::new(today, Time::new(14, 0, 0).unwrap_or_default()),
        confirmed_at: ctx.now(),
        ..Visit::default()
    };
    Screen::new(t!("ui.editors")).body(
        Panel::new(t!("ui.panel_dates")).add(
            Form::of(visit)
                .row(|r| r.field(Visit::day).field(Visit::arrives))
                .row(|r| r.field(Visit::slot).field(Visit::confirmed_at))
                .range(Visit::stay_from, Visit::stay_to)
                .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
                .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
                .submit(t!("ui.book"), book_visit)
                .on_success(|_, ui| ui.toast(t!("ui.booked"))),
        ),
    )
}

pub fn feature() -> Feature {
    Feature::new("ui")
        .route_prefix("/ui")
        .guard(SignedIn)
        .screen(layout)
        .screen(editors)
        .menu(|m| m.group(t!("menu.ui"), Icon::Table).item(t!("ui.layout"), layout).item(t!("ui.editors"), editors))
}
