//! SQL builder tests. They need no database.

use super::build::{self, Shape};
use super::*;
use crate::action::{Aggregate, AggregateRequest, Cursor, DataQuery, Filter, PageRequest, Sort};
use crate::value::Money;
use crate::{Rule, Value};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Product {
    id: i64,
    code: String,
    note: Option<String>,
    price: Money,
    active: bool,
    stock: i64,
    version: i64,
}

const fn meta(name: &'static str, kind: crate::FieldKind) -> crate::FieldMeta {
    crate::FieldMeta {
        model: "Product",
        name,
        label: name,
        kind,
        rules: &[],
        key: false,
        readonly: false,
        version: false,
        optional: false,
        mask: None,
    }
}

static FIELDS: [crate::FieldMeta; 7] = [
    crate::FieldMeta {
        key: true,
        ..meta("id", crate::FieldKind::Id)
    },
    crate::FieldMeta {
        rules: &[Rule::Unique],
        ..meta("code", crate::FieldKind::Text)
    },
    crate::FieldMeta {
        optional: true,
        ..meta("note", crate::FieldKind::Text)
    },
    meta("price", crate::FieldKind::Money),
    meta("active", crate::FieldKind::Bool),
    crate::FieldMeta {
        readonly: true,
        ..meta("stock", crate::FieldKind::Integer)
    },
    crate::FieldMeta {
        version: true,
        ..meta("version", crate::FieldKind::Integer)
    },
];

impl crate::Model for Product {
    const NAME: &'static str = "Product";
    fn fields() -> &'static [crate::FieldMeta] {
        &FIELDS
    }
    fn value_at(&self, index: usize) -> Option<Value> {
        use crate::FieldType;
        Some(match index {
            0 => Value::Int(self.id),
            1 => self.code.to_value(),
            2 => self.note.to_value(),
            3 => Value::Money(self.price),
            4 => Value::Bool(self.active),
            5 => Value::Int(self.stock),
            6 => Value::Int(self.version),
            _ => return None,
        })
    }
    fn validate(&self) -> std::result::Result<(), crate::ValidationErrors> {
        Ok(())
    }
}

impl crate::Entity for Product {
    const TABLE: &'static str = "shop.products";
}

const SELECT: &str = r#"SELECT CAST("id" AS BIGINT) AS c0, "code" AS c1, "note" AS c2, CAST("price" AS BIGINT) AS c3, "price_currency" AS c4, "active" AS c5, CAST("stock" AS BIGINT) AS c6, CAST("version" AS BIGINT) AS c7 FROM "shop"."products""#;

fn plan(q: &DataQuery) -> build::FindPlan {
    build::plan_find::<Product>(&Postgres, q, 1000).unwrap()
}

fn rows(q: &DataQuery) -> Sql {
    plan(q).main
}

