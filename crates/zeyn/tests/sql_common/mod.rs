//! Data layer tests shared by every database: results of `Db::find` are compared with
//! `DataQuery::apply` over the same rows, so SQL and memory data behave the same, then saves,
//! conflicts, unique errors, transactions and streams are checked.

use zeyn::prelude::*;
use zeyn::sql::{Arg, Db, PoolLimits};

/// Hand written statements that differ between databases.
pub struct Raw {
    /// Drops the test tables (and sequences); must not fail when they are missing.
    pub drop: &'static [&'static str],
    /// `UPDATE zeyn_test_items SET qty = <p1> WHERE id = <p2>`.
    pub set_qty: &'static str,
    /// Fills `zeyn_test_stream` with ids 1 to `STREAM_ROWS`.
    pub fill_stream: &'static str,
}

/// Rows the stream test reads.
pub const STREAM_ROWS: u64 = 100_000;
use zeyn::{Aggregate, AggregateRequest, Cursor, Filter, PageRequest, Sort, Value};

#[choice]
pub enum Kind {
    Drink,
    Food,
    Tool,
}

#[entity(table = "zeyn_test_items")]
pub struct Item {
    #[key]
    pub id: ItemId,
    #[rule(required, max_len = 40, unique)]
    pub code: String,
    pub name: String,
    pub note: Option<String>,
    pub kind: Kind,
    pub price: Money,
    pub qty: i64,
    pub small: i32,
    pub active: bool,
    pub day: Date,
    pub at: Option<DateTime>,
    pub clock: Time,
    pub local: LocalDateTime,
    #[readonly]
    pub stock: i64,
    #[version]
    pub version: i64,
}

#[entity(table = "zeyn_test_stream")]
pub struct Line {
    #[key]
    pub id: LineId,
    pub text: String,
}

/// Database URLs from `var`, separated by spaces. Empty means skip, and the test says so.
pub fn urls(var: &str) -> Vec<String> {
    let urls: Vec<String> = std::env::var(var).unwrap_or_default().split_whitespace().map(String::from).collect();
    if urls.is_empty() {
        eprintln!("skipped: set {var} to a database URL to run these tests");
    }
    urls
}

fn item(i: i64) -> Item {
    let names = ["Black tea", "Green tea", "Coffee", "Olive oil", "Honey", "Hazelnuts"];
    let day = Date::new(2026, 1 + (i % 12) as u8, 1 + (i % 28) as u8).unwrap();
    Item {
        id: ItemId(0),
        code: format!("P-{i:04}"),
        name: format!("{} {}", names[(i % 6) as usize], i % 40),
        note: (i % 3 != 0).then(|| format!("Note {}", i % 5)),
        kind: [Kind::Drink, Kind::Food, Kind::Tool][(i % 3) as usize],
        price: Money::tl(i * 1_275 % 9_000),
        qty: i % 17 - 5,
        small: (i % 9) as i32,
        active: i % 4 != 0,
        day,
        at: (i % 5 != 0).then(|| DateTime::from_unix_ms(1_767_225_600_000 + i * 3_600_123)),
        clock: Time::new((i % 24) as u8, (i % 60) as u8, 0).unwrap(),
        local: LocalDateTime::new(day, Time::new(9, (i % 60) as u8, 30).unwrap()),
        stock: 0,
        version: 0,
    }
}

async fn fresh(url: &str, raw: &Raw, limits: PoolLimits) -> Db {
    let db = Db::connect(url, limits).await.unwrap();
    for statement in raw.drop {
        db.execute(statement, vec![]).await.unwrap();
    }
    db.create_table::<Item>().await.unwrap();
    db.create_table::<Line>().await.unwrap();
    db
}

