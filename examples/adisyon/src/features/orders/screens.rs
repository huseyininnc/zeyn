use super::actions::{add_item, find_lines, find_tables, remove_line, send_to_kitchen, take_payment};
use super::models::{AddItem, DiningTable, Payment, PaymentMethod, TableParams, TableStatus};
use crate::features::kitchen::models::{LineStatus, OrderLine};
use crate::features::menu::models::Category;
use crate::restaurant::Restaurant;
use zeyn::prelude::*;

/// The floor: every table, its status and open total. A table's name opens its order.
#[screen(route = "/", permission = "pos.order")]
pub fn floor(_: (), restaurant: Restaurant, ctx: Ctx) -> Result<Screen> {
    let tables = restaurant.tables()?;
    let busy = tables.iter().filter(|t| t.status != TableStatus::Free).count();
    let open = Money::tl(tables.iter().map(|t| t.open_total.minor).sum());
    Ok(Screen::new(t!("floor.title")).body(
        Stack::new()
            .add(
                DescriptionList::new()
                    .columns(3)
                    .item(t!("floor.free"), (tables.len() - busy).to_string())
                    .item(t!("floor.busy"), busy.to_string())
                    .item(t!("floor.open_total"), open.format(ctx.locale())),
            )
            .add(
                DataGrid::of(find_tables)
                    .mode(DataMode::Server)
                    .on_open(|t| Effect::navigate(order, TableParams { table: t.id }))
                    .column(DiningTable::name)
                    .link(|t| Effect::navigate(order, TableParams { table: t.id }))
                    .column(DiningTable::seats)
                    .column(DiningTable::status)
                    .badge(|s| match s {
                        TableStatus::Free => Tone::Success,
                        TableStatus::Ordering => Tone::Warning,
                        TableStatus::Kitchen => Tone::Info,
                        TableStatus::Served => Tone::Brand,
                    })
                    .column(DiningTable::open_total)
                    .sum(),
            ),
    ))
}

/// One table's order: the menu as buttons by category on the left, the order on the right.
#[screen(route = "/order", permission = "pos.order")]
pub fn order(params: TableParams, restaurant: Restaurant, ctx: Ctx) -> Result<Screen> {
    let table = restaurant.table(params.table)?;
    let menu = restaurant.menu()?;
    let id = table.id;
    let mut categories = Tabs::new(t!("orders.menu_tabs"));
    for category in [Category::Soups, Category::Mains, Category::Desserts, Category::Drinks] {
        let mut buttons = Row::new();
        for item in menu.iter().filter(|m| m.category == category && m.active) {
            let label = format!("{} · {}", item.name, item.price.format(ctx.locale()));
            buttons = buttons.col(
                ActionButton::new(label, add_item, AddItem { table: id, item: item.id })
                    .on_success(move |_, ui| ui.navigate(order, TableParams { table: id })),
            );
        }
        categories = categories.tab(Text::key(format!("Category.{category:?}")), buttons);
    }
    let lines = DataGrid::of(find_lines)
        .mode(DataMode::Server)
        .scope(OrderLine::table, id)
        .row_style(|l: &OrderLine| (l.status == LineStatus::Ready).then_some(RowStyle::Success))
        .column(OrderLine::item)
        .column(OrderLine::quantity)
        .column(OrderLine::amount)
        .sum()
        .column(OrderLine::status)
        .badge(|s| match s {
            LineStatus::New => Tone::Warning,
            LineStatus::Sent => Tone::Info,
            LineStatus::Ready => Tone::Success,
        })
        .row_action(t!("orders.remove"), remove_line, |l| l.id);
    let send = ActionButton::new(t!("orders.send"), send_to_kitchen, id).on_success(move |sent, ui| {
        if let Ok(n) = sent {
            ui.toast(t!("orders.sent", n = n.to_string()));
        }
        ui.navigate(order, TableParams { table: id });
    });
    Ok(Screen::new(t!("orders.title", table = table.name.clone()))
        .toolbar(|t| {
            t.button(t!("floor.title"))
                .icon(Icon::Table)
                .opens(floor, ())
                .button(t!("pay.open"))
                .icon(Icon::Save)
                .opens(payment, TableParams { table: id })
        })
        .body(Split::new(categories, Stack::new().add(lines).add(send)).ratio(55)))
}

/// The payment window. Cash must cover the total; the change is shown after paying.
#[screen(permission = "pos.cashier")]
pub fn payment(params: TableParams, restaurant: Restaurant, ctx: Ctx) -> Result<Screen> {
    let table = restaurant.table(params.table)?;
    let due = table.open_total;
    Ok(Screen::window(t!("pay.title", table = table.name.clone())).body(
        Stack::new().add(DescriptionList::new().item(t!("pay.total"), due.format(ctx.locale()))).add(
            Form::of(Payment { table: table.id, method: PaymentMethod::Cash, received: due })
                .field(Payment::method)
                .control(Payment::method, Control::Segmented)
                .field(Payment::received)
                .submit(t!("pay.submit"), take_payment)
                .on_success(|change, ui| {
                    if let Ok(change) = change {
                        ui.toast(t!("pay.done", change = change.format("tr")));
                    }
                    ui.navigate(floor, ());
                }),
        ),
    ))
}