#[test]
fn plain_page_sorts_by_key_and_fetches_one_extra_row() {
    let sql = rows(&DataQuery::default());
    assert_eq!(
        sql.text,
        format!(r#"{SELECT} ORDER BY "id" ASC LIMIT $1 OFFSET $2"#)
    );
    assert_eq!(sql.args, vec![Arg::Int(51), Arg::Int(0)]);
    assert_eq!(sql.cols.len(), 8);
}

#[test]
fn client_limit_is_clamped_and_offset_is_bound() {
    let q = DataQuery {
        page: PageRequest::Offset {
            offset: 40,
            limit: 100_000,
        },
        ..DataQuery::default()
    };
    assert_eq!(rows(&q).args, vec![Arg::Int(501), Arg::Int(40)]);
}

#[test]
fn filters_become_bound_parameters() {
    let q = DataQuery {
        filters: vec![
            Filter::contains("code", "Te%a_"),
            Filter::range("price", Some("10"), Some("99.90")),
            Filter::range("stock", Some("1.5"), Some("7.5")),
            Filter::is("active", true),
            Filter::blank("note", true),
        ],
        ..DataQuery::default()
    };
    let sql = rows(&q);
    assert!(sql.text.contains(
        r#" WHERE LOWER("code") LIKE $1 ESCAPE '\' AND "price" >= $2 AND "price" <= $3 AND "stock" >= $4 AND "stock" <= $5 AND "active" = $6 AND "note" IS NULL ORDER BY"#
    ), "{}", sql.text);
    assert_eq!(
        sql.args[..6],
        [
            Arg::Text(r"%te\%a\_%".into()),
            Arg::Int(1000),
            Arg::Int(9990),
            Arg::Int(2),
            Arg::Int(7),
            Arg::Bool(true)
        ]
    );
}

#[test]
fn free_text_searches_every_text_field_with_one_bound_pattern_each() {
    let q = DataQuery {
        q: " İstanbul_%".into(),
        filters: vec![Filter::is("active", true)],
        ..DataQuery::default()
    };
    let sql = rows(&q);
    assert!(sql.text.contains(
        r#" WHERE "active" = $1 AND (LOWER(TRANSLATE("code", 'İı', 'ii')) LIKE $2 ESCAPE '\' OR LOWER(TRANSLATE("note", 'İı', 'ii')) LIKE $3 ESCAPE '\') ORDER BY"#
    ), "{}", sql.text);
    assert_eq!(sql.args[1], Arg::Text(r"%istanbul\_\%%".into()));
    let long = DataQuery {
        q: "x".repeat(crate::MAX_SEARCH_QUERY + 1),
        ..DataQuery::default()
    };
    assert!(
        !rows(&long).text.contains("WHERE"),
        "text over the cap is ignored"
    );
}

#[test]
fn unknown_fields_and_injection_attempts_never_reach_the_sql() {
    let q = DataQuery {
        filters: vec![
            Filter::contains("code\"; DROP TABLE products; --", "x"),
            Filter::contains("code", "'; DROP TABLE x; --"),
        ],
        sort: vec![Sort {
            field: "1; DROP TABLE products".into(),
            desc: true,
        }],
        ..DataQuery::default()
    };
    let sql = rows(&q);
    assert!(!sql.text.contains("DROP"), "{}", sql.text);
    assert_eq!(sql.args[0], Arg::Text("%'; drop table x; --%".into()));
    assert!(
        sql.text
            .ends_with(r#"ORDER BY "id" ASC LIMIT $2 OFFSET $3"#)
    );
}

#[test]
fn bad_table_names_are_refused() {
    struct Bad;
    let d: &dyn Dialect = &Postgres;
    assert!(build::ident(d, "products; drop").is_err());
    assert!(build::ident(d, "a.b.c").is_err());
    assert!(build::ident(d, &"x".repeat(64)).is_err());
    assert_eq!(
        build::ident(d, "shop.products").unwrap(),
        r#""shop"."products""#
    );
    let _ = Bad;
}

#[test]
fn choices_and_keys_use_in_lists() {
    let q = DataQuery {
        filters: vec![Filter::any_of("id", ["3", "x", "9"])],
        ..DataQuery::default()
    };
    let sql = rows(&q);
    assert!(
        sql.text
            .contains(r#"WHERE "id" IN ($1, $2)"#),
        "{}",
        sql.text
    );
    assert_eq!(sql.args[..2], [Arg::Int(3), Arg::Int(9)]);
    let none = DataQuery {
        filters: vec![Filter::any_of("id", ["x"])],
        ..DataQuery::default()
    };
    assert!(rows(&none).text.contains("WHERE 1 = 0"));
}

#[test]
fn money_contains_is_ignored_like_unsupported_conditions() {
    let q = DataQuery {
        filters: vec![Filter::contains("price", "12")],
        ..DataQuery::default()
    };
    assert!(!rows(&q).text.contains("WHERE"));
}

#[test]
fn nullable_sort_keys_place_nulls_like_memory() {
    let q = DataQuery {
        sort: vec![
            Sort {
                field: "note".into(),
                desc: true,
            },
            Sort {
                field: "code".into(),
                desc: false,
            },
        ],
        ..DataQuery::default()
    };
    assert!(
        rows(&q)
            .text
            .contains(r#"ORDER BY "note" DESC NULLS LAST, "code" ASC, "id" ASC"#)
    );
}

fn product(id: i64, code: &str, note: Option<&str>) -> Product {
    Product {
        id,
        code: code.into(),
        note: note.map(Into::into),
        price: Money::tl(100),
        active: true,
        stock: 1,
        version: 1,
    }
}

fn keys(q: &DataQuery) -> Vec<build::OrderKey> {
    match plan(q).shape {
        Shape::Rows(k) => k,
        Shape::Groups { .. } => unreachable!(),
    }
}

#[test]
fn keyset_cursor_continues_after_the_last_row() {
    let base = DataQuery {
        sort: vec![Sort {
            field: "code".into(),
            desc: true,
        }],
        ..DataQuery::default()
    };
    let cursor = build::row_cursor(&Postgres, &keys(&base), &product(7, "P-7", None));
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(cursor),
            limit: 20,
        },
        ..base
    };
    let sql = rows(&q);
    assert!(
        sql.text.contains(r#" WHERE "code" <= $1 AND (("code" < $2) OR ("code" = $3 AND "id" > $4)) ORDER BY "code" DESC, "id" ASC LIMIT $5 OFFSET $6"#),
        "{}",
        sql.text
    );
    assert_eq!(
        sql.args,
        vec![
            Arg::Text("P-7".into()),
            Arg::Text("P-7".into()),
            Arg::Text("P-7".into()),
            Arg::Int(7),
            Arg::Int(21),
            Arg::Int(0)
        ]
    );
}

#[test]
fn keyset_cursor_handles_null_sort_values() {
    let base = DataQuery {
        sort: vec![Sort {
            field: "note".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let cursor = build::row_cursor(&Postgres, &keys(&base), &product(4, "A", None));
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(cursor),
            limit: 10,
        },
        ..base.clone()
    };
    assert!(
        rows(&q)
            .text
            .contains(r#"WHERE (("note" IS NOT NULL) OR ("note" IS NULL AND "id" > $1))"#),
        "{}",
        rows(&q).text
    );

    let desc = DataQuery {
        sort: vec![Sort {
            field: "note".into(),
            desc: true,
        }],
        ..DataQuery::default()
    };
    let cursor = build::row_cursor(&Postgres, &keys(&desc), &product(4, "A", Some("b")));
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(cursor),
            limit: 10,
        },
        ..desc
    };
    assert!(
        rows(&q).text.contains(
            r#"WHERE ((("note" < $1 OR "note" IS NULL)) OR ("note" = $2 AND "id" > $3))"#
        ),
        "{}",
        rows(&q).text
    );
}

#[test]
fn cursors_from_another_order_or_tampered_are_refused() {
    let by_code = DataQuery {
        sort: vec![Sort {
            field: "code".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let cursor = build::row_cursor(&Postgres, &keys(&by_code), &product(1, "A", None));
    let other = DataQuery {
        page: PageRequest::After {
            cursor: Some(cursor),
            limit: 10,
        },
        ..DataQuery::default()
    };
    let err = build::plan_find::<Product>(&Postgres, &other, 10)
        .err()
        .unwrap();
    assert_eq!(err.code(), "validation");
    for junk in ["!!!", "e30", &"A".repeat(9000)] {
        let q = DataQuery {
            page: PageRequest::After {
                cursor: Some(Cursor(junk.into())),
                limit: 10,
            },
            ..DataQuery::default()
        };
        assert!(
            build::plan_find::<Product>(&Postgres, &q, 10).is_err(),
            "{junk}"
        );
    }
    let wrong_type = build::row_cursor(
        &Postgres,
        &keys(&DataQuery::default()),
        &product(1, "A", None),
    );
    let text = String::from_utf8(
        base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &wrong_type.0,
        )
        .unwrap(),
    )
    .unwrap();
    let forged = text.replace("[1]", "[\"1 OR 1=1\"]");
    let forged = Cursor(base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        forged,
    ));
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(forged),
            limit: 10,
        },
        ..DataQuery::default()
    };
    assert!(build::plan_find::<Product>(&Postgres, &q, 10).is_err());
}

#[test]
fn total_is_a_capped_count() {
    let q = DataQuery {
        with_total: true,
        filters: vec![Filter::is("active", true)],
        ..DataQuery::default()
    };
    let count = plan(&q).count.unwrap();
    assert_eq!(
        count.text,
        r#"SELECT COUNT(*) AS c0 FROM (SELECT 1 AS c0 FROM "shop"."products" WHERE "active" = $1 LIMIT $2 OFFSET $3) zc"#
    );
    assert_eq!(
        count.args,
        vec![Arg::Bool(true), Arg::Int(1000), Arg::Int(0)]
    );
}

#[test]
fn totals_are_one_aggregate_query_and_skip_unfit_columns() {
    let q = DataQuery {
        aggregates: vec![
            AggregateRequest {
                field: "price".into(),
                op: Aggregate::Sum,
            },
            AggregateRequest {
                field: "stock".into(),
                op: Aggregate::Avg,
            },
            AggregateRequest {
                field: "code".into(),
                op: Aggregate::Sum,
            },
            AggregateRequest {
                field: "code".into(),
                op: Aggregate::Count,
            },
        ],
        ..DataQuery::default()
    };
    let (sql, aggs) = plan(&q).totals.unwrap();
    assert_eq!(aggs.len(), 3);
    assert_eq!(
        sql.text,
        r#"SELECT CAST(COALESCE(SUM("price"), 0) AS BIGINT) AS c0, MIN("price_currency") AS c1, MAX("price_currency") AS c2, CAST(AVG("stock") AS DOUBLE PRECISION) AS c3, CAST(COUNT("code") AS BIGINT) AS c4 FROM "shop"."products""#
    );
    let cells = vec![
        Cell::Int(250),
        Cell::Text("TRY".into()),
        Cell::Text("TRY".into()),
        Cell::Float(2.5),
        Cell::Int(4),
    ];
    let totals = build::read_aggs(Product::fields_static(), &aggs, &mut cells.into_iter());
    assert_eq!(totals[0].value, Value::Money(Money::tl(250)));
    assert_eq!(totals[1].value, Value::Decimal(2.5));
    assert_eq!(totals[2].value, Value::Int(4));
    let mixed = vec![
        Cell::Int(250),
        Cell::Text("TRY".into()),
        Cell::Text("USD".into()),
        Cell::Null,
        Cell::Int(0),
    ];
    let totals = build::read_aggs(Product::fields_static(), &aggs, &mut mixed.into_iter());
    assert_eq!(totals.len(), 2, "money in two currencies has no total");
    assert_eq!(totals[0].value, Value::Null, "average of nothing is empty");
}

impl Product {
    fn fields_static() -> &'static [crate::FieldMeta] {
        &FIELDS
    }
}

#[test]
fn groups_use_group_by_and_page_like_rows() {
    let q = DataQuery {
        group: Some("active".into()),
        with_total: true,
        aggregates: vec![AggregateRequest {
            field: "stock".into(),
            op: Aggregate::Sum,
        }],
        ..DataQuery::default()
    };
    let p = plan(&q);
    assert_eq!(
        p.main.text,
        r#"SELECT "active" AS c0, CAST(COUNT(*) AS BIGINT) AS c1, CAST(COALESCE(SUM("stock"), 0) AS BIGINT) AS c2 FROM "shop"."products" GROUP BY "active" ORDER BY "active" ASC NULLS FIRST LIMIT $1 OFFSET $2"#
    );
    assert!(
        p.count
            .unwrap()
            .text
            .contains("GROUP BY \"active\" LIMIT")
    );
    assert!(matches!(p.shape, Shape::Groups { offset: 0, .. }));
    let next = build::group_cursor(50);
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(next),
            limit: 50,
        },
        ..q
    };
    assert!(matches!(plan(&q).shape, Shape::Groups { offset: 50, .. }));
}