fn ids(items: &[Item]) -> Vec<i64> {
    items.iter().map(|i| i.id.0).collect()
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Decimal(x), Value::Decimal(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

async fn same(db: &Db, all: &[Item], q: DataQuery) {
    let mem = q.clone().apply(all.to_vec());
    let sql: Page<Item> = db.find(&q).await.unwrap();
    assert_eq!(ids(&sql.items), ids(&mem.items), "rows of {q:?}");
    if q.with_total {
        assert_eq!(sql.total, mem.total, "total of {q:?}");
    }
    assert_eq!(sql.totals.len(), mem.totals.len(), "totals of {q:?}: {:?} vs {:?}", sql.totals, mem.totals);
    for (a, b) in sql.totals.iter().zip(&mem.totals) {
        assert!(a.field == b.field && a.op == b.op && close(&a.value, &b.value), "{a:?} vs {b:?} in {q:?}");
    }
    assert_eq!(sql.groups.len(), mem.groups.len(), "groups of {q:?}");
    for (a, b) in sql.groups.iter().zip(&mem.groups) {
        assert_eq!((&a.value, a.count), (&b.value, b.count), "group of {q:?}");
        assert_eq!(a.totals.len(), b.totals.len());
        for (x, y) in a.totals.iter().zip(&b.totals) {
            assert!(close(&x.value, &y.value), "{x:?} vs {y:?}");
        }
    }
    assert_eq!(sql.next.is_some(), mem.next.is_some(), "more rows after {q:?}");
}

fn query(filters: Vec<Filter>, sort: Vec<(&str, bool)>, offset: u64, limit: u32) -> DataQuery {
    DataQuery {
        page: PageRequest::Offset { offset, limit },
        sort: sort.into_iter().map(|(f, desc)| Sort { field: f.into(), desc }).collect(),
        filters,
        with_total: true,
        ..DataQuery::default()
    }
}

/// Lists, totals, groups, keyset walks, saves, conflicts, unique errors and transactions.
pub async fn data_layer(url: &str, raw: &Raw) {
    let db = fresh(url, raw, PoolLimits { max_connections: 3, ..PoolLimits::default() }).await;

    let mut tx = db.begin().await.unwrap();
    for i in 1..=300 {
        tx.insert(&item(i)).await.unwrap();
    }
    tx.execute("UPDATE zeyn_test_items SET stock = MOD(id, 7)", vec![]).await.unwrap();
    tx.commit().await.unwrap();

    let mut all = Vec::new();
    let mut stream = db.stream::<Item>(&DataQuery::default());
    while let Some(row) = stream.next().await {
        all.push(row.unwrap());
    }
    assert_eq!(all.len(), 300);
    assert_eq!(all[0].code, "P-0001");
    assert_eq!(all[0].version, 1);
    assert!(
        all[3] == Item { id: ItemId(4), stock: 4, version: 1, ..item(4) },
        "every type reads back as written: {:?}",
        all[3]
    );

    same(&db, &all, query(vec![], vec![], 0, 25)).await;
    same(&db, &all, query(vec![], vec![("name", false)], 50, 25)).await;
    same(&db, &all, query(vec![], vec![("note", true), ("price", false)], 0, 60)).await;
    same(&db, &all, query(vec![], vec![("at", false)], 280, 30)).await;
    same(&db, &all, query(vec![Filter::contains("name", "TEA")], vec![("qty", true)], 0, 500)).await;
    same(
        &db,
        &all,
        query(vec![Filter { starts_with: Some("gre".into()), ..Filter::contains("name", "") }], vec![], 0, 50),
    )
    .await;
    same(
        &db,
        &all,
        query(vec![Filter { equals: Some("p-0042".into()), ..Filter::contains("code", "") }], vec![], 0, 50),
    )
    .await;
    same(&db, &all, query(vec![Filter::range("price", Some("10"), Some("45.5"))], vec![], 0, 500)).await;
    same(&db, &all, query(vec![Filter::range("qty", Some("-2.5"), Some("3.5"))], vec![("small", true)], 0, 500)).await;
    same(
        &db,
        &all,
        query(vec![Filter::range("day", Some("2026-03-01"), Some("2026-05-15"))], vec![("day", false)], 0, 500),
    )
    .await;
    same(&db, &all, query(vec![Filter::range("at", Some("2026-01-02"), Some("2026-01-05"))], vec![], 0, 500)).await;
    same(&db, &all, query(vec![Filter::range("clock", Some("10:00"), Some("14:30"))], vec![("clock", true)], 0, 500))
        .await;
    same(&db, &all, query(vec![Filter::range("local", Some("2026-02-01"), None)], vec![("local", true)], 0, 500)).await;
    same(&db, &all, query(vec![Filter::any_of("kind", ["Food", "Tool", "Nope"])], vec![("kind", false)], 0, 500)).await;
    same(&db, &all, query(vec![Filter::is("active", false), Filter::blank("note", true)], vec![], 0, 500)).await;
    same(
        &db,
        &all,
        query(vec![Filter::blank("at", false), Filter::any_of("id", ["3", "30", "300", "x"])], vec![], 0, 50),
    )
    .await;
    same(&db, &all, query(vec![Filter::contains("day", "2026-03")], vec![], 0, 500)).await;
    same(&db, &all, query(vec![Filter::contains("code", "%"), Filter::contains("code", "_")], vec![], 0, 500)).await;

    let totals = |field: &str, ops: &[Aggregate]| {
        ops.iter().map(|op| AggregateRequest { field: field.into(), op: *op }).collect::<Vec<_>>()
    };
    let mut q = query(vec![Filter::is("active", true)], vec![], 0, 10);
    q.aggregates = [
        totals("price", &[Aggregate::Sum, Aggregate::Avg, Aggregate::Min, Aggregate::Max, Aggregate::Count]),
        totals("qty", &[Aggregate::Sum, Aggregate::Avg, Aggregate::Min, Aggregate::Max]),
        totals("day", &[Aggregate::Min, Aggregate::Max]),
        totals("at", &[Aggregate::Min, Aggregate::Max, Aggregate::Count]),
        totals("clock", &[Aggregate::Max]),
        totals("note", &[Aggregate::Count]),
    ]
    .concat();
    same(&db, &all, q.clone()).await;
    let mut none = q.clone();
    none.filters = vec![Filter::contains("code", "nothing")];
    same(&db, &all, none).await;

    for (field, desc) in [("kind", false), ("active", true), ("note", false), ("price", false)] {
        let mut g = query(vec![], vec![(field, desc)], 0, 2);
        g.group = Some(field.into());
        g.aggregates = totals("qty", &[Aggregate::Sum, Aggregate::Avg]);
        same(&db, &all, g.clone()).await;
        g.page = PageRequest::Offset { offset: 2, limit: 2 };
        same(&db, &all, g).await;
    }

    for sort in [
        vec![("note", true), ("name", false)],
        vec![("note", false)],
        vec![("at", true)],
        vec![("price", true), ("day", false)],
    ] {
        let mut expected = query(vec![Filter::range("qty", Some("-3"), None)], sort.clone(), 0, 500);
        expected.with_total = false;
        let expected = expected.apply(all.clone());
        let mut walked = Vec::new();
        let mut cursor: Option<Cursor> = None;
        loop {
            let mut q = query(vec![Filter::range("qty", Some("-3"), None)], sort.clone(), 0, 0);
            q.page = PageRequest::After { cursor: cursor.clone(), limit: 17 };
            q.with_total = false;
            let page: Page<Item> = db.find(&q).await.unwrap();
            assert!(page.items.len() <= 17);
            walked.extend(ids(&page.items));
            match page.next {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(walked, ids(&expected.items), "keyset walk by {sort:?}");
    }

    let mut first: Item = db.get(ItemId(5)).await.unwrap();
    assert_eq!(first.stock, 5);
    let stale = first.clone();
    first.name = "Renamed".into();
    first.stock = 999;
    let saved = db.update(&first).await.unwrap();
    assert_eq!((saved.version, saved.name.as_str(), saved.stock), (2, "Renamed", 5), "readonly stock is not written");
    assert_eq!(db.update(&stale).await.unwrap_err(), Error::Conflict);
    assert_eq!(db.update(&Item { id: ItemId(99_999), version: 1, ..item(1) }).await.unwrap_err(), Error::Conflict);

    let dup = db.insert(&Item { code: "P-0007".into(), ..item(1) }).await.unwrap_err();
    let Error::Validation(errors) = dup else { panic!("{dup:?}") };
    assert_eq!(errors.errors[0].field, "code");
    assert_eq!(errors.errors[0].rule, zeyn::Rule::Unique);
    let mut taken = db.get::<Item>(ItemId(8)).await.unwrap();
    taken.code = "P-0009".into();
    assert!(matches!(db.update(&taken).await, Err(Error::Validation(_))));

    let mut tx = db.begin().await.unwrap();
    let a = tx.insert(&item(1001)).await.unwrap();
    tx.insert(&item(1002)).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(db.get::<Item>(a.id).await.unwrap_err(), Error::NotFound);
    {
        let mut tx = db.begin().await.unwrap();
        tx.insert(&item(1003)).await.unwrap();
    }
    let count: Page<Item> = db.find(&query(vec![Filter::contains("code", "P-100")], vec![], 0, 10)).await.unwrap();
    assert_eq!(count.total, Some(0), "a dropped transaction rolls back");
    let mut tx = db.begin().await.unwrap();
    let kept = tx.insert(&item(1004)).await.unwrap();
    let mut changed = tx.get::<Item>(kept.id).await.unwrap();
    changed.qty = 77;
    tx.update(&changed).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(db.get::<Item>(kept.id).await.unwrap().qty, 77);

    db.delete::<Item>(kept.id).await.unwrap();
    assert_eq!(db.delete::<Item>(kept.id).await.unwrap_err(), Error::NotFound);
    assert_eq!(db.get::<Item>(kept.id).await.unwrap_err(), Error::NotFound);

    assert_eq!(db.execute(raw.set_qty, vec![Arg::Int(5), Arg::Int(1)]).await.unwrap(), 1);

    let capped = Db::connect(url, PoolLimits { count_cap: 10, ..PoolLimits::default() }).await.unwrap();
    let page: Page<Item> = capped.find(&query(vec![], vec![], 0, 5)).await.unwrap();
    assert_eq!(page.total, Some(10));

    let mut forged = query(vec![], vec![], 0, 5);
    forged.page = PageRequest::After { cursor: Some(Cursor("eyJvIjpbXSwidiI6W119".into())), limit: 5 };
    assert!(matches!(db.find::<Item>(&forged).await, Err(Error::Validation(_))));
}

/// Entities have no `PartialEq`; compare them through JSON.
impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        serde_json::to_value(self).unwrap() == serde_json::to_value(other).unwrap()
    }
}

/// Streams a large result in order and drops a stream early without losing the connection.
pub async fn streams(url: &str, raw: &Raw) {
    let db = Db::connect(url, PoolLimits { max_connections: 2, ..PoolLimits::default() }).await.unwrap();
    db.execute(raw.fill_stream, vec![]).await.unwrap();

    let mut stream =
        db.stream::<Line>(&DataQuery { sort: vec![Sort { field: "id".into(), desc: true }], ..DataQuery::default() });
    let (mut n, mut last) = (0u64, i64::MAX);
    while let Some(row) = stream.next().await {
        let row = row.unwrap();
        assert!(row.id.0 < last);
        last = row.id.0;
        n += 1;
    }
    assert_eq!(n, STREAM_ROWS);

    let mut early = db.stream::<Line>(&DataQuery::default());
    assert!(early.next().await.unwrap().is_ok());
    drop(early);
    let page: Page<Line> = db.find(&DataQuery { with_total: true, ..DataQuery::default() }).await.unwrap();
    assert_eq!(page.total, Some(STREAM_ROWS), "the pool is usable after a stream is dropped early");
}
