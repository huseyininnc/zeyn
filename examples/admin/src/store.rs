//! Where the records live: in memory with sample data (the default), or in PostgreSQL when
//! `database.url` is set and the app is built with the `postgres` feature.

use crate::models::*;
use crate::roles::Role;
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
#[cfg(feature = "postgres")]
use zeyn::sql::Db;

/// The records of every table, for the store without a database.
#[derive(Default)]
pub struct Memory {
    customers: RwLock<Vec<Customer>>,
    products: RwLock<Vec<Product>>,
    orders: RwLock<Vec<Order>>,
    events: RwLock<Vec<OrderEvent>>,
    users: RwLock<Vec<User>>,
    settings: RwLock<Vec<Settings>>,
}

/// An entity the store keeps: its list in memory, its key and its row version.
pub trait Row: Entity + Clone + Send + Sync + 'static {
    fn rows(memory: &Memory) -> &RwLock<Vec<Self>>;
    fn key(&self) -> i64;
    fn set_key(&mut self, key: i64);
    fn version(&self) -> i64;
    fn set_version(&mut self, version: i64);
}

/// Implements [`Row`] for an entity with an `id` key and a `version` field.
macro_rules! row {
    ($entity:ty, $list:ident, $key:ident) => {
        impl Row for $entity {
            fn rows(memory: &Memory) -> &RwLock<Vec<Self>> {
                &memory.$list
            }
            fn key(&self) -> i64 {
                self.id.0
            }
            fn set_key(&mut self, key: i64) {
                self.id = $key(key);
            }
            fn version(&self) -> i64 {
                self.version
            }
            fn set_version(&mut self, version: i64) {
                self.version = version;
            }
        }
    };
}

row!(Customer, customers, CustomerId);
row!(Product, products, ProductId);
row!(Order, orders, OrderId);
row!(OrderEvent, events, OrderEventId);
row!(User, users, UserId);
row!(Settings, settings, SettingsId);

/// The app's data, registered as a service and taken by actions and screens.
#[derive(Clone)]
pub enum Store {
    Memory(Arc<Memory>),
    #[cfg(feature = "postgres")]
    Sql(Db),
}

impl Service for Store {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("store lock is poisoned".into())
}

impl Store {
    /// A store in memory with the same sample data the migrations write.
    pub fn sample(today: Date) -> Self {
        let memory = Memory::default();
        seed(&memory, today);
        Store::Memory(Arc::new(memory))
    }

    /// A PostgreSQL store. The migrations in `migrations/` create the tables and the sample data
    /// on the first start.
    #[cfg(feature = "postgres")]
    pub async fn connect(config: &zeyn::Config) -> Result<Self> {
        let db = Db::from_config(config).await?;
        db.migrate_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
            .await?;
        Ok(Store::Sql(db))
    }

    /// Filters, sorts, totals and pages rows: in SQL, or in memory.
    pub async fn find<E: Row>(&self, q: &DataQuery) -> Result<Page<E>> {
        match self {
            Store::Memory(m) => {
                let rows = E::rows(m)
                    .read()
                    .map_err(poisoned)?
                    .clone();
                Ok(q.apply(rows))
            }
            #[cfg(feature = "postgres")]
            Store::Sql(db) => db.find(q).await,
        }
    }

    /// One row by its key, or `NotFound`.
    pub async fn get<E: Row>(&self, key: i64) -> Result<E> {
        match self {
            Store::Memory(m) => E::rows(m)
                .read()
                .map_err(poisoned)?
                .iter()
                .find(|r| r.key() == key)
                .cloned()
                .ok_or(Error::NotFound),
            #[cfg(feature = "postgres")]
            Store::Sql(db) => db.get(key).await,
        }
    }

    /// Inserts a row with key 0 or updates one. An update fails with `Conflict` when the row
    /// version is not the stored one, so nobody overwrites a newer save.
    pub async fn save<E: Row>(&self, row: E) -> Result<E> {
        match self {
            Store::Memory(m) => {
                let mut rows = E::rows(m).write().map_err(poisoned)?;
                save_in_memory(&mut rows, row)
            }
            #[cfg(feature = "postgres")]
            Store::Sql(db) if row.key() == 0 => db.insert(&row).await,
            #[cfg(feature = "postgres")]
            Store::Sql(db) => db.update(&row).await,
        }
    }
}

fn save_in_memory<E: Row>(rows: &mut Vec<E>, mut row: E) -> Result<E> {
    if row.key() == 0 {
        let next = rows
            .iter()
            .map(Row::key)
            .max()
            .unwrap_or(0)
            + 1;
        row.set_key(next);
        row.set_version(1);
        rows.push(row.clone());
        return Ok(row);
    }
    let current = rows
        .iter_mut()
        .find(|r| r.key() == row.key())
        .ok_or(Error::NotFound)?;
    if current.version() != row.version() {
        return Err(Error::Conflict);
    }
    row.set_version(row.version() + 1);
    *current = row.clone();
    Ok(row)
}

