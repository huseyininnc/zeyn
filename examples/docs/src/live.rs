//! Live examples: real components placed next to their code samples with `@live name`.
//!
//! Each example is built by the framework on every request like any other screen node, so grids
//! page, forms validate and pickers open exactly as they do in an app. The data is a fixed sample
//! built once.

use std::sync::LazyLock;
use zeyn::prelude::*;
use zeyn::ui::{LookupItem, LookupQuery, TreeItem, TreeQuery};

/// A product of the sample data.
#[entity(table = "samples")]
pub struct Sample {
    #[key]
    pub id: SampleId,
    #[rule(required, max_len = 20)]
    pub code: String,
    #[rule(required, max_len = 80)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    pub stock: i64,
    pub active: bool,
}

/// Unit of measure.
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}

static SAMPLES: LazyLock<Vec<Sample>> = LazyLock::new(|| {
    let names = [
        "Black tea",
        "Green tea",
        "Coffee beans",
        "Olive oil",
        "Honey",
        "Hazelnuts",
        "Dried apricots",
        "Cheese",
        "Bulgur",
        "Lentils",
    ];
    (1..=120)
        .map(|i| Sample {
            id: SampleId(i),
            code: format!("S-{i:04}"),
            name: format!("{} {}", names[(i % 10) as usize], 100 + i * 5),
            unit: match i % 3 {
                0 => Unit::Kg,
                1 => Unit::Piece,
                _ => Unit::Liter,
            },
            price: Money::tl(450 + (i * 7_919) % 250_000),
            stock: (i * 37) % 120,
            active: i % 11 != 0,
        })
        .collect()
});

/// Sample products: filter, sort and page.
#[action(public)]
pub async fn search_samples(q: DataQuery) -> Result<Page<Sample>> {
    Ok(q.apply(SAMPLES.clone()))
}

/// Sample products for lookups.
#[action(public)]
pub async fn find_samples(q: LookupQuery) -> Result<Page<LookupItem>> {
    Ok(q.apply(SAMPLES.iter().map(|s| LookupItem::new(s.id.0, s.name.clone()).hint(s.code.clone())).collect()))
}

/// Sample products grouped by unit.
#[action(public)]
pub async fn sample_tree(q: TreeQuery) -> Result<Vec<TreeItem>> {
    let units = [("piece", Unit::Piece, "Piece"), ("kg", Unit::Kg, "Kilogram"), ("liter", Unit::Liter, "Liter")];
    Ok(match q.parent.as_deref() {
        None => units
            .iter()
            .map(|(id, unit, label)| {
                TreeItem::branch(*id, *label).hint(SAMPLES.iter().filter(|s| s.unit == *unit).count().to_string())
            })
            .collect(),
        Some(parent) => {
            let Some((_, unit, _)) = units.iter().find(|(id, _, _)| *id == parent) else { return Err(Error::NotFound) };
            SAMPLES
                .iter()
                .filter(|s| s.unit == *unit)
                .take(12)
                .map(|s| TreeItem::leaf(s.id.0.to_string(), s.name.clone()).hint(s.code.clone()))
                .collect()
        }
    })
}

/// A contact form for the form example.
#[model]
pub struct Contact {
    #[rule(required, max_len = 60)]
    pub name: String,
    #[rule(required, email)]
    pub email: String,
    pub size: Size,
    pub delivery: Delivery,
    pub product: Option<SampleId>,
    pub newsletter: bool,
    #[rule(min = 1, max = 99)]
    pub quantity: i64,
    #[rule(max_len = 300)]
    pub note: Option<String>,
}

/// Package size.
#[choice]
pub enum Size {
    Small,
    Medium,
    Large,
}

/// How an order is delivered.
#[choice]
pub enum Delivery {
    Pickup,
    Courier,
    Cargo,
}

/// Accepts a valid contact and sends it back. Nothing is stored.
#[action(public)]
pub async fn send_contact(contact: Valid<Contact>) -> Result<String> {
    Ok(contact.name.clone())
}

/// A visit booked with the date and time pickers.
#[model]
pub struct Visit {
    #[rule(required)]
    pub day: Option<Date>,
    pub arrives: Time,
    pub slot: LocalDateTime,
    pub stay_from: Option<Date>,
    pub stay_to: Option<Date>,
}

