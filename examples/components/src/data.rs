//! Sample data the examples show: products, people and events, built once and kept in memory.
//! Nothing a visitor sends is stored.

use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;
use zeyn::ui::{LookupItem, LookupQuery, TreeItem, TreeQuery};

/// Most entries a store that visitors write keeps; the oldest are dropped first.
pub const MAX_VISITOR_ENTRIES: usize = 256;

/// What visitors of the public catalog wrote, such as cart quantities or ratings: at most
/// [`MAX_VISITOR_ENTRIES`], the oldest dropped first, so the memory it takes never grows with
/// traffic. Writes reach it through actions, which count against the app's write limit.
pub struct VisitorLog<T> {
    entries: Mutex<VecDeque<T>>,
}

impl<T: Clone> Default for VisitorLog<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> VisitorLog<T> {
    pub const fn new() -> Self {
        VisitorLog {
            entries: Mutex::new(VecDeque::new()),
        }
    }

    /// Keeps `entry`, dropping the oldest one when the log is full.
    pub fn push(&self, entry: T) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.len() == MAX_VISITOR_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// How many entries the log holds.
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map_or(0, |e| e.len())
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The newest entry, if any.
    pub fn last(&self) -> Option<T> {
        self.entries
            .lock()
            .ok()?
            .back()
            .cloned()
    }
}

/// Cart quantities visitors set: item and quantity.
pub static CART: VisitorLog<(String, i64)> = VisitorLog::new();

/// Ratings visitors sent, as text.
pub static RATINGS: VisitorLog<String> = VisitorLog::new();

/// A product of the sample shop.
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    #[rule(required, max_len = 80)]
    pub name: String,
    pub kind: Kind,
    #[rule(min = 0)]
    pub price: Money,
    pub stock: i64,
    pub active: bool,
    pub image: String,
}

/// What kind of product it is.
#[choice]
pub enum Kind {
    Book,
    Tool,
    Plant,
}

/// A person of the sample team.
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    pub city: String,
    pub role: String,
}

/// An event of the sample week.
#[entity(table = "events")]
pub struct Event {
    #[key]
    pub id: EventId,
    pub title: String,
    pub day: Option<Date>,
    pub starts: String,
}

/// A warehouse total of the sample shop: large whole numbers, a year and a decimal.
#[entity(table = "warehouses")]
pub struct Warehouse {
    #[key]
    pub id: WarehouseId,
    pub city: String,
    pub units: i64,
    pub opened: i64,
    #[rule(scale = 1)]
    pub tons: Decimal,
}

/// Sample warehouses.
#[action(public)]
pub async fn search_warehouses(q: DataQuery) -> Result<Page<Warehouse>> {
    let rows = [
        ("Istanbul", 1_284_310, 1998, 48_215),
        ("Izmir", 642_005, 2006, 23_904),
        ("Ankara", 98_760, 2014, 3_506),
        ("Bursa", 7_250, 2021, 412),
    ]
    .into_iter()
    .zip(1..)
    .map(|((city, units, opened, tons), id)| Warehouse {
        id: WarehouseId(id),
        city: city.into(),
        units,
        opened,
        tons: Decimal::new(tons, 1).unwrap_or_default(),
    })
    .collect();
    Ok(q.apply(rows))
}

/// A million computed products: only the rows of the asked page are built, from their number.
#[action(public)]
pub async fn million_products(q: DataQuery) -> Result<Page<Product>> {
    const TOTAL: u64 = 1_000_000;
    let span = q.span();
    let rows = (span.offset..span.end().min(TOTAL))
        .map(|n| {
            let base = &PRODUCTS[(n % PRODUCTS.len() as u64) as usize];
            Product {
                id: ProductId(n as i64 + 1),
                code: format!("M-{:07}", n + 1),
                ..base.clone()
            }
        })
        .collect();
    Ok(Page::window(rows, TOTAL, span))
}

/// Rows and columns of the computed query result.
pub const RESULT_ROWS: u64 = 10_000;
pub const RESULT_COLUMNS: usize = 30;

