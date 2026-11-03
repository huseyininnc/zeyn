//! The SQL account store: the schema for both dialects always, and a full round trip against
//! PostgreSQL when `ZEYN_TEST_PG` names a database (and Oracle when `ZEYN_TEST_ORACLE` does and the
//! `oracle` feature is on), as in `tests/postgres.rs`:
//!
//! ```text
//! docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
//! ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features accounts,postgres --test accounts_sql
//! ```

use zeyn::accounts::{
    AccountStore, Accounts, Claim, PasswordHasher, SqlAccountStore, User, UserLogin, create_tables,
};
use zeyn::prelude::*;
use zeyn::sql::{Db, PoolLimits};

#[test]
fn the_schema_follows_identity_with_neutral_names_and_a_prefix() {
    for dialect in ["postgres", "oracle"] {
        let sql = create_tables(dialect, "shop_").unwrap();
        let all = sql.join("\n");
        for table in [
            "shop_users",
            "shop_roles",
            "shop_user_roles",
            "shop_user_claims",
            "shop_role_claims",
            "shop_user_logins",
            "shop_user_tokens",
            "shop_user_passkeys",
        ] {
            assert!(
                all.contains(&format!("CREATE TABLE {table} (")),
                "{dialect} {table}"
            );
        }
        assert!(
            all.contains("UNIQUE (normalized_user_name)")
                && all.contains("UNIQUE (normalized_name)")
        );
        assert!(all.contains("ON shop_users (normalized_email)"));
        assert!(all.contains("PRIMARY KEY (login_provider, provider_key)"));
        assert!(!all.to_lowercase().contains("aspnet"));
    }
    assert!(create_tables("oracle", "z_").unwrap()[0].contains("VARCHAR2(256)"));
    assert!(
        create_tables("postgres", "Bad-Prefix").is_err(),
        "the prefix is checked"
    );
    assert!(create_tables("postgres", "x; DROP TABLE y; --").is_err());
}

fn urls(var: &str) -> Vec<String> {
    std::env::var(var)
        .map(|v| {
            v.split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn the_sql_store_round_trips_users_roles_claims_logins_tokens_and_passkeys() {
    let mut targets = urls("ZEYN_TEST_PG");
    if cfg!(feature = "oracle") {
        targets.extend(urls("ZEYN_TEST_ORACLE"));
    }
    for url in targets {
        let db = Db::connect(&url, PoolLimits::default())
            .await
            .unwrap();
        let prefix = format!("t{}_", std::process::id() % 100_000);
        let store = SqlAccountStore::new(db)
            .prefix(&prefix)
            .unwrap();
        store.create_tables().await.unwrap();
        let a = Accounts::new(
            store.clone(),
            Secret::new("an accounts key for tests only, not used anywhere real".into()),
        )
        .options(|o| {
            o.hasher = PasswordHasher {
                memory_kib: 256,
                iterations: 1,
                parallelism: 1,
            }
        });
        let mut u = a
            .users()
            .create(
                User::new("Sql@Example.com").with_email("Sql@Example.com"),
                Some("Secret-123"),
            )
            .await
            .unwrap();
        assert!(
            a.users()
                .create(User::new("sql@example.COM"), Some("Secret-123"))
                .await
                .is_err(),
            "unique normalized name"
        );
        assert!(
            a.users()
                .find_by_email("SQL@example.com")
                .await
                .unwrap()
                .is_some()
        );
        a.roles()
            .create("Admins")
            .await
            .unwrap();
        a.users()
            .add_to_role(&mut u, "admins")
            .await
            .unwrap();
        a.users()
            .add_claim(&mut u, Claim::new("permission", "x.read"))
            .await
            .unwrap();
        a.users()
            .add_login(
                &mut u,
                UserLogin {
                    provider: "kc".into(),
                    provider_key: "sub-1".into(),
                    display_name: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            a.users()
                .find_by_login("kc", "sub-1")
                .await
                .unwrap()
                .unwrap()
                .id,
            u.id
        );
        let codes = a
            .users()
            .generate_recovery_codes(&u, 10)
            .await
            .unwrap();
        assert!(
            a.users()
                .redeem_recovery_code(&u, &codes[3])
                .await
                .unwrap()
        );
        assert_eq!(
            a.users()
                .count_recovery_codes(&u)
                .await
                .unwrap(),
            9
        );
        let stale = u.concurrency_stamp.clone();
        a.users()
            .update_security_stamp(&mut u)
            .await
            .unwrap();
        let mut old = u.clone();
        old.concurrency_stamp = stale;
        assert_eq!(
            a.users().update(&mut old).await,
            Err(Error::Conflict),
            "stale saves conflict"
        );
        let (page, total) = a
            .users()
            .list(Some("sql"), 0, 10)
            .await
            .unwrap();
        assert_eq!((page.len(), total), (1, 1));
        assert_eq!(a.users().roles(&u).await.unwrap(), ["Admins"]);
        a.users().delete(&u).await.unwrap();
        assert!(
            store
                .user_by_id(&u.id)
                .await
                .unwrap()
                .is_none()
        );
    }
}
