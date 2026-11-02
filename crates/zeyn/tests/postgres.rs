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
//! Without it every test returns early and says so.
#![cfg(feature = "postgres")]

mod sql_common;

use sql_common::{Raw, urls};
use zeyn::sql::{Arg, Db, PoolLimits};

const RAW: Raw = Raw {
    drop: &["DROP TABLE IF EXISTS zeyn_test_items", "DROP TABLE IF EXISTS zeyn_test_stream"],
    set_qty: "UPDATE zeyn_test_items SET qty = $1 WHERE id = $2",
    fill_stream: "INSERT INTO zeyn_test_stream (id, text) SELECT g, 'line ' || g FROM generate_series(1, 100000) g",
};

#[tokio::test(flavor = "multi_thread")]
async fn postgres_data_layer_matches_memory_and_saves_safely() {
    for url in urls("ZEYN_TEST_PG") {
        sql_common::data_layer(&url, &RAW).await;
        sql_common::streams(&url, &RAW).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn postgres_runs_migrations() {
    for url in urls("ZEYN_TEST_PG") {
        let db = Db::connect(&url, PoolLimits::default()).await.unwrap();
        db.execute("DROP TABLE IF EXISTS zeyn_test_migrated", vec![]).await.unwrap();
        db.execute("DROP TABLE IF EXISTS _sqlx_migrations", vec![]).await.unwrap();
        let dir = std::env::temp_dir().join(format!("zeyn-migrations-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("0001_create.sql"), "CREATE TABLE zeyn_test_migrated (id BIGINT PRIMARY KEY);")
            .unwrap();
        db.migrate_dir(&dir).await.unwrap();
        db.migrate_dir(&dir).await.unwrap();
        assert_eq!(db.execute("INSERT INTO zeyn_test_migrated (id) VALUES ($1)", vec![Arg::Int(1)]).await.unwrap(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