#[action(public)]
pub async fn book_visit(visit: Valid<Visit>) -> Result<Visit> {
    Ok(visit.into_inner())
}

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

#[action(public)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}

#[action(public)]
pub async fn count_samples(_: ()) -> Result<i64> {
    Ok(SAMPLES.len() as i64)
}

fn grid() -> Node {
    DataGrid::of(search_samples)
        .paging(Paging::pages(8))
        .column(Sample::code)
        .width(110)
        .filter()
        .column(Sample::name)
        .flex(1)
        .filter()
        .column(Sample::unit)
        .width(120)
        .filter()
        .badge(|u| match u {
            Unit::Piece => Tone::Neutral,
            Unit::Kg => Tone::Info,
            Unit::Liter => Tone::Brand,
        })
        .column(Sample::price)
        .width(150)
        .avg()
        .column(Sample::stock)
        .width(150)
        .sum()
        .progress(120)
        .column(Sample::active)
        .width(100)
        .check_icon()
        .row_style(|s| match s.stock {
            0 => Some(RowStyle::Danger),
            1..=9 => Some(RowStyle::Warning),
            _ => None,
        })
        .into()
}

/// The live example called `name`, or `None` for an unknown name.
pub fn example(name: &str, ctx: &Ctx) -> Option<Node> {
    let months = || ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];
    Some(match name {
        "datagrid" => grid(),
        "form" => Form::of(Contact { quantity: 1, ..Contact::default() })
            .row(|r| r.field(Contact::name).field(Contact::email))
            .row(|r| r.field(Contact::quantity).field(Contact::size))
            .submit(t!("docs.live.send"), send_contact)
            .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
            .into(),
        "controls" => Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
            .lookup(Contact::product, find_samples)
            .row(|r| r.field(Contact::delivery).field(Contact::size))
            .control(Contact::delivery, Control::Radio)
            .control(Contact::size, Control::Segmented)
            .field(Contact::newsletter)
            .control(Contact::newsletter, Control::Switch)
            .field(Contact::note)
            .submit(t!("docs.live.send"), send_contact)
            .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
            .into(),
        "dates" => {
            let today = ctx.today();
            Form::of(Visit {
                day: Some(today),
                arrives: Time::new(9, 30, 0).unwrap_or_default(),
                slot: LocalDateTime::new(today, Time::new(14, 0, 0).unwrap_or_default()),
                ..Visit::default()
            })
            .row(|r| r.field(Visit::day).field(Visit::arrives))
            .field(Visit::slot)
            .range(Visit::stay_from, Visit::stay_to)
            .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
            .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
            .submit(t!("docs.live.book"), book_visit)
            .on_success(|_, ui| ui.toast(t!("docs.live.booked")))
            .into()
        }
        "lines" => Form::of(Invoice {
            customer: "Northwind".into(),
            lines: vec![
                Line { item: "Green tea".into(), quantity: 12, unit_price: Money::tl(8369) },
                Line { item: "Olive oil".into(), quantity: 4, unit_price: Money::tl(24207) },
            ],
        })
        .field(Invoice::customer)
        .lines(Invoice::lines, |l| {
            l.column(Line::item).column(Line::quantity).column(Line::unit_price).total(Line::quantity, Line::unit_price)
        })
        .submit(t!("docs.live.save"), save_invoice)
        .on_success(|_, ui| ui.toast(t!("docs.live.saved")))
        .into(),
        "layout" => Stack::new()
            .add(
                Row::new()
                    .col(Card::new().title("Orders"))
                    .col(Card::new().title("Stock"))
                    .col(Card::new().title("People")),
            )
            .add(
                Panel::new("Products").collapsible().add(
                    Tabs::new("Views")
                        .tab("Card", Card::new().title("A card inside a tab"))
                        .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
                ),
            )
            .into(),
        "display" => Stack::new()
            .add(Alert::info("Imports run in the background.").dismissible())
            .add(Alert::warning("Three products are low on stock.").title("Stock"))
            .add(
                Row::new()
                    .compact()
                    .col(Badge::new("Paid").tone(Tone::Success))
                    .col(Badge::new("Overdue").tone(Tone::Danger))
                    .col(Badge::new("Draft")),
            )
            .add(Progress::new("Import", 64, 100))
            .add(Timeline::new().event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info).event(
                ctx.now().add_ms(-3_600_000),
                "Approved",
                Some(Text::from("by the manager")),
                Tone::Success,
            ))
            .add(Skeleton::lines(2))
            .into(),
        "action" => ActionButton::new(t!("docs.live.count"), count_samples, ())
            .on_success(|n, ui| {
                if let Ok(n) = n {
                    ui.toast(t!("docs.live.counted", n = n));
                }
            })
            .into(),
        "tree" => Tree::of(sample_tree).label("Products by unit").into(),
        "chart" => Stack::new()
            .add(
                Row::new()
                    .col(
                        Kpi::new("Revenue", 184_250.0)
                            .delta(12.4)
                            .target(200_000.0)
                            .trend([140.0, 152.0, 160.0, 184.0]),
                    )
                    .col(Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()),
            )
            .add(
                Row::new()
                    .col(
                        Chart::bar("Sales")
                            .labels(months())
                            .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
                            .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]),
                    )
                    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0])),
            )
            .add(
                Row::new()
                    .col(
                        Chart::combo("Sales and margin")
                            .labels(months())
                            .series("Sales", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
                            .line_series("Margin %", [18.0, 14.0, 21.0, 22.0, 19.0, 24.0])
                            .right_axis(),
                    )
                    .col(Chart::gauge("Capacity").value(72.0).range(0.0, 100.0).target(85.0)),
            )
            .into(),
        "shell" => Stack::new()
            .gap(2)
            .add(
                Toolbox::new("Tools")
                    .tool("select", "Select", Some(Icon::Search))
                    .tool("draw", "Draw", Some(Icon::Edit))
                    .tool("text", "Text", None),
            )
            .add(StatusBar::new("Status").left("Ready").right("UTF-8"))
            .into(),
        "content" => Stack::new()
            .gap(2)
            .add(Heading::h3("Brewing green tea"))
            .add(MetaLine::new().part("12 May 2026").part("5 min read").part("Guides"))
            .add(Paragraph::new("Water just off the boil and two minutes of steeping keep the taste soft."))
            .add(Paragraph::new("Times are a starting point.").muted().small())
            .add(
                PrevNext::new()
                    .prev(crate::introduction, ())
                    .prev_title("Introduction")
                    .next(crate::cli, ())
                    .next_title("CLI and projects"),
            )
            .into(),
        "autogrid" => ["Orders", "Stock", "People", "Invoices", "Reports"]
            .into_iter()
            .fold(AutoGrid::new().min(160), |tiles, title| tiles.add(Card::new().title(title)))
            .into(),
        "inputs" => inputs(),
        "overlay" => overlay(),
        "menus" => menus(),
        "toasts" => toasts(),
        "multi" => Form::of(Reach { channels: vec![Channel::Email], ..Reach::default() })
            .field(Reach::channels)
            .lookup_columns(Reach::product, find_sample_rows, ["Code", "Name", "Unit"])
            .submit(t!("docs.live.save"), save_reach)
            .on_success(|_, ui| ui.toast(t!("docs.live.saved")))
            .into(),
        _ => return None,
    })
}

