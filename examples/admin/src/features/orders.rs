//! Orders: a server paged grid with filters, chips, saved views and export, and a detail page
//! with tabs and the order's history. Saving an order notifies [`ORDERS_TOPIC`], so other open
//! pages refresh in place.

use crate::models::{Order, OrderEvent, OrderEventId, OrderStatus};
use crate::store::Store;
use crate::{ORDERS_TOPIC, can};
use zeyn::prelude::*;
use zeyn::realtime::Socket;
use zeyn::ui::Live;
use zeyn::{Filter, PageRequest, Sort};

/// Orders for the grids: filter, sort, total and page.
#[action(permission = "admin.orders.read")]
pub async fn search_orders(q: DataQuery, store: Store) -> Result<Page<Order>> {
    store.find(&q).await
}

/// Orders that still need work, shown as a count next to the menu item.
#[action(permission = "admin.orders.read")]
pub async fn open_orders(_: (), store: Store) -> Result<u64> {
    count_open(&store).await
}

/// Counts the orders whose status is one of [`OrderStatus::OPEN`].
pub async fn count_open(store: &Store) -> Result<u64> {
    let q = DataQuery {
        filters: vec![Filter {
            field: "status".into(),
            any_of: OrderStatus::OPEN
                .iter()
                .map(|s| format!("{s:?}"))
                .collect(),
            ..Filter::default()
        }],
        page: PageRequest::Offset {
            offset: 0,
            limit: 1,
        },
        with_total: true,
        ..DataQuery::default()
    };
    Ok(store
        .find::<Order>(&q)
        .await?
        .total
        .unwrap_or(0))
}

/// Saves the status and notes of an order. The other fields come from the stored order, so a
/// forged request cannot change them. Writes a history entry with the changed fields and tells
/// every open page that shows orders to refresh.
#[action(permission = "admin.orders.write", audit)]
pub async fn save_order(
    input: Valid<Order>,
    store: Store,
    socket: Socket,
    ctx: Ctx,
) -> Result<Order> {
    let input = input.into_inner();
    let before: Order = store.get(input.id.0).await?;
    let after = Order {
        status: input.status,
        notes: input.notes,
        version: input.version,
        ..before.clone()
    };
    let changes = zeyn::audit::diff(&before, &after);
    ctx.record_changes(&before, &after);
    let saved = store.save(after).await?;
    if !changes.is_empty() {
        store
            .save(OrderEvent {
                id: OrderEventId(0),
                order_id: saved.id.0,
                at: ctx.now(),
                user_name: ctx
                    .identity()
                    .user
                    .clone()
                    .unwrap_or_default(),
                kind: "saved".into(),
                detail: serde_json::to_string(&changes).unwrap_or_default(),
                version: 0,
            })
            .await?;
    }
    socket.notify(&ctx, ORDERS_TOPIC);
    Ok(saved)
}

/// The order list.
#[screen(route = "/orders", permission = "admin.orders.read")]
pub fn order_list() -> Screen {
    Screen::new(t!("orders.title"))
        .subtitle(t!("orders.subtitle"))
        .toolbar(|t| t.export(Export::Csv))
        .body(Live::new(ORDERS_TOPIC).add(order_grid()))
}

