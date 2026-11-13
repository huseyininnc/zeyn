//! PostgreSQL data layer against a real database.
//!
//! Set `ZEYN_TEST_PG` to a database URL (several separated by spaces) to run them, for example
//! with Docker:
//!
//! ```text
//! docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
//! ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test postgres
//! ```
//!
//! The Alpine image sorts `en_US` text by bytes. To check that rank columns ignore a language
//! collation, also run against a database created with
//! `CREATE DATABASE zr LOCALE_PROVIDER icu ICU_LOCALE 'en-US' LOCALE 'C' TEMPLATE template0`.
//!
//! Without it every test returns early and says so.
#![cfg(feature = "postgres")]

mod sql_common;

use sql_common::{Raw, urls};
use zeyn::sql::{Arg, Db, PoolLimits};

const RAW: Raw = Raw {
    drop: &[
        "DROP TABLE IF EXISTS zeyn_test_items",
        "DROP TABLE IF EXISTS zeyn_test_stream",
    ],
    set_qty: "UPDATE zeyn_test_items SET qty = $1 WHERE id = $2",
    fill_stream: "INSERT INTO zeyn_test_stream (id, text) SELECT g, 'line ' || g FROM generate_series(1, 100000) g",
};

#[tokio::test(flavor = "multi_thread")]
async fn postgres_data_layer_matches_memory_and_saves_safely() {
    for url in urls("ZEYN_TEST_PG") {
        sql_common::data_layer(&url, &RAW).await;
        sql_common::streams(&url, &RAW).await;
        sql_common::folded_search(&url, &RAW).await;
        sql_common::ranks(&url, &["DROP TABLE IF EXISTS zeyn_test_cards"], &[]).await;
    }
}

const QUERIES: sql_common::Queries = sql_common::Queries {
    drop: &[
        "DROP TABLE IF EXISTS zeyn_test_q_thing",
        "DROP TABLE IF EXISTS zeyn_test_q_owner",
    ],
    like: "SELECT id, name, note FROM zeyn_test_q_thing WHERE name LIKE $1 ORDER BY id",
    join: "SELECT t.id, t.name, o.name AS owner FROM zeyn_test_q_thing t LEFT JOIN zeyn_test_q_owner o ON o.id = t.owner_id WHERE t.id >= $1 ORDER BY t.id",
    by_id: "SELECT id, name, note FROM zeyn_test_q_thing WHERE id = $1",
    bad_type: "SELECT name AS id, name, note FROM zeyn_test_q_thing",
};

#[tokio::test(flavor = "multi_thread")]
async fn postgres_hand_written_queries_read_models() {
    for url in urls("ZEYN_TEST_PG") {
        sql_common::queries(&url, &QUERIES).await;
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        db.execute("CREATE EXTENSION IF NOT EXISTS pg_trgm", vec![])
            .await
            .unwrap();
        db.execute("DROP TABLE IF EXISTS zeyn_test_q_title", vec![])
            .await
            .unwrap();
        db.execute(
            "CREATE TABLE zeyn_test_q_title AS SELECT g AS id, 'title ' || g AS name, NULL::text AS note FROM generate_series(1, 1500) g",
            vec![],
        )
        .await
        .unwrap();
        let near: Vec<sql_common::Thing> = db
            .query(
                "SELECT id, name, note, similarity(name, $1) AS score FROM zeyn_test_q_title WHERE name % $1 ORDER BY score DESC, id LIMIT $2",
                vec!["title 1499".into(), 5.into()],
            )
            .await
            .unwrap();
        assert_eq!(near.len(), 5);
        assert_eq!(near[0].name, "title 1499");
        let unbounded = db
            .query::<sql_common::Thing>("SELECT id, name, note FROM zeyn_test_q_title", vec![])
            .await
            .unwrap_err();
        assert!(
            format!("{unbounded:?}").contains("cap of 1000"),
            "{unbounded:?}"
        );
        db.execute("DROP TABLE zeyn_test_q_title", vec![])
            .await
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn postgres_runs_migrations() {
    for url in urls("ZEYN_TEST_PG") {
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        db.execute("DROP TABLE IF EXISTS zeyn_test_migrated", vec![])
            .await
            .unwrap();
        db.execute("DROP TABLE IF EXISTS _sqlx_migrations", vec![])
            .await
            .unwrap();
        let dir = std::env::temp_dir().join(format!("zeyn-migrations-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("0001_create.sql"),
            "CREATE TABLE zeyn_test_migrated (id BIGINT PRIMARY KEY);",
        )
        .unwrap();
        db.migrate_dir(&dir).await.unwrap();
        db.migrate_dir(&dir).await.unwrap();
        assert_eq!(
            db.execute(
                "INSERT INTO zeyn_test_migrated (id) VALUES ($1)",
                vec![Arg::Int(1)]
            )
            .await
            .unwrap(),
            1
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// A screen that reads the database while it is built, for the DevTools test.
#[cfg(debug_assertions)]
#[zeyn::prelude::screen(route = "/counted", public)]
async fn counted(_: (), db: Db) -> zeyn::prelude::Result<zeyn::prelude::Screen> {
    db.execute("SELECT 1", vec![]).await?;
    Ok(zeyn::prelude::Screen::new("Counted"))
}

#[cfg(debug_assertions)]
#[tokio::test(flavor = "multi_thread")]
async fn postgres_statements_show_in_the_devtools_with_their_time() {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use zeyn::prelude::{App, Feature};
    for url in urls("ZEYN_TEST_PG") {
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        let router = App::new("Counted")
            .service(db)
            .feature(Feature::new("counted").screen(counted))
            .router()
            .unwrap();
        let get = |path: &str| {
            axum::http::Request::get(path)
                .body(axum::body::Body::empty())
                .unwrap()
        };
        zeyn::server::devtools(true);
        router
            .clone()
            .oneshot(get("/counted"))
            .await
            .unwrap();
        let res = router
            .oneshot(get("/_zeyn/devtools"))
            .await
            .unwrap();
        zeyn::server::devtools(false);
        let bytes = res
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let log: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let page = &log["requests"][0];
        assert_eq!(page["query_count"], 1, "{log}");
        assert_eq!(page["queries"][0]["sql"], "SELECT 1");
        assert!(
            page["queries"][0]["ms"]
                .as_f64()
                .is_some()
        );
    }
}