/// Terms for the decimal, percent, masked and password example.
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[label("IBAN")]
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
    #[rule(min_len = 10, max_len = 128)]
    pub password: String,
}

/// Accepts valid terms and sends nothing back. Nothing is stored.
#[action(public)]
pub async fn save_terms(terms: Valid<Terms>) -> Result<String> {
    Ok(terms.phone.clone())
}

/// The decimal, percent, masked and password inputs.
fn inputs() -> Node {
    Form::of(Terms {
        rate: Decimal::parse("1.0825").unwrap_or_default(),
        discount: Decimal::parse("12.5"),
        phone: "5321234567".into(),
        ..Terms::default()
    })
    .row(|r| r.field(Terms::rate).field(Terms::discount))
    .control(Terms::discount, Control::Percent)
    .row(|r| r.field(Terms::phone).field(Terms::iban))
    .field(Terms::password)
    .control(Terms::password, Control::PasswordMeter)
    .submit(t!("docs.live.save"), save_terms)
    .on_success(|_, ui| ui.toast(t!("docs.live.saved")))
    .into()
}

/// How a customer wants to be reached.
#[choice]
pub enum Channel {
    Email,
    Phone,
    #[label("Text message")]
    Sms,
    Post,
}

/// Contact preferences, for the multi select and the multi column lookup.
#[model]
pub struct Reach {
    #[rule(required)]
    pub channels: Vec<Channel>,
    pub product: Option<SampleId>,
}