/// Sample customers.
pub const CUSTOMERS: i64 = 500;

/// Sample products.
pub const PRODUCTS: i64 = 200;

/// Sample orders.
pub const ORDERS: i64 = 5000;

/// Sample users.
pub const USERS: i64 = 12;

const FIRST: [&str; 12] = [
    "Ada", "Grace", "Alan", "Linus", "Barbara", "Edsger", "Margaret", "Dennis", "Frances", "Ken",
    "Radia", "Niklaus",
];
const LAST: [&str; 10] = [
    "Lovelace", "Hopper", "Turing", "Torvalds", "Liskov", "Dijkstra", "Hamilton", "Ritchie",
    "Allen", "Thompson",
];
const CITIES: [&str; 8] = [
    "Istanbul", "Ankara", "Izmir", "Berlin", "London", "Paris", "Madrid", "Vienna",
];
const NAMES: [&str; 10] = [
    "Laptop",
    "Monitor",
    "Keyboard",
    "Licence",
    "Support plan",
    "Printer paper",
    "Toner",
    "Router",
    "Training day",
    "Cloud backup",
];

/// Sample customers, products, orders and users, built by the same rules as `0002_seed.sql`.
fn seed(memory: &Memory, today: Date) {
    let customers: Vec<Customer> = (1..=CUSTOMERS)
        .map(|i| customer(i, today))
        .collect();
    let orders: Vec<Order> = (1..=ORDERS)
        .map(|i| order(i, today, &customers))
        .collect();
    fill(&memory.products, (1..=PRODUCTS).map(product).collect());
    fill(&memory.orders, orders);
    fill(&memory.customers, customers);
    fill(&memory.users, (1..=USERS).map(user).collect());
    fill(
        &memory.settings,
        vec![Settings {
            id: SettingsId(1),
            company: "Acme Trading".into(),
            support_email: "support@acme.example".into(),
            ship_within_days: 3,
            low_stock: 10,
            notify_new_orders: true,
            weekly_report: false,
            version: 1,
        }],
    );
}

fn fill<E>(list: &RwLock<Vec<E>>, rows: Vec<E>) {
    if let Ok(mut l) = list.write() {
        *l = rows;
    }
}

fn person(i: i64) -> (String, String) {
    let first = FIRST[(i % 12) as usize];
    let last = LAST[(i % 10) as usize];
    (
        format!("{first} {last}"),
        format!(
            "{}.{}{i}@example.com",
            first.to_lowercase(),
            last.to_lowercase()
        ),
    )
}

fn customer(i: i64, today: Date) -> Customer {
    let (name, email) = person(i);
    Customer {
        id: CustomerId(i),
        name,
        email,
        city: CITIES[(i % 8) as usize].into(),
        segment: match i % 5 {
            0 => Segment::Partner,
            1 | 2 => Segment::Business,
            _ => Segment::Retail,
        },
        active: i % 11 != 0,
        since: today
            .add_days(-(i * 3 % 720))
            .unwrap_or(today),
        version: 1,
    }
}

fn product(i: i64) -> Product {
    Product {
        id: ProductId(i),
        sku: format!("SKU-{i:04}"),
        name: format!("{} {i}", NAMES[(i % 10) as usize]),
        category: match i % 10 {
            0..=2 => Category::Hardware,
            3 => Category::Software,
            4 | 8 => Category::Service,
            _ => Category::Supplies,
        },
        price: Money::tl((i % 37 + 1) * 2_490),
        stock: i * 13 % 90,
        active: i % 13 != 0,
        version: 1,
    }
}

fn order(i: i64, today: Date, customers: &[Customer]) -> Order {
    let customer = &customers[(i * 7 % CUSTOMERS) as usize];
    let age = i * 37 % 365;
    Order {
        id: OrderId(i),
        number: format!("SO-{i:05}"),
        customer_id: customer.id.0,
        customer_name: customer.name.clone(),
        placed: today.add_days(-age).unwrap_or(today),
        status: match (age, i % 17) {
            (_, 0) => OrderStatus::Cancelled,
            (0..=2, _) => OrderStatus::New,
            (3..=6, _) => OrderStatus::Processing,
            (7..=13, _) => OrderStatus::Shipped,
            _ => OrderStatus::Delivered,
        },
        items: i % 6 + 1,
        total: Money::tl(i * 7_919 % 450_000 + 9_900),
        notes: None,
        version: 1,
    }
}

fn user(i: i64) -> User {
    let (name, email) = person(i + 1000);
    User {
        id: UserId(i),
        name,
        email,
        role: match i {
            1..=2 => Role::Admin,
            3..=6 => Role::Manager,
            _ => Role::Viewer,
        },
        active: i != USERS,
        version: 1,
    }
}