#[test]
fn insert_skips_new_key_and_readonly_and_starts_the_version() {
    let sql = build::plan_insert(&Postgres, &product(0, "A", Some("n")), None, true).unwrap();
    assert!(sql.text.starts_with(
        r#"INSERT INTO "shop"."products" ("code", "note", "price", "price_currency", "active", "version") VALUES ($1, $2, $3, $4, $5, $6) RETURNING CAST("id" AS BIGINT) AS c0"#
    ), "{}", sql.text);
    assert_eq!(
        sql.args,
        vec![
            Arg::Text("A".into()),
            Arg::Text("n".into()),
            Arg::Int(100),
            Arg::Text("TRY".into()),
            Arg::Bool(true),
            Arg::Int(1)
        ]
    );
    let none = build::plan_insert(&Postgres, &product(0, "A", None), None, false).unwrap();
    assert_eq!(none.args[1], Arg::Null(Col::Text));
}

#[test]
fn update_checks_the_row_version() {
    let mut p = product(9, "A", None);
    p.version = 3;
    let (sql, versioned) = build::plan_update(&Postgres, &p, false).unwrap();
    assert!(versioned);
    assert_eq!(
        sql.text,
        r#"UPDATE "shop"."products" SET "code" = $1, "note" = $2, "price" = $3, "price_currency" = $4, "active" = $5, "version" = "version" + 1 WHERE "id" = $6 AND "version" = $7"#
    );
    assert_eq!(sql.args[5..], [Arg::Int(9), Arg::Int(3)]);
}

