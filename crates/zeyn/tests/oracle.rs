//! Oracle data layer against real databases, 11g and 12c or later.
//!
//! Needs Oracle Instant Client (see `docs/database.md`). Set `ZEYN_TEST_ORACLE` to one or more
//! URLs separated by spaces, for example with Docker:
//!
//! ```text
//! docker run -d --name zeyn-ora11 -p 15211:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:11-slim
//! docker run -d --name zeyn-ora21 -p 15221:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:21-slim
//! ZEYN_TEST_ORACLE="oracle://zeyn:zeyn@127.0.0.1:15211/XE oracle://zeyn:zeyn@127.0.0.1:15221/XEPDB1" cargo test -p zeyn --features oracle --test oracle
//! ```
//!
//! Without it every test returns early and says so.
#![cfg(feature = "oracle")]

mod sql_common;

use sql_common::{Item, Raw, urls};
use zeyn::prelude::*;
use zeyn::sql::{Db, PoolLimits};
use zeyn::{PageRequest, Sort};

const RAW: Raw = Raw {
    drop: &[
        "BEGIN EXECUTE IMMEDIATE 'DROP TABLE zeyn_test_items PURGE'; EXCEPTION WHEN OTHERS THEN NULL; END;",
        "BEGIN EXECUTE IMMEDIATE 'DROP SEQUENCE zeyn_test_items_seq'; EXCEPTION WHEN OTHERS THEN NULL; END;",
        "BEGIN EXECUTE IMMEDIATE 'DROP TABLE zeyn_test_stream PURGE'; EXCEPTION WHEN OTHERS THEN NULL; END;",
        "BEGIN EXECUTE IMMEDIATE 'DROP SEQUENCE zeyn_test_stream_seq'; EXCEPTION WHEN OTHERS THEN NULL; END;",
    ],
    set_qty: "UPDATE zeyn_test_items SET qty = :1 WHERE id = :2",
    fill_stream: "INSERT INTO zeyn_test_stream (id, text) SELECT LEVEL, 'line ' || LEVEL FROM DUAL CONNECT BY LEVEL <= 100000",
};

const QUERIES: sql_common::Queries = sql_common::Queries {
    drop: &[
        "BEGIN EXECUTE IMMEDIATE 'DROP TABLE zeyn_test_q_thing PURGE'; EXCEPTION WHEN OTHERS THEN NULL; END;",
        "BEGIN EXECUTE IMMEDIATE 'DROP TABLE zeyn_test_q_owner PURGE'; EXCEPTION WHEN OTHERS THEN NULL; END;",
    ],
    like: "SELECT id, name, note FROM zeyn_test_q_thing WHERE name LIKE :1 ORDER BY id",
    join: "SELECT t.id, t.name, o.name AS owner FROM zeyn_test_q_thing t LEFT JOIN zeyn_test_q_owner o ON o.id = t.owner_id WHERE t.id >= :1 ORDER BY t.id",
    by_id: "SELECT id, name, note FROM zeyn_test_q_thing WHERE id = :1",
    bad_type: "SELECT name AS id, name, note FROM zeyn_test_q_thing",
};

#[tokio::test(flavor = "multi_thread")]
async fn oracle_hand_written_queries_read_models() {
    for url in urls("ZEYN_TEST_ORACLE") {
        sql_common::queries(&url, &QUERIES).await;
        sql_common::queries(&format!("{url}?compat=11"), &QUERIES).await;
    }
}

const DROP_CARDS: [&str; 2] = [
    "BEGIN EXECUTE IMMEDIATE 'DROP TABLE zeyn_test_cards PURGE'; EXCEPTION WHEN OTHERS THEN NULL; END;",
    "BEGIN EXECUTE IMMEDIATE 'DROP SEQUENCE zeyn_test_cards_seq'; EXCEPTION WHEN OTHERS THEN NULL; END;",
];

/// A session that sorts and compares text by language rules, where `a` comes before `B`.
const LINGUISTIC: [&str; 2] = [
    "ALTER SESSION SET NLS_SORT = 'GERMAN'",
    "ALTER SESSION SET NLS_COMP = 'LINGUISTIC'",
];

/// One test so the databases are not shared by tests running at the same time.
#[tokio::test(flavor = "multi_thread")]
async fn oracle_data_layer_matches_memory_and_saves_safely() {
    for url in urls("ZEYN_TEST_ORACLE") {
        eprintln!(
            "oracle: {}",
            url.rsplit('@')
                .next()
                .unwrap_or_default()
        );
        sql_common::data_layer(&url, &RAW).await;
        sql_common::streams(&url, &RAW).await;
        for session in [&[][..], &LINGUISTIC[..]] {
            sql_common::ranks(&url, &DROP_CARDS, session).await;
            sql_common::ranks(&format!("{url}?compat=11"), &DROP_CARDS, session).await;
        }

        let auto = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        let rownum = Db::connect(&format!("{url}?compat=11"), PoolLimits::default())
            .await
            .unwrap();
        let q = DataQuery {
            page: PageRequest::Offset {
                offset: 40,
                limit: 15,
            },
            sort: vec![Sort {
                field: "name".into(),
                desc: true,
            }],
            with_total: true,
            ..DataQuery::default()
        };
        let a: Page<Item> = auto.find(&q).await.unwrap();
        let b: Page<Item> = rownum.find(&q).await.unwrap();
        assert_eq!(a.items.len(), 15);
        assert!(
            a.items == b.items && a.total == b.total,
            "ROWNUM paging matches the version's own paging"
        );

        let all = DataQuery {
            page: PageRequest::Offset {
                offset: 0,
                limit: 500,
            },
            ..DataQuery::default()
        };
        let version = auto
            .find::<Component>(&all)
            .await
            .unwrap()
            .items
            .into_iter()
            .find(|c| c.product.starts_with("Oracle"))
            .unwrap()
            .version;
        let major: u32 = version
            .split('.')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let sequences: Vec<String> = auto
            .find::<Sequence>(&all)
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|s| s.sequence_name)
            .collect();
        let own = sequences
            .iter()
            .any(|s| s == "ZEYN_TEST_ITEMS_SEQ");
        eprintln!("oracle {version}: key sequence {own}");
        assert_eq!(
            own,
            major < 12,
            "11g keys come from a sequence, later versions from identity columns"
        );
        sql_common::folded_search(&url, &RAW).await;
    }
}

/// The database's version, read like any other table.
#[entity(table = "product_component_version")]
pub struct Component {
    #[key]
    pub product: String,
    pub version: String,
}

#[entity(table = "user_sequences")]
pub struct Sequence {
    #[key]
    pub sequence_name: String,
}