/// A query result whose columns are known only when it runs, as a SQL preview gets them: 10,000
/// rows of 30 columns of every kind, computed from their numbers for the asked window only.
#[action(public)]
pub async fn query_result(q: DataQuery) -> Result<DynamicPage> {
    const KINDS: [DynamicKind; 6] = [
        DynamicKind::Text,
        DynamicKind::Integer,
        DynamicKind::Decimal,
        DynamicKind::Date,
        DynamicKind::DateTime,
        DynamicKind::Bool,
    ];
    let columns = (0..RESULT_COLUMNS)
        .map(|c| {
            let kind = KINDS[c % KINDS.len()];
            DynamicColumn::new(format!("col_{c}"), format!("Column {}", c + 1), kind)
        })
        .collect::<Vec<_>>();
    let span = q.span();
    let rows = (span.offset..span.end().min(RESULT_ROWS))
        .map(|n| {
            let cells = columns
                .iter()
                .enumerate()
                .map(|(c, col)| {
                    let v = n * 7_919 + c as u64 * 104_729;
                    match col.kind {
                        DynamicKind::Integer => serde_json::json!(v % 10_000_000),
                        DynamicKind::Decimal => {
                            serde_json::json!(format!("{}.{:02}", v % 100_000, v % 100))
                        }
                        DynamicKind::Date => {
                            serde_json::json!(format!("2026-{:02}-{:02}", v % 12 + 1, v % 28 + 1))
                        }
                        DynamicKind::DateTime => serde_json::json!(format!(
                            "2026-{:02}-{:02}T{:02}:{:02}:00Z",
                            v % 12 + 1,
                            v % 28 + 1,
                            v % 24,
                            v % 60
                        )),
                        DynamicKind::Bool => serde_json::json!(v.is_multiple_of(2)),
                        _ => serde_json::json!(format!("Row {} item {}", n + 1, c + 1)),
                    }
                })
                .collect();
            serde_json::Value::Array(cells)
        })
        .collect();
    Ok(DynamicPage::new(columns, rows).total(RESULT_ROWS))
}

/// The address of sample picture `n` (1 to 12), served by the site as an SVG.
pub fn picture(n: i64) -> String {
    format!("/site/img/{}.svg", n.rem_euclid(12) + 1)
}

/// Sample picture `n`: a soft gradient with a circle, in a hue of its own.
pub fn svg(n: u32) -> String {
    let hue = (n * 47) % 360;
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 300" width="300" height="300"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="hsl({hue} 70% 72%)"/><stop offset="1" stop-color="hsl({} 60% 45%)"/></linearGradient></defs><rect width="300" height="300" fill="url(#g)"/><circle cx="{}" cy="{}" r="70" fill="hsl({hue} 80% 92% / 0.55)"/></svg>"##,
        (hue + 40) % 360,
        90 + (n * 37) % 120,
        90 + (n * 53) % 120,
    )
}

static PRODUCTS: LazyLock<Vec<Product>> = LazyLock::new(|| {
    let names = [
        "Field notes",
        "Garden trowel",
        "Fern",
        "City atlas",
        "Hand saw",
        "Olive tree",
        "Cookbook",
        "Tape measure",
        "Cactus",
        "Poetry",
        "Pliers",
        "Bamboo",
    ];
    (1..=60)
        .map(|i| Product {
            id: ProductId(i),
            code: format!("P-{i:03}"),
            name: format!("{} {}", names[(i as usize - 1) % names.len()], 100 + i),
            kind: match (i - 1) % 3 {
                0 => Kind::Book,
                1 => Kind::Tool,
                _ => Kind::Plant,
            },
            price: Money::tl(1_250 + (i * 7_919) % 60_000),
            stock: (i * 37) % 90,
            active: i % 9 != 0,
            image: picture(i),
        })
        .collect()
});

/// Every sample product.
pub fn products() -> &'static [Product] {
    &PRODUCTS
}