#[test]
fn unique_violations_name_the_field() {
    let Err(crate::Error::Validation(e)) =
        Err::<(), _>(build::unique_error::<Product>(Some("products_code_key")))
    else {
        panic!()
    };
    assert_eq!(e.errors[0].field, "code");
    assert_eq!(e.errors[0].rule, Rule::Unique);
}

#[test]
fn rows_decode_into_models_without_a_map() {
    let mut cells = vec![
        Cell::Int(3),
        Cell::Text("P-3".into()),
        Cell::Null,
        Cell::Int(1250),
        Cell::Text("USD".into()),
        Cell::Bool(true),
        Cell::Int(5),
        Cell::Int(2),
    ];
    let p: Product = super::decode::decode_row(&mut cells).unwrap();
    assert!(cells.is_empty());
    assert_eq!(p.code, "P-3");
    assert_eq!(p.note, None);
    assert_eq!(p.price, Money::new(1250, crate::Currency::Usd));
    let mut empty = vec![
        Cell::Int(3),
        Cell::Null,
        Cell::Null,
        Cell::Int(0),
        Cell::Text("TRY".into()),
        Cell::Bool(false),
        Cell::Int(0),
        Cell::Int(1),
    ];
    let p: Product = super::decode::decode_row(&mut empty).unwrap();
    assert_eq!(
        p.code, "",
        "NULL in a required text field reads as empty (Oracle)"
    );
}

