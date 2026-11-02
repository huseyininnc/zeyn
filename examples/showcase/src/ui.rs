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

/// How a request is delivered.
#[choice]
pub enum Delivery {
    Pickup,
    Courier,
    Cargo,
}

/// Package size.
#[choice]
pub enum Size {
    Small,
    Medium,
    Large,
}

/// Warehouse city: a long list for the searchable listbox.
#[choice]
pub enum City {
    Adana,
    Ankara,
    Antalya,
    Bursa,
    Eskisehir,
    Gaziantep,
    Istanbul,
    Izmir,
    Kayseri,
    Konya,
    Mersin,
    Trabzon,
}

/// A stock request that uses every control.
#[model]
pub struct StockRequest {
    #[rule(required)]
    pub product: Option<crate::stock::ProductId>,
    pub alternatives: Vec<i64>,
    #[rule(min = 1, max = 500)]
    pub quantity: i64,
    #[rule(min = 0, max = 1000000)]
    pub budget: Money,
    pub priority: crate::gallery::Priority,
    pub delivery: Delivery,
    pub size: Size,
    pub warehouse: Option<City>,
    pub urgent: bool,
    pub insured: bool,
    #[rule(max_len = 500)]
    pub note: Option<String>,
}

#[action(permission = "stock.product.read")]
pub async fn find_products(q: LookupQuery, store: crate::stock::ProductStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|p| LookupItem::new(p.id.0, p.name).hint(p.code)).collect();
    Ok(q.apply(items))
}

#[action(public)]
pub async fn send_request(request: Valid<StockRequest>) -> Result<StockRequest> {
    Ok(request.into_inner())
}

#[screen(route = "/controls", permission = "stock.product.read")]
pub fn controls() -> Screen {
    let request = StockRequest {
        alternatives: vec![2, 5],
        quantity: 12,
        budget: Money::tl(2_500_000),
        size: Size::Medium,
        insured: true,
        ..StockRequest::default()
    };
    Screen::new(t!("ui.controls")).body(
        Panel::new(t!("ui.panel_request")).add(
            Form::of(request)
                .lookup(StockRequest::product, find_products)
                .lookup_many(StockRequest::alternatives, find_products)
                .row(|r| r.field(StockRequest::quantity).field(StockRequest::budget).field(StockRequest::priority))
                .row(|r| r.field(StockRequest::delivery).field(StockRequest::size))
                .control(StockRequest::delivery, Control::Radio)
                .control(StockRequest::size, Control::Segmented)
                .field(StockRequest::warehouse)
                .control(StockRequest::warehouse, Control::Listbox)
                .row(|r| r.field(StockRequest::urgent).field(StockRequest::insured))
                .control(StockRequest::urgent, Control::Switch)
                .field(StockRequest::note)
                .submit(t!("ui.send"), send_request)
                .on_success(|_, ui| ui.toast(t!("ui.sent"))),
        ),
    )
}

pub fn feature() -> Feature {
    Feature::new("ui")
        .route_prefix("/ui")
        .guard(SignedIn)
        .screen(layout)
        .screen(editors)
        .screen(controls)
        .menu(|m| {
            m.group(t!("menu.ui"), Icon::Table)
                .item(t!("ui.layout"), layout)
                .item(t!("ui.editors"), editors)
                .item(t!("ui.controls"), controls)
        })
}