static PEOPLE: LazyLock<Vec<Person>> = LazyLock::new(|| {
    let rows = [
        ("Ada Lovelace", "London", "Analyst"),
        ("Alan Turing", "Manchester", "Researcher"),
        ("Grace Hopper", "New York", "Engineer"),
        ("İlknur Şahin", "İzmir", "Designer"),
        ("Hedy Lamarr", "Vienna", "Inventor"),
        ("Katherine Johnson", "Hampton", "Mathematician"),
        ("Ömer Çelik", "Ankara", "Developer"),
        ("Margaret Hamilton", "Boston", "Engineer"),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (name, city, role))| Person {
            id: PersonId(i as i64 + 1),
            name: name.to_string(),
            city: city.to_string(),
            role: role.to_string(),
        })
        .collect()
});

/// Every sample person.
pub fn people() -> &'static [Person] {
    &PEOPLE
}

/// Sample products: filter, sort and page.
#[action(public)]
pub async fn search_products(q: DataQuery) -> Result<Page<Product>> {
    Ok(q.apply(PRODUCTS.clone()))
}

/// How many items the large catalog has, on [`SHELVES`] shelves of equal size.
pub const ITEMS: i64 = 2000;

/// Shelves of the large catalog; a shelf fits one page of a card grid.
pub const SHELVES: i64 = 4;

/// An item of the large catalog, with a photo of its own.
#[entity(table = "items")]
pub struct Item {
    #[key]
    pub id: ItemId,
    pub name: String,
    pub price: Money,
    pub shelf: i64,
    pub color: String,
    pub brand: String,
    pub old_price: Money,
}

impl Item {
    /// The full photo of the item. Every 97th item has none, to show the fallback.
    pub fn photo(&self) -> String {
        match self.id.0 % 97 {
            0 => format!("/site/photo/missing-{}.svg", self.id.0),
            _ => format!("/site/photo/{}.svg", self.id.0),
        }
    }

    /// The small, low resolution photo shown first.
    pub fn low(&self) -> String {
        format!("/site/photo/{}-low.svg", self.id.0)
    }
}

/// The colors of the large catalog: value and name.
pub const COLORS: [(&str, &str); 6] = [
    ("black", "Black"),
    ("white", "White"),
    ("red", "Red"),
    ("blue", "Blue"),
    ("green", "Green"),
    ("sand", "Sand"),
];

/// The brands of the large catalog.
pub const BRANDS: [&str; 8] = [
    "Arda", "Birch", "Cove", "Dune", "Elm", "Fjord", "Grove", "Haven",
];

/// Every item of the large catalog.
pub fn items() -> &'static [Item] {
    &ITEM_ROWS
}

static ITEM_ROWS: LazyLock<Vec<Item>> = LazyLock::new(|| {
    (1..=ITEMS)
        .map(|i| Item {
            id: ItemId(i),
            name: format!("Item {i}"),
            price: Money::tl(990 + (i * 7_919) % 90_000),
            shelf: (i - 1) * SHELVES / ITEMS,
            color: COLORS[(i % COLORS.len() as i64) as usize]
                .0
                .to_string(),
            brand: BRANDS[((i / 7) % BRANDS.len() as i64) as usize].to_string(),
            old_price: match i % 5 {
                0 => Money::tl((990 + (i * 7_919) % 90_000) * 5 / 4),
                _ => Money::tl(0),
            },
        })
        .collect()
});

/// The items of the large catalog: filter, sort and page.
#[action(public)]
pub async fn search_items(q: DataQuery) -> Result<Page<Item>> {
    Ok(q.apply(ITEM_ROWS.clone()))
}

/// Photo `n` of the large catalog, full (a gradient with a circle) or low (one color).
pub fn photo_svg(n: u32, low: bool) -> String {
    let hue = (n * 47) % 360;
    match low {
        true => format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3 3" width="3" height="3"><rect width="3" height="3" fill="hsl({hue} 65% 62%)"/></svg>"#
        ),
        false => svg(n),
    }
}

