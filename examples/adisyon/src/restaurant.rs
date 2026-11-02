//! The restaurant's state: menu, tables, order lines and sales.
//!
//! One lock guards everything, so sending an order to the kitchen checks and lowers the stock of
//! every item in one step. A real app does the same inside one database transaction.

use crate::features::kitchen::models::{LineStatus, OrderLine, OrderLineId};
use crate::features::menu::models::{Category, MenuItem, MenuItemId};
use crate::features::orders::models::{DiningTable, DiningTableId, PaymentMethod, Sale, SaleId, TableStatus};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;

#[derive(Default)]
struct State {
    menu: Vec<MenuItem>,
    tables: Vec<(DiningTableId, String, i64)>,
    lines: Vec<OrderLine>,
    sales: Vec<(Sale, Vec<(MenuItemId, i64)>)>,
    next_line: i64,
}

/// Shared restaurant state, injected into actions and screens.
#[derive(Clone, Default)]
pub struct Restaurant(Arc<RwLock<State>>);

impl Service for Restaurant {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("restaurant lock is poisoned".into())
}

/// A refusal the user can act on, shown as a form level message.
pub fn refuse(message: Text) -> Error {
    let mut errors = ValidationErrors::default();
    errors.add_general(message);
    Error::Validation(errors)
}

fn tl(lira: i64) -> Money {
    Money::tl(lira * 100)
}

impl Restaurant {
    /// A small restaurant: eight tables and a short menu, one dessert nearly sold out.
    pub fn seeded() -> Self {
        let menu = [
            ("Lentil soup", Category::Soups, 120, 40),
            ("Tripe soup", Category::Soups, 160, 15),
            ("Adana kebab", Category::Mains, 420, 25),
            ("Chicken shish", Category::Mains, 360, 30),
            ("Stuffed peppers", Category::Mains, 280, 12),
            ("Grilled meatballs", Category::Mains, 390, 20),
            ("Baklava", Category::Desserts, 210, 18),
            ("Rice pudding", Category::Desserts, 140, 2),
            ("Kunefe", Category::Desserts, 230, 10),
            ("Tea", Category::Drinks, 30, 500),
            ("Turkish coffee", Category::Drinks, 90, 200),
            ("Ayran", Category::Drinks, 60, 80),
        ];
        let state = State {
            menu: menu
                .iter()
                .enumerate()
                .map(|(i, (name, category, price, stock))| MenuItem {
                    id: MenuItemId(i as i64 + 1),
                    name: (*name).into(),
                    category: *category,
                    price: tl(*price),
                    stock: *stock,
                    active: true,
                    version: 1,
                })
                .collect(),
            tables: (1..=8)
                .map(|i| {
                    let name = if i <= 6 { format!("Table {i}") } else { format!("Garden {}", i - 6) };
                    (DiningTableId(i), name, if i % 3 == 0 { 6 } else { 4 })
                })
                .collect(),
            next_line: 1,
            ..State::default()
        };
        Restaurant(Arc::new(RwLock::new(state)))
    }

    pub fn menu(&self) -> Result<Vec<MenuItem>> {
        Ok(self.0.read().map_err(poisoned)?.menu.clone())
    }

    /// Saves a menu item; a stale row version fails with `Conflict`.
    pub fn save_item(&self, item: MenuItem) -> Result<MenuItem> {
        let mut state = self.0.write().map_err(poisoned)?;
        let current = state.menu.iter_mut().find(|m| m.id == item.id).ok_or(Error::NotFound)?;
        if current.version != item.version {
            return Err(Error::Conflict);
        }
        *current = MenuItem { version: item.version + 1, ..item };
        Ok(current.clone())
    }

    /// Tables with their status and the total of their open order.
    pub fn tables(&self) -> Result<Vec<DiningTable>> {
        let state = self.0.read().map_err(poisoned)?;
        Ok(state
            .tables
            .iter()
            .map(|(id, name, seats)| {
                let lines: Vec<&OrderLine> = state.lines.iter().filter(|l| l.table == *id).collect();
                let status = if lines.is_empty() {
                    TableStatus::Free
                } else if lines.iter().any(|l| l.status == LineStatus::New) {
                    TableStatus::Ordering
                } else if lines.iter().any(|l| l.status == LineStatus::Sent) {
                    TableStatus::Kitchen
                } else {
                    TableStatus::Served
                };
                DiningTable { id: *id, name: name.clone(), seats: *seats, status, open_total: total(&lines) }
            })
            .collect())
    }

    pub fn table(&self, id: DiningTableId) -> Result<DiningTable> {
        self.tables()?.into_iter().find(|t| t.id == id).ok_or(Error::NotFound)
    }

    /// Every open order line of every table.
    pub fn lines(&self) -> Result<Vec<OrderLine>> {
        Ok(self.0.read().map_err(poisoned)?.lines.clone())
    }

    /// Adds one portion of a menu item to a table's order. Refused when the item is off the menu
    /// or the portions already ordered would use up its stock.
    pub fn add(&self, table: DiningTableId, item: MenuItemId, now: DateTime) -> Result<OrderLine> {
        let mut state = self.0.write().map_err(poisoned)?;
        let table_name = state.tables.iter().find(|t| t.0 == table).map(|t| t.1.clone()).ok_or(Error::NotFound)?;
        let menu_item = state.menu.iter().find(|m| m.id == item).cloned().ok_or(Error::NotFound)?;
        let waiting: i64 =
            state.lines.iter().filter(|l| l.item_id == item && l.status == LineStatus::New).map(|l| l.quantity).sum();
        if !menu_item.active || menu_item.stock <= waiting {
            return Err(refuse(Text::key("orders.out_of_stock").arg("item", menu_item.name)));
        }
        if let Some(line) =
            state.lines.iter_mut().find(|l| l.table == table && l.item_id == item && l.status == LineStatus::New)
        {
            line.quantity += 1;
            line.amount = Money::tl(line.unit_price.minor * line.quantity);
            return Ok(line.clone());
        }
        let line = OrderLine {
            id: OrderLineId(state.next_line),
            table,
            table_name,
            item_id: item,
            item: menu_item.name,
            quantity: 1,
            unit_price: menu_item.price,
            amount: menu_item.price,
            status: LineStatus::New,
            ordered: now,
        };
        state.next_line += 1;
        state.lines.push(line.clone());
        Ok(line)
    }

