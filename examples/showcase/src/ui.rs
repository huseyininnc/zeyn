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

/// One invoice line.
#[model]
pub struct InvoiceLine {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

/// An invoice with its lines.
#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    pub issued: Date,
    #[rule(required)]
    pub lines: Vec<InvoiceLine>,
}

#[action(public)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    let minor = invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum();
    Ok(Money::tl(minor))
}

#[screen(route = "/invoice", public)]
pub fn invoice_form(_: (), ctx: Ctx) -> Screen {
    let line = |item: &str, quantity, kurus| InvoiceLine { item: item.into(), quantity, unit_price: Money::tl(kurus) };
    let invoice = Invoice {
        customer: "Anadolu Gıda".into(),
        issued: ctx.today(),
        lines: vec![line("Green tea 105", 12, 8369), line("Olive oil 115", 4, 24207), line("Pine honey 120", 2, 32126)],
    };
    Screen::new(t!("ui.invoice")).body(
        Panel::new(t!("ui.panel_invoice")).add(
            Form::of(invoice)
                .row(|r| r.field(Invoice::customer).field(Invoice::issued))
                .lines(Invoice::lines, |l| {
                    l.column(InvoiceLine::item)
                        .column(InvoiceLine::quantity)
                        .column(InvoiceLine::unit_price)
                        .total(InvoiceLine::quantity, InvoiceLine::unit_price)
                })
                .submit(t!("ui.save_invoice"), save_invoice)
                .on_success(|total, ui| {
                    if let Ok(t) = total {
                        ui.toast(t!("ui.invoice_saved", total = t.format("tr")));
                    }
                }),
        ),
    )
}

#[action(permission = "stock.product.write")]
pub async fn archive_samples(count: i64) -> Result<i64> {
    Ok(count)
}

#[action(permission = "stock.product.read")]
pub async fn check_stock(_: ()) -> Result<i64> {
    Ok(3)
}

#[screen(route = "/widgets", permission = "stock.product.read")]
pub fn widgets(_: (), ctx: Ctx) -> Screen {
    let now = ctx.now();
    let hour = 3_600_000;
    let invoice = Invoice { customer: "Anadolu Gıda".into(), issued: ctx.today(), lines: Vec::new() };
    Screen::new(t!("ui.widgets")).body(
        Stack::new()
            .add(Breadcrumb::new().link(t!("ui.layout"), layout, ()).current(t!("ui.widgets")))
            .add(
                Row::new()
                    .col(
                        Stack::new()
                            .gap(2)
                            .add(Alert::info(t!("ui.alert_info")).dismissible())
                            .add(Alert::success(t!("ui.alert_success")))
                            .add(Alert::warning(t!("ui.alert_warning")).title(t!("ui.alert_warning_title")))
                            .add(Alert::danger(t!("ui.alert_danger"))),
                    )
                    .col(
                        Card::new()
                            .title(t!("ui.card_invoice"))
                            .add(
                                DescriptionList::of(&invoice)
                                    .field(Invoice::customer)
                                    .field(Invoice::issued)
                                    .item(t!("ui.status"), "INV-2026-0042"),
                            )
                            .add(Row::new().compact().col(Badge::new(t!("ui.badge_paid")).tone(Tone::Success)).col(Badge::new(t!("ui.badge_overdue")).tone(Tone::Danger)).col(Badge::new(t!("ui.badge_draft"))))
                            .add(Progress::new(t!("ui.progress_import"), 64, 100))
                            .add(Progress::new(t!("ui.progress_quota"), 92, 100).tone(Tone::Warning)),
                    ),
            )
            .add(
                Row::new()
                    .col(
                        Card::new().title(t!("ui.card_history")).add(
                            Timeline::new()
                                .event(now.add_ms(-26 * hour), t!("ui.event_created"), None, Tone::Info)
                                .event(now.add_ms(-5 * hour), t!("ui.event_approved"), Some(t!("ui.event_approved_by")), Tone::Success)
                                .event(now.add_ms(-hour), t!("ui.event_shipped"), None, Tone::Neutral),
                        ),
                    )
                    .col(
                        Card::new()
                            .title(t!("ui.card_actions"))
                            .add(
                                ActionButton::new(t!("ui.archive"), archive_samples, 12)
                                    .confirm(t!("ui.archive_confirm"))
                                    .tone(Tone::Danger)
                                    .on_success(|n, ui| {
                                        if let Ok(n) = n {
                                            ui.toast(t!("ui.archived", n = n));
                                        }
                                    }),
                            )
                            .add(ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
                                ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::Navigate { screen: "product_list", params: serde_json::Value::Null }));
                            }))
                            .add(Skeleton::lines(3)),
                    )
                    .col(Card::new().add(EmptyState::new(t!("ui.empty_title")).text(t!("ui.empty_text")).opens(t!("ui.empty_button"), crate::stock::product_editor, crate::stock::ProductParams { id: None }))),
            ),
    )
}

/// A job application filled in three steps.
#[model]
pub struct Application {
    #[rule(required, max_len = 60)]
    pub full_name: String,
    #[rule(required, email)]
    pub email: String,
    #[rule(required, max_len = 40)]
    pub city: String,
    pub start: Option<Date>,
    pub remote: bool,
    #[rule(max_len = 400)]
    pub motivation: String,
}

#[action(public)]
pub async fn apply(application: Valid<Application>) -> Result<String> {
    Ok(application.full_name.clone())
}

#[screen(route = "/wizard", public)]
pub fn wizard() -> Screen {
    Screen::new(t!("ui.wizard")).body(
        Panel::new(t!("ui.panel_application")).add(
            Form::of(Application::default())
                .step(t!("ui.step_person"))
                .field(Application::full_name)
                .field(Application::email)
                .step(t!("ui.step_work"))
                .row(|r| r.field(Application::city).field(Application::start))
                .field(Application::remote)
                .control(Application::remote, Control::Switch)
                .step(t!("ui.step_motivation"))
                .field(Application::motivation)
                .submit(t!("ui.apply"), apply)
                .on_success(|_, ui| ui.toast(t!("ui.applied"))),
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
        .screen(invoice_form)
        .screen(widgets)
        .screen(wizard)
        .menu(|m| {
            m.group(t!("menu.ui"), Icon::Table)
                .item(t!("ui.layout"), layout)
                .item(t!("ui.editors"), editors)
                .item(t!("ui.controls"), controls)
                .item(t!("ui.invoice"), invoice_form)
                .item(t!("ui.widgets"), widgets)
                .item(t!("ui.wizard"), wizard)
        })
}