/// Sample people: filter, sort and page.
#[action(public)]
pub async fn search_people(q: DataQuery) -> Result<Page<Person>> {
    Ok(q.apply(PEOPLE.clone()))
}

/// Sample people without a total, like an action that does not count its rows.
#[action(public)]
pub async fn people_uncounted(q: DataQuery) -> Result<Page<Person>> {
    let mut page = q.apply(PEOPLE.clone());
    page.total = None;
    Ok(page)
}

/// Events of the current week, a few each day.
#[action(public)]
pub async fn week_events(q: DataQuery, ctx: Ctx) -> Result<Page<Event>> {
    Ok(q.apply(week(ctx.today())))
}

/// The sample events of the week of `today`, Monday first.
pub fn week(today: Date) -> Vec<Event> {
    let monday = today
        .add_days(1 - i64::from(today.weekday()))
        .unwrap_or(today);
    let plan = [
        (0, "Team stand up", "09:00"),
        (0, "Design review", "14:00"),
        (1, "Workshop", "10:30"),
        (2, "Release", "16:00"),
        (3, "Planning", "09:30"),
        (3, "Lunch talk", "12:30"),
        (4, "Demo day", "15:00"),
        (5, "Open house", "11:00"),
    ];
    plan.iter()
        .enumerate()
        .map(|(i, (offset, title, starts))| Event {
            id: EventId(i as i64 + 1),
            title: title.to_string(),
            day: monday.add_days(*offset),
            starts: starts.to_string(),
        })
        .collect()
}

/// Sample products for lookups.
#[action(public)]
pub async fn find_products(q: LookupQuery) -> Result<Page<LookupItem>> {
    Ok(q.apply(
        PRODUCTS
            .iter()
            .map(|p| LookupItem::new(p.id.0, p.name.clone()).hint(p.code.clone()))
            .collect(),
    ))
}

/// Sample products grouped by kind.
#[action(public)]
pub async fn product_tree(q: TreeQuery) -> Result<Vec<TreeItem>> {
    let kinds = [
        ("book", Kind::Book, "Books"),
        ("tool", Kind::Tool, "Tools"),
        ("plant", Kind::Plant, "Plants"),
    ];
    Ok(match q.parent.as_deref() {
        None => kinds
            .iter()
            .map(|(id, kind, label)| {
                TreeItem::branch(*id, *label).hint(
                    PRODUCTS
                        .iter()
                        .filter(|p| p.kind == *kind)
                        .count()
                        .to_string(),
                )
            })
            .collect(),
        Some(parent) => {
            let Some((_, kind, _)) = kinds
                .iter()
                .find(|(id, _, _)| *id == parent)
            else {
                return Err(Error::NotFound);
            };
            PRODUCTS
                .iter()
                .filter(|p| p.kind == *kind)
                .take(8)
                .map(|p| TreeItem::leaf(p.id.0.to_string(), p.name.clone()).hint(p.code.clone()))
                .collect()
        }
    })
}

/// Counts the sample products.
#[action(public)]
pub async fn count_products(_: ()) -> Result<i64> {
    Ok(PRODUCTS.len() as i64)
}

/// Counts the sample products slowly, to show a busy overlay.
#[action(public)]
pub async fn slow_count(_: ()) -> Result<i64> {
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    Ok(PRODUCTS.len() as i64)
}

/// Pretends to save. Nothing is stored.
#[action(public)]
pub async fn pretend_save(_: ()) -> Result<()> {
    Ok(())
}

/// Pretends to save a product after the form rules checked it. Nothing is stored.
#[action(public)]
pub async fn pretend_save_product(_: Valid<Product>) -> Result<()> {
    Ok(())
}

/// Pretends to delete one product. Nothing changes.
#[action(public)]
pub async fn pretend_delete(_: ProductId) -> Result<()> {
    Ok(())
}

/// Pretends to delete the selected products. Nothing changes.
#[action(public)]
pub async fn pretend_delete_all(_: Vec<ProductId>) -> Result<()> {
    Ok(())
}