#[test]
fn create_table_scaffolds_constraints() {
    let sql = create_table::<Product>(&Postgres).unwrap();
    assert_eq!(
        sql[0],
        "CREATE TABLE \"shop\".\"products\" (\n    \"id\" BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,\n    \"code\" TEXT NOT NULL,\n    CONSTRAINT \"products_code_key\" UNIQUE (\"code\"),\n    \"note\" TEXT,\n    \"price\" BIGINT NOT NULL,\n    \"price_currency\" VARCHAR(3) NOT NULL,\n    \"active\" BOOLEAN NOT NULL,\n    \"stock\" BIGINT DEFAULT 0 NOT NULL,\n    \"version\" BIGINT DEFAULT 1 NOT NULL\n)"
    );
}

#[test]
fn pool_limits_come_from_config() {
    let config = crate::Config::default();
    assert_eq!(
        PoolLimits::from_config(&config).unwrap(),
        PoolLimits::default()
    );
}

const V11: Oracle = Oracle {
    compat: OracleCompat::V11,
};
const V12: Oracle = Oracle {
    compat: OracleCompat::V12,
};

#[test]
fn oracle_12c_pages_with_offset_fetch() {
    let q = DataQuery {
        page: PageRequest::Offset {
            offset: 20,
            limit: 10,
        },
        sort: vec![Sort {
            field: "code".into(),
            desc: false,
        }],
        filters: vec![Filter::contains("code", "a"), Filter::is("active", true)],
        ..DataQuery::default()
    };
    let sql = build::plan_find::<Product>(&V12, &q, 100)
        .unwrap()
        .main;
    assert_eq!(
        sql.text,
        r#"SELECT "ID" AS c0, "CODE" AS c1, "NOTE" AS c2, "PRICE" AS c3, "PRICE_CURRENCY" AS c4, "ACTIVE" AS c5, "STOCK" AS c6, "VERSION" AS c7 FROM "SHOP"."PRODUCTS" WHERE LOWER("CODE") LIKE :1 ESCAPE '\' AND "ACTIVE" = :2 ORDER BY "CODE" ASC NULLS FIRST, "ID" ASC OFFSET :3 ROWS FETCH NEXT :4 ROWS ONLY"#
    );
    assert_eq!(
        sql.args,
        vec![
            Arg::Text("%a%".into()),
            Arg::Bool(true),
            Arg::Int(20),
            Arg::Int(11)
        ]
    );
}