#[action(public)]
pub async fn save_reach(reach: Valid<Reach>) -> Result<usize> {
    Ok(reach.channels.len())
}

/// Sample products with code, name and unit columns.
#[action(public)]
pub async fn find_sample_rows(q: LookupQuery) -> Result<Page<LookupItem>> {
    let unit = |u: Unit| match u {
        Unit::Piece => "Piece",
        Unit::Kg => "Kg",
        Unit::Liter => "Liter",
    };
    Ok(q.apply(
        SAMPLES
            .iter()
            .map(|s| {
                LookupItem::new(s.id.0, s.name.clone()).cells([s.code.clone(), s.name.clone(), unit(s.unit).into()])
            })
            .collect(),
    ))
}

/// Counts the sample products slowly, to show the busy overlay.
#[action(public)]
pub async fn slow_count(_: ()) -> Result<i64> {
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    Ok(SAMPLES.len() as i64)
}

fn overlay() -> Node {
    let sample = &SAMPLES[0];
    Row::new()
        .compact()
        .col(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")).on_success(
            |n, ui| {
                if let Ok(n) = n {
                    ui.toast(t!("docs.live.counted", n = n));
                }
            },
        ))
        .col(
            Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(
                DescriptionList::of(sample)
                    .field(Sample::code)
                    .field(Sample::unit)
                    .field(Sample::price)
                    .field(Sample::stock),
            ),
        )
        .col(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ()).on_success(|n, ui| {
            if let Ok(n) = n {
                ui.toast(t!("docs.live.counted", n = n));
            }
        })))
        .into()
}

/// Pretends to save, for the menu and toast examples.
#[action(public)]
pub async fn save_sample(_: ()) -> Result<()> {
    Ok(())
}

fn menus() -> Node {
    Row::new()
        .compact()
        .col(
            MenuButton::new(t!("docs.live.more"))
                .action_then(t!("docs.live.count"), count_samples, (), |n, ui| {
                    if let Ok(n) = n {
                        ui.toast(t!("docs.live.counted", n = n));
                    }
                })
                .separator()
                .action_then(t!("docs.live.archive"), save_sample, (), |_, ui| {
                    ui.notify(Toast::warning(t!("docs.live.archived")))
                })
                .confirm(t!("docs.live.archive_confirm")),
        )
        .col(MenuButton::split(t!("docs.live.save"), save_sample, ()).action_then(
            t!("docs.live.save_copy"),
            save_sample,
            (),
            |_, ui| ui.notify(Toast::success(t!("docs.live.saved"))),
        ))
        .into()
}

fn toasts() -> Node {
    Row::new()
        .compact()
        .col(ActionButton::new(t!("docs.live.toast_success"), save_sample, ()).on_success(|_, ui| {
            ui.notify(
                Toast::success(t!("docs.live.saved"))
                    .action(t!("docs.live.again"), Effect::Toast { message: t!("docs.live.sent") }),
            )
        }))
        .col(
            ActionButton::new(t!("docs.live.toast_warning"), save_sample, ())
                .on_success(|_, ui| ui.notify(Toast::warning(t!("docs.live.archived")))),
        )
        .col(
            ActionButton::new(t!("docs.live.toast_danger"), save_sample, ())
                .on_success(|_, ui| ui.notify(Toast::danger(t!("docs.live.denied")))),
        )
        .into()
}

/// Names of every live example, for checks and tests.
pub const EXAMPLES: &[&str] = &[
    "datagrid", "form", "controls", "dates", "lines", "layout", "display", "action", "tree", "chart", "shell",
    "inputs", "multi", "overlay", "menus", "toasts", "content", "autogrid",
];