/// The grid of every order, newest first.
pub fn order_grid() -> DataGrid<Order> {
    DataGrid::of(search_orders)
        .mode(DataMode::Server)
        .paging(Paging::pages(25))
        .page_sizes(&[25, 50, 100])
        .sort_by_desc(Order::placed)
        .column(Order::number)
        .width(120)
        .filter()
        .link(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
        .column(Order::customer_name)
        .flex(1)
        .filter()
        .column(Order::placed)
        .width(140)
        .filter()
        .column(Order::status)
        .width(140)
        .filter()
        .badge(|s| s.tone())
        .column(Order::items)
        .width(90)
        .column(Order::total)
        .width(150)
        .filter()
        .sum()
        .search()
        .filter_builder()
        .filter_chips()
        .exportable()
        .on_open(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
}

/// Which order the detail page shows: `/orders/42`.
#[params]
pub struct OrderParams {
    pub id: i64,
}

/// One order: its details, an editor for users who may change it, and its history.
#[screen(route = "/orders/{id}", permission = "admin.orders.read")]
pub async fn order_page(p: OrderParams, store: Store, ctx: Ctx) -> Result<Screen> {
    let order: Order = store.get(p.id).await?;
    let details = DescriptionList::of(&order)
        .field(Order::number)
        .field(Order::customer_name)
        .field(Order::placed)
        .field(Order::status)
        .field(Order::items)
        .field(Order::total);
    let mut summary = Row::new().col(Card::new().add(details));
    if can(&ctx, "admin.orders.write") {
        summary = summary.col(Panel::new(t!("orders.edit")).add(order_form(order.clone())));
    }
    Ok(Screen::new(t!("orders.order_title", number = order.number))
        .body(
            Breadcrumb::new()
                .link(t!("orders.title"), order_list, ())
                .current(order.number.clone()),
        )
        .body(
            Tabs::new(t!("orders.tabs"))
                .tab(t!("orders.details"), summary)
                .tab(t!("orders.history"), history(&store, &order, &ctx).await?),
        ))
}

fn order_form(order: Order) -> Node {
    let id = order.id.0;
    Form::of(order)
        .field(Order::status)
        .field(Order::notes)
        .submit(t!("orders.save"), save_order)
        .on_success(move |saved, ui| {
            if let Ok(o) = saved {
                ui.toast(t!("orders.saved", number = o.number));
            }
            ui.navigate(order_page, OrderParams { id });
        })
        .into()
}

/// The order's history, newest first: every save with the fields it changed, then the order
/// being placed.
async fn history(store: &Store, order: &Order, ctx: &Ctx) -> Result<Timeline> {
    let id = order.id.0.to_string();
    let q = DataQuery {
        filters: vec![Filter::range("order_id", Some(&id), Some(&id))],
        sort: vec![Sort {
            field: "at".into(),
            desc: true,
        }],
        page: PageRequest::Offset {
            offset: 0,
            limit: 100,
        },
        ..DataQuery::default()
    };
    let mut timeline = Timeline::new();
    for e in store
        .find::<OrderEvent>(&q)
        .await?
        .items
    {
        let detail = t!(
            "orders.event_by",
            user = e.user_name,
            changes = changes_text(&e.detail, ctx)
        );
        timeline = timeline.event(e.at, t!("orders.event_saved"), Some(detail), Tone::Info);
    }
    Ok(timeline.event(
        order.placed,
        t!("orders.event_placed"),
        Some(t!(
            "orders.event_placed_detail",
            customer = order.customer_name
        )),
        Tone::Success,
    ))
}

/// The changes of a history entry in the user's language: `Status: New → Shipped`.
fn changes_text(json: &str, ctx: &Ctx) -> String {
    let changes: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    changes
        .iter()
        .map(|c| {
            let field = c["field"].as_str().unwrap_or_default();
            let shown = |v: &serde_json::Value| match (field, v.as_str()) {
                ("status", Some(s)) => ctx.t(&Text::key(format!("OrderStatus.{s}"))),
                (_, Some(s)) => s.to_string(),
                (_, None) => "-".into(),
            };
            format!(
                "{}: {} → {}",
                ctx.t(&Text::key(format!("Order.{field}"))),
                shown(&c["old"]),
                shown(&c["new"])
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Goes to the order list from anywhere: the command palette and `Alt+Shift+O`.
fn go_to_orders() -> Command {
    Command::navigate("admin.orders", t!("orders.go_to"), order_list, ())
        .shortcut("Alt+Shift+O")
        .icon(Icon::Receipt)
}

/// The orders feature, its menu and its command.
pub fn feature() -> Feature {
    Feature::new("orders")
        .guard(SignedIn)
        .screen(order_list)
        .screen(order_page)
        .api(search_orders)
        .api(save_order)
        .command(go_to_orders())
        .menu(|m| {
            m.group(t!("menu.sales"), Icon::Cart)
                .item(t!("orders.title"), order_list)
                .icon(Icon::Receipt)
                .count(open_orders)
                .count_listens(ORDERS_TOPIC)
        })
}
