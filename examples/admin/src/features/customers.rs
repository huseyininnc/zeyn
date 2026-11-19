//! Customers: a searchable list, an editor window and a detail page with the customer's orders.

use super::orders::{OrderParams, order_page, search_orders};
use crate::models::{Customer, CustomerId, Order};
use crate::store::Store;
use crate::{ORDERS_TOPIC, can};
use zeyn::prelude::*;
use zeyn::{Aggregate, AggregateRequest, Filter, PageRequest};

/// Customers for the list: filter, sort and page.
#[action(permission = "admin.customers.read")]
pub async fn search_customers(q: DataQuery, store: Store) -> Result<Page<Customer>> {
    store.find(&q).await
}

/// Creates a customer (id 0) or saves one. The date a customer joined is set by the store.
#[action(permission = "admin.customers.write", audit)]
pub async fn save_customer(input: Valid<Customer>, store: Store, ctx: Ctx) -> Result<Customer> {
    let mut customer = input.into_inner();
    if customer.id.0 == 0 {
        customer.since = ctx.today();
    } else {
        let before: Customer = store.get(customer.id.0).await?;
        customer.since = before.since;
        ctx.record_changes(&before, &customer);
    }
    store.save(customer).await
}

/// The customer list.
#[screen(route = "/customers", permission = "admin.customers.read")]
pub fn customer_list(_: (), ctx: Ctx) -> Screen {
    let mut screen = Screen::new(t!("customers.title"));
    if can(&ctx, "admin.customers.write") {
        screen = screen.toolbar(|t| {
            t.button(t!("customers.new"))
                .icon(Icon::UserPlus)
                .shortcut("Alt+N")
                .opens(customer_form, CustomerFormParams { id: None })
                .export(Export::Csv)
        });
    }
    screen.body(
        DataGrid::of(search_customers)
            .mode(DataMode::Server)
            .paging(Paging::pages(25))
            .column(Customer::name)
            .flex(1)
            .filter()
            .link(|c| Effect::navigate(customer_page, CustomerParams { id: c.id.0 }))
            .column(Customer::email)
            .flex(1)
            .filter()
            .column(Customer::city)
            .width(140)
            .filter()
            .column(Customer::segment)
            .width(140)
            .filter()
            .status(|s| match s {
                crate::models::Segment::Partner => Tone::Brand,
                crate::models::Segment::Business => Tone::Info,
                crate::models::Segment::Retail => Tone::Neutral,
            })
            .column(Customer::since)
            .width(140)
            .filter()
            .column(Customer::active)
            .width(100)
            .filter()
            .check_icon()
            .search()
            .filter_chips()
            .exportable()
            .on_open(|c| Effect::navigate(customer_page, CustomerParams { id: c.id.0 })),
    )
}

/// Which customer the detail page shows: `/customers/7`.
#[params]
pub struct CustomerParams {
    pub id: i64,
}

/// One customer: key figures, the profile and the orders.
#[screen(route = "/customers/{id}", permission = "admin.customers.read")]
pub async fn customer_page(p: CustomerParams, store: Store, ctx: Ctx) -> Result<Screen> {
    let customer: Customer = store.get(p.id).await?;
    let q = DataQuery {
        filters: vec![Filter::range(
            "customer_id",
            Some(&p.id.to_string()),
            Some(&p.id.to_string()),
        )],
        aggregates: vec![AggregateRequest {
            field: "total".into(),
            op: Aggregate::Sum,
        }],
        page: PageRequest::Offset {
            offset: 0,
            limit: 1,
        },
        with_total: true,
        ..DataQuery::default()
    };
    let orders = store.find::<Order>(&q).await?;
    let spent = match orders.totals.first().map(|t| &t.value) {
        Some(zeyn::Value::Money(m)) => *m,
        _ => Money::tl(0),
    };
    let mut screen = Screen::new(customer.name.clone());
    if can(&ctx, "admin.customers.write") {
        screen = screen.toolbar(|t| {
            t.button(t!("customers.edit"))
                .icon(Icon::Edit)
                .shortcut("Alt+E")
                .opens(
                    customer_form,
                    CustomerFormParams {
                        id: Some(customer.id),
                    },
                )
        });
    }
    Ok(screen
        .body(
            Breadcrumb::new()
                .link(t!("customers.title"), customer_list, ())
                .current(customer.name.clone()),
        )
        .body(
            Row::new()
                .col(Kpi::new(
                    t!("customers.orders"),
                    orders.total.unwrap_or(0) as f64,
                ))
                .col(Kpi::money(t!("customers.spent"), spent)),
        )
        .body(
            Tabs::new(t!("customers.tabs"))
                .tab(
                    t!("customers.profile"),
                    Card::new().add(
                        DescriptionList::of(&customer)
                            .field(Customer::name)
                            .field(Customer::email)
                            .field(Customer::city)
                            .field(Customer::segment)
                            .field(Customer::since)
                            .field(Customer::active),
                    ),
                )
                .tab(
                    t!("customers.orders"),
                    DataGrid::of(search_orders)
                        .scope(Order::customer_id, p.id)
                        .paging(Paging::pages(10))
                        .sort_by_desc(Order::placed)
                        .column(Order::number)
                        .width(120)
                        .link(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
                        .column(Order::placed)
                        .width(140)
                        .column(Order::status)
                        .flex(1)
                        .badge(|s| s.tone())
                        .column(Order::total)
                        .width(150)
                        .sum()
                        .on_open(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
                        .listens(ORDERS_TOPIC),
                ),
        ))
}

/// Which customer the editor changes; `None` creates one.
#[params]
pub struct CustomerFormParams {
    pub id: Option<CustomerId>,
}

/// The customer editor window.
#[screen(permission = "admin.customers.write")]
pub async fn customer_form(p: CustomerFormParams, store: Store) -> Result<Screen> {
    let customer = match p.id {
        Some(id) => store.get(id.0).await?,
        None => Customer {
            active: true,
            ..Customer::default()
        },
    };
    Ok(Screen::window(t!("customers.customer"))
        .width(640)
        .body(
            Form::of(customer)
                .row(|r| {
                    r.field(Customer::name)
                        .field(Customer::email)
                })
                .row(|r| {
                    r.field(Customer::city)
                        .field(Customer::segment)
                })
                .field(Customer::active)
                .submit(t!("customers.save"), save_customer)
                .on_success(|saved, ui| {
                    if let Ok(c) = saved {
                        ui.toast(t!("customers.saved", name = c.name));
                    }
                    ui.close_and_refresh();
                }),
        ))
}

/// The customers feature, its menu and its command.
pub fn feature() -> Feature {
    Feature::new("customers")
        .guard(SignedIn)
        .screen(customer_list)
        .screen(customer_page)
        .screen(customer_form)
        .api(search_customers)
        .api(save_customer)
        .command(
            Command::open(
                "admin.new_customer",
                t!("customers.new"),
                customer_form,
                CustomerFormParams { id: None },
            )
            .shortcut("Alt+Shift+C")
            .icon(Icon::UserPlus),
        )
        .menu(|m| {
            m.group(t!("menu.sales"), Icon::Cart)
                .item(t!("customers.title"), customer_list)
                .icon(Icon::Users)
        })
}
