//! Component gallery: every field kind and every grid paging style in one place.

use crate::stock::{Product, search_products};
use zeyn::prelude::*;
use zeyn::{FieldKind, FieldType, Value};

/// A phone number: a field type defined in the app, shown with a client side editor named `phone`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Phone(pub String);

impl FieldType for Phone {
    const KIND: FieldKind = FieldKind::Custom { editor: "phone" };

    fn to_value(&self) -> Value {
        Value::Text(self.0.clone())
    }
}

/// One field of every kind.
#[model]
pub struct Sample {
    #[rule(required, min_len = 3, max_len = 60)]
    pub full_name: String,
    #[rule(required, email)]
    pub email: String,
    #[rule(min = 1, max = 120)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub budget: Money,
    pub approved: bool,
    pub priority: Priority,
    pub start_date: Date,
    pub reminder: Time,
    pub created_at: DateTime,
    pub meeting: LocalDateTime,
    pub phone: Phone,
    pub due: Option<Date>,
    #[rule(max_len = 300)]
    pub comment: Option<String>,
    #[readonly]
    pub reference: String,
}

/// Priority.
#[choice]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[action(public)]
pub async fn check_sample(sample: Valid<Sample>) -> Result<String> {
    Ok(sample.full_name.clone())
}

#[screen(route = "/fields", public)]
pub fn fields(_: (), ctx: Ctx) -> Screen {
    let today = ctx.today();
    let sample = Sample {
        quantity: 3,
        budget: Money::tl(1_250_000),
        priority: Priority::Normal,
        start_date: today,
        reminder: Time::new(9, 30, 0).unwrap_or_default(),
        created_at: ctx.now(),
        meeting: LocalDateTime::new(today.add_days(2).unwrap_or(today), Time::new(14, 0, 0).unwrap_or_default()),
        phone: Phone("+90 212 555 01 02".into()),
        reference: "REF-2026-0042".into(),
        ..Sample::default()
    };
    Screen::new(t!("gallery.fields")).body(
        Form::of(sample)
            .row(|r| r.field(Sample::full_name).field(Sample::email))
            .row(|r| r.field(Sample::quantity).field(Sample::budget).field(Sample::priority))
            .row(|r| r.field(Sample::start_date).field(Sample::reminder).field(Sample::due))
            .row(|r| r.field(Sample::created_at).field(Sample::meeting))
            .row(|r| r.field(Sample::phone).field(Sample::reference).field(Sample::approved))
            .field(Sample::comment)
            .submit(t!("gallery.check"), check_sample)
            .on_success(|name, ui| {
                if let Ok(n) = name {
                    ui.toast(t!("gallery.valid", name = n));
                }
            }),
    )
}

#[screen(route = "/infinite", permission = "stock.product.read")]
pub fn infinite() -> Screen {
    Screen::new(t!("gallery.infinite")).body(
        Grid::of(search_products)
            .mode(GridMode::Server)
            .paging(Paging::infinite(30))
            .column(Product::code)
            .width(120)
            .filter()
            .column(Product::name)
            .flex(1)
            .filter()
            .column(Product::price)
            .width(140)
            .column(Product::stock)
            .width(110),
    )
}

#[action(permission = "stock.product.read")]
pub async fn top_products(q: GridQuery, store: crate::stock::ProductStore) -> Result<Page<Product>> {
    let mut all = store.all()?;
    all.sort_by(|a, b| b.price.minor.cmp(&a.price.minor));
    all.truncate(8);
    Ok(q.apply(all))
}

#[screen(route = "/compact", permission = "stock.product.read")]
pub fn compact() -> Screen {
    Screen::new(t!("gallery.compact"))
        .body(Grid::of(top_products).paging(Paging::none()).column(Product::name).flex(1).column(Product::price).width(160))
        .body(Grid::auto(search_products).mode(GridMode::Server).paging(Paging::pages(10)))
}

pub fn feature() -> Feature {
    Feature::new("gallery")
        .route_prefix("/gallery")
        .guard(SignedIn)
        .screen(fields)
        .screen(infinite)
        .screen(compact)
        .screen(crate::wide::wide)
        .menu(|m| {
            m.group(t!("menu.gallery"), Icon::Table)
                .item(t!("gallery.fields"), fields)
                .item(t!("gallery.infinite"), infinite)
                .item(t!("gallery.compact"), compact)
                .item(t!("gallery.wide"), crate::wide::wide)
        })
}