    /// Removes a line that has not gone to the kitchen yet.
    pub fn remove(&self, id: OrderLineId) -> Result<()> {
        let mut state = self.0.write().map_err(poisoned)?;
        let line = state.lines.iter().find(|l| l.id == id).ok_or(Error::NotFound)?;
        if line.status != LineStatus::New {
            return Err(refuse(Text::key("orders.already_sent")));
        }
        state.lines.retain(|l| l.id != id);
        Ok(())
    }

    /// Sends the table's new lines to the kitchen and takes their portions out of stock, all or
    /// nothing. Returns how many portions were sent.
    pub fn send(&self, table: DiningTableId, now: DateTime) -> Result<i64> {
        let mut guard = self.0.write().map_err(poisoned)?;
        let state = &mut *guard;
        let new: Vec<(MenuItemId, i64)> = state
            .lines
            .iter()
            .filter(|l| l.table == table && l.status == LineStatus::New)
            .map(|l| (l.item_id, l.quantity))
            .collect();
        if new.is_empty() {
            return Err(refuse(Text::key("orders.nothing_to_send")));
        }
        for (item, quantity) in &new {
            let menu_item = state.menu.iter().find(|m| m.id == *item).ok_or(Error::NotFound)?;
            if menu_item.stock < *quantity {
                return Err(refuse(Text::key("orders.out_of_stock").arg("item", menu_item.name.clone())));
            }
        }
        for (item, quantity) in &new {
            if let Some(m) = state.menu.iter_mut().find(|m| m.id == *item) {
                m.stock -= quantity;
                m.version += 1;
            }
        }
        for line in state.lines.iter_mut().filter(|l| l.table == table && l.status == LineStatus::New) {
            line.status = LineStatus::Sent;
            line.ordered = now;
        }
        Ok(new.iter().map(|(_, q)| q).sum())
    }

    /// The kitchen marks a line as ready to serve.
    pub fn ready(&self, id: OrderLineId) -> Result<()> {
        let mut state = self.0.write().map_err(poisoned)?;
        let line = state.lines.iter_mut().find(|l| l.id == id).ok_or(Error::NotFound)?;
        if line.status != LineStatus::Sent {
            return Err(Error::Conflict);
        }
        line.status = LineStatus::Ready;
        Ok(())
    }

    /// Closes the table's bill. Every line must have gone to the kitchen, and cash must cover the
    /// total. Returns the change to give back. `locale` formats the amounts in messages.
    pub fn pay(
        &self,
        table: DiningTableId,
        method: PaymentMethod,
        received: Money,
        now: DateTime,
        locale: &str,
    ) -> Result<Money> {
        let mut state = self.0.write().map_err(poisoned)?;
        let table_name = state.tables.iter().find(|t| t.0 == table).map(|t| t.1.clone()).ok_or(Error::NotFound)?;
        let lines: Vec<&OrderLine> = state.lines.iter().filter(|l| l.table == table).collect();
        if lines.is_empty() {
            return Err(refuse(Text::key("pay.nothing_to_pay")));
        }
        if lines.iter().any(|l| l.status == LineStatus::New) {
            return Err(refuse(Text::key("pay.send_first")));
        }
        let due = total(&lines);
        let change = match method {
            PaymentMethod::Card => Money::tl(0),
            PaymentMethod::Cash if received.minor < due.minor => {
                return Err(refuse(Text::key("pay.not_enough").arg("total", due.format(locale))));
            }
            PaymentMethod::Cash => Money::tl(received.minor - due.minor),
        };
        let items = lines.iter().map(|l| l.quantity).sum();
        let sold: Vec<(MenuItemId, i64)> = lines.iter().map(|l| (l.item_id, l.amount.minor)).collect();
        let id = SaleId(state.sales.len() as i64 + 1);
        state.sales.push((Sale { id, table: table_name, at: now, method, items, total: due }, sold));
        state.lines.retain(|l| l.table != table);
        Ok(change)
    }

    pub fn sales(&self) -> Result<Vec<Sale>> {
        Ok(self.0.read().map_err(poisoned)?.sales.iter().map(|(s, _)| s.clone()).collect())
    }

    /// Revenue per menu category over every sale, in lira.
    pub fn revenue_by_category(&self) -> Result<Vec<(Category, f64)>> {
        let state = self.0.read().map_err(poisoned)?;
        Ok([Category::Soups, Category::Mains, Category::Desserts, Category::Drinks]
            .iter()
            .map(|c| {
                let minor: i64 = state
                    .sales
                    .iter()
                    .flat_map(|(_, items)| items.iter())
                    .filter(|(item, _)| state.menu.iter().any(|m| m.id == *item && m.category == *c))
                    .map(|(_, amount)| amount)
                    .sum();
                (*c, minor as f64 / 100.0)
            })
            .collect())
    }
}

fn total(lines: &[&OrderLine]) -> Money {
    Money::tl(lines.iter().map(|l| l.amount.minor).sum())
}