#[test]
fn oracle_11g_pages_with_rownum() {
    let q = DataQuery {
        page: PageRequest::Offset {
            offset: 20,
            limit: 10,
        },
        ..DataQuery::default()
    };
    let sql = build::plan_find::<Product>(&V11, &q, 100)
        .unwrap()
        .main;
    assert!(
        sql.text
            .starts_with(r#"SELECT * FROM (SELECT zq.*, ROWNUM zeyn_rn FROM (SELECT "ID" AS c0"#),
        "{}",
        sql.text
    );
    assert!(
        sql.text.ends_with(
            r#" ORDER BY "ID" ASC) zq WHERE ROWNUM <= :1) WHERE zeyn_rn > :2 ORDER BY zeyn_rn"#
        ),
        "{}",
        sql.text
    );
    assert_eq!(sql.args, vec![Arg::Int(31), Arg::Int(20)]);
    let count = build::plan_find::<Product>(
        &V11,
        &DataQuery {
            with_total: true,
            ..DataQuery::default()
        },
        500,
    )
    .unwrap()
    .count
    .unwrap();
    assert_eq!(
        count.text,
        r#"SELECT COUNT(*) AS c0 FROM (SELECT * FROM (SELECT zq.*, ROWNUM zeyn_rn FROM (SELECT 1 AS c0 FROM "SHOP"."PRODUCTS") zq WHERE ROWNUM <= :1) WHERE zeyn_rn > :2 ORDER BY zeyn_rn) zc"#
    );
}

#[test]
fn oracle_dates_convert_with_fixed_formats() {
    let mut out = String::new();
    V11.param(&mut out, 3, Cast::Moment);
    assert_eq!(out, r#"TO_TIMESTAMP(:3, 'YYYY-MM-DD"T"HH24:MI:SS.FF3"Z"')"#);
    out.clear();
    V12.read(&mut out, r#""DAY""#, Cast::Date);
    assert_eq!(out, r#"TO_CHAR("DAY", 'YYYY-MM-DD')"#);
    out.clear();
    V12.param(&mut out, 1, Cast::Time);
    assert_eq!(out, ":1", "times are VARCHAR2(8) text that sorts in order");
}

#[test]
fn oracle_treats_empty_text_as_null_in_cursors_and_order() {
    let q = DataQuery {
        sort: vec![Sort {
            field: "code".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let keys = match build::plan_find::<Product>(&V12, &q, 10)
        .unwrap()
        .shape
    {
        Shape::Rows(k) => k,
        Shape::Groups { .. } => unreachable!(),
    };
    assert!(keys[0].nullable);
    let cursor = build::row_cursor(&V12, &keys, &product(5, "", None));
    let next = DataQuery {
        page: PageRequest::After {
            cursor: Some(cursor),
            limit: 10,
        },
        ..q
    };
    let sql = build::plan_find::<Product>(&V12, &next, 10)
        .unwrap()
        .main;
    assert!(
        sql.text
            .contains(r#"WHERE (("CODE" IS NOT NULL) OR ("CODE" IS NULL AND "ID" > :1))"#),
        "{}",
        sql.text
    );
}

#[test]
fn oracle_create_table_uses_a_sequence_on_11g_and_identity_later() {
    let v11 = create_table::<Product>(&V11).unwrap();
    assert!(
        v11[0].contains(r#""ID" NUMBER(19) PRIMARY KEY"#),
        "{}",
        v11[0]
    );
    assert!(
        v11[0].contains(r#""CODE" VARCHAR2(4000),"#),
        "text is nullable because '' is NULL: {}",
        v11[0]
    );
    assert!(v11[0].contains(r#""ACTIVE" NUMBER(1) NOT NULL"#));
    assert!(v11[0].contains(r#""STOCK" NUMBER(19) DEFAULT 0 NOT NULL"#));
    assert_eq!(v11[1], r#"CREATE SEQUENCE "SHOP"."PRODUCTS_SEQ""#);
    let v12 = create_table::<Product>(&V12).unwrap();
    assert_eq!(v12.len(), 1);
    assert!(
        v12[0].contains(r#""ID" NUMBER(19) GENERATED BY DEFAULT ON NULL AS IDENTITY PRIMARY KEY"#)
    );
}

#[test]
fn oracle_writes_without_returning_lists() {
    let sql = build::plan_insert(&V11, &product(0, "A", None), Some(Arg::Int(42)), false).unwrap();
    assert_eq!(
        sql.text,
        r#"INSERT INTO "SHOP"."PRODUCTS" ("ID", "CODE", "NOTE", "PRICE", "PRICE_CURRENCY", "ACTIVE", "VERSION") VALUES (:1, :2, :3, :4, :5, :6, :7)"#
    );
    assert_eq!(sql.args[0], Arg::Int(42));
    let (update, _) = build::plan_update(&V12, &product(4, "A", None), V12.returning()).unwrap();
    assert!(
        update
            .text
            .ends_with(r#"WHERE "ID" = :6 AND "VERSION" = :7"#),
        "{}",
        update.text
    );
}

#[test]
fn oracle_identifiers_stay_within_30_characters() {
    assert!(build::ident(&V11, &"a".repeat(30)).is_ok());
    assert!(build::ident(&V11, &"a".repeat(31)).is_err());
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Holder {
    id: i64,
    tax_id: String,
}

static HOLDER_FIELDS: [crate::FieldMeta; 2] = [
    crate::FieldMeta {
        key: true,
        model: "Holder",
        ..meta("id", crate::FieldKind::Id)
    },
    crate::FieldMeta {
        model: "Holder",
        mask: Some(crate::Masking {
            style: crate::MaskStyle::LastFour,
            permission: "holder.tax_id.unmask",
        }),
        ..meta("tax_id", crate::FieldKind::Text)
    },
];

impl crate::Model for Holder {
    const NAME: &'static str = "Holder";
    fn fields() -> &'static [crate::FieldMeta] {
        &HOLDER_FIELDS
    }
    fn value_at(&self, index: usize) -> Option<Value> {
        use crate::FieldType;
        Some(match index {
            0 => Value::Int(self.id),
            1 => self.tax_id.to_value(),
            _ => return None,
        })
    }
    fn validate(&self) -> std::result::Result<(), crate::ValidationErrors> {
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Measure {
    id: i64,
    rate: crate::Decimal,
}

static MEASURE_FIELDS: [crate::FieldMeta; 2] = [
    crate::FieldMeta {
        key: true,
        model: "Measure",
        ..meta("id", crate::FieldKind::Id)
    },
    crate::FieldMeta {
        model: "Measure",
        rules: &[Rule::Precision(12), Rule::Scale(4)],
        ..meta("rate", crate::FieldKind::Decimal)
    },
];

impl crate::Model for Measure {
    const NAME: &'static str = "Measure";
    fn fields() -> &'static [crate::FieldMeta] {
        &MEASURE_FIELDS
    }
    fn value_at(&self, index: usize) -> Option<Value> {
        Some(match index {
            0 => Value::Int(self.id),
            1 => Value::Number(self.rate),
            _ => return None,
        })
    }
    fn validate(&self) -> std::result::Result<(), crate::ValidationErrors> {
        Ok(())
    }
}

impl crate::Entity for Holder {
    const TABLE: &'static str = "holders";
}

#[test]
fn cursors_of_orders_on_masked_fields_hold_no_values() {
    let by_tax = DataQuery {
        sort: vec![Sort {
            field: "tax_id".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let plan = build::plan_find::<Holder>(&Postgres, &by_tax, 10).unwrap();
    let Shape::Rows(keys) = plan.shape else {
        unreachable!()
    };
    let row = Holder {
        id: 7,
        tax_id: "12345678901".into(),
    };
    let next = build::next_cursor(&Postgres, &keys, &row, 0, 50);
    let text = String::from_utf8(
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &next.0).unwrap(),
    )
    .unwrap();
    assert!(
        !text.contains("12345678901") && !text.contains("8901"),
        "{text}"
    );

    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(next),
            limit: 50,
        },
        ..by_tax.clone()
    };
    let p = build::plan_find::<Holder>(&Postgres, &q, 10).unwrap();
    assert!(!p.main.text.contains("WHERE"), "{}", p.main.text);
    assert_eq!(p.skip, 50);
    assert_eq!(p.main.args.last(), Some(&Arg::Int(50)));

    let keyset = build::row_cursor(&Postgres, &keys, &row);
    let forged = DataQuery {
        page: PageRequest::After {
            cursor: Some(keyset),
            limit: 50,
        },
        ..by_tax
    };
    assert!(
        build::plan_find::<Holder>(&Postgres, &forged, 10).is_err(),
        "a keyset cursor is refused here"
    );

    let plain = build::plan_find::<Holder>(&Postgres, &DataQuery::default(), 10).unwrap();
    let Shape::Rows(keys) = plain.shape else {
        unreachable!()
    };
    let next = build::next_cursor(&Postgres, &keys, &row, 0, 50);
    let q = DataQuery {
        page: PageRequest::After {
            cursor: Some(next),
            limit: 50,
        },
        ..DataQuery::default()
    };
    assert!(
        build::plan_find::<Holder>(&Postgres, &q, 10)
            .unwrap()
            .main
            .text
            .contains(r#""id" > $1"#)
    );
}

impl crate::Entity for Measure {
    const TABLE: &'static str = "measures";
}

#[test]
fn decimals_map_to_numeric_columns_with_their_precision_and_scale() {
    let pg = create_table::<Measure>(&Postgres).unwrap();
    assert!(
        pg[0].contains(r#""rate" NUMERIC(12, 4) NOT NULL"#),
        "{}",
        pg[0]
    );
    let ora = create_table::<Measure>(&V12).unwrap();
    assert!(
        ora[0].contains(r#""RATE" NUMBER(12, 4) NOT NULL"#),
        "{}",
        ora[0]
    );
}

#[test]
fn decimals_bind_as_exact_text_and_read_back_without_session_settings() {
    let q = DataQuery {
        filters: vec![Filter::range("rate", Some("0.125"), Some("2,5"))],
        ..DataQuery::default()
    };
    let sql = build::plan_find::<Measure>(&Postgres, &q, 10)
        .unwrap()
        .main;
    assert!(
        sql.text
            .contains(r#"CAST("rate" AS TEXT) AS c1"#),
        "{}",
        sql.text
    );
    assert!(
        sql.text
            .contains(r#""rate" >= CAST($1 AS numeric) AND "rate" <= CAST($2 AS numeric)"#),
        "{}",
        sql.text
    );
    assert_eq!(
        sql.args[..2],
        [Arg::Text("125E-3".into()), Arg::Text("25E-1".into())]
    );
    let ora = build::plan_find::<Measure>(&V12, &q, 10)
        .unwrap()
        .main;
    assert!(
        ora.text
            .contains(r#"TO_CHAR("RATE", 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''') AS c1"#),
        "{}",
        ora.text
    );
    assert!(
        ora.text
            .contains(r#""RATE" >= TO_NUMBER(:1)"#),
        "{}",
        ora.text
    );

    let mut cells = vec![Cell::Int(1), Cell::Text("-.5".into())];
    let m: Measure = super::decode::decode_row(&mut cells).unwrap();
    assert_eq!(
        m.rate,
        crate::Decimal::parse("-0.5").unwrap(),
        "Oracle writes -.5"
    );
    let mut args = Vec::new();
    super::decode::args_of(&crate::FieldKind::Decimal, Value::Number(m.rate), &mut args);
    assert_eq!(args, vec![Arg::Text("-5E-1".into())]);
}

#[test]
fn decimal_totals_are_exact_and_averages_round_to_the_scale() {
    let q = DataQuery {
        aggregates: vec![
            AggregateRequest {
                field: "rate".into(),
                op: Aggregate::Sum,
            },
            AggregateRequest {
                field: "rate".into(),
                op: Aggregate::Avg,
            },
            AggregateRequest {
                field: "rate".into(),
                op: Aggregate::Max,
            },
        ],
        ..DataQuery::default()
    };
    let (sql, aggs) = build::plan_find::<Measure>(&Postgres, &q, 10)
        .unwrap()
        .totals
        .unwrap();
    assert_eq!(
        sql.text,
        r#"SELECT CAST(COALESCE(SUM("rate"), 0) AS TEXT) AS c0, CAST(ROUND(AVG("rate"), 4) AS TEXT) AS c1, CAST(MAX("rate") AS TEXT) AS c2 FROM "measures""#
    );
    let cells = vec![
        Cell::Text("10000000000000000.0003".into()),
        Cell::Text("0.3333".into()),
        Cell::Text("9.5000".into()),
    ];
    let totals = build::read_aggs(&MEASURE_FIELDS, &aggs, &mut cells.into_iter());
    assert_eq!(
        totals[0].value,
        Value::Number(crate::Decimal::parse("10000000000000000.0003").unwrap())
    );
    assert_eq!(
        totals[1].value,
        Value::Number(crate::Decimal::parse("0.3333").unwrap())
    );
    assert_eq!(
        totals[2].value,
        Value::Number(crate::Decimal::parse("9.5").unwrap())
    );
}

#[test]
fn decimal_cursors_keep_the_exact_value() {
    let q = DataQuery {
        sort: vec![Sort {
            field: "rate".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let keys = match build::plan_find::<Measure>(&Postgres, &q, 10)
        .unwrap()
        .shape
    {
        Shape::Rows(keys) => keys,
        _ => unreachable!(),
    };
    let row = Measure {
        id: 7,
        rate: crate::Decimal::parse("0.1000").unwrap(),
    };
    let cursor = build::row_cursor(&Postgres, &keys, &row);
    let args = build::read_cursor(&MEASURE_FIELDS, &keys, &cursor).unwrap();
    assert_eq!(args, vec![Arg::Text("1000E-4".into()), Arg::Int(7)]);
}

#[test]
fn hand_written_query_reads_fields_by_name_and_binds_the_cap_after_its_parameters() {
    let sql = build::plan_query::<Product>(
        &Postgres,
        "SELECT * FROM shop.products WHERE code % $1 ORDER BY similarity(code, $1) DESC;",
        vec![Arg::from("tea")],
        1001,
    )
    .unwrap();
    assert_eq!(
        sql.text,
        r#"SELECT CAST("id" AS BIGINT) AS c0, "code" AS c1, "note" AS c2, CAST("price" AS BIGINT) AS c3, "price_currency" AS c4, "active" AS c5, CAST("stock" AS BIGINT) AS c6, CAST("version" AS BIGINT) AS c7 FROM (SELECT * FROM shop.products WHERE code % $1 ORDER BY similarity(code, $1) DESC) zeyn_q LIMIT $2 OFFSET $3"#
    );
    assert_eq!(
        sql.args,
        vec![Arg::from("tea"), Arg::Int(1001), Arg::Int(0)]
    );
    assert_eq!(sql.cols.len(), 8);
}

#[test]
fn hand_written_query_uses_oracle_placeholders_and_rownum_on_11g() {
    let sql = build::plan_query::<Product>(
        &V11,
        "SELECT * FROM products WHERE code = :1",
        vec![Arg::from("tea")],
        2,
    )
    .unwrap();
    assert!(
        sql.text
            .starts_with("SELECT * FROM (SELECT zq.*, ROWNUM zeyn_rn FROM (SELECT \"ID\" AS c0"),
        "{}",
        sql.text
    );
    assert!(
        sql.text.contains(
            " FROM (SELECT * FROM products WHERE code = :1) zeyn_q) zq WHERE ROWNUM <= :2) WHERE zeyn_rn > :3"
        ),
        "{}",
        sql.text
    );
    let v12 = build::plan_query::<Product>(&V12, "SELECT * FROM products", Vec::new(), 2).unwrap();
    assert!(
        v12.text
            .ends_with(" zeyn_q OFFSET :1 ROWS FETCH NEXT :2 ROWS ONLY"),
        "{}",
        v12.text
    );
}

#[test]
fn over_cap_error_tells_how_to_fix_it() {
    let Error::Internal(text) = build::over_cap("Hit", 1000) else {
        panic!("not internal");
    };
    assert!(text.contains("1000") && text.contains("LIMIT") && text.contains("query_max"));
}
