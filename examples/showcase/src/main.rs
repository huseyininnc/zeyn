//! Northwind Operations: a tour of Zeyn in one app.
//!
//! Stock with a paged grid and an editor, today's orders in the user's time zone, an archive of a
//! million orders, a leave request form, key numbers, an API with docs, English and Turkish, a time
//! zone picker, rate limits, audit, a license and agent tools.
//!
//! Run: `cargo run --offline -p zeyn-example-showcase`, then open http://127.0.0.1:3002

mod assistant;
mod charts;
mod gallery;
mod inputs;
mod islands;
mod media;
mod overview;
mod people;
mod pro;
mod sales;
mod stock;
mod ui;
mod uploads;
mod wide;

use std::time::Duration;
use zeyn::prelude::*;
use zeyn::{Config, EnvSource, FixedLicense, I18n, License, MemoryViewStore, StaticSource};

/// The company standard every app of the organization starts with.
struct CompanyStandard {
    config: Config,
}

impl Plugin for CompanyStandard {
    fn install(self, app: App) -> App {
        let zone = self.config.get("time.zone").and_then(TimeZone::named).unwrap_or(TimeZone::UTC);
        app.config(self.config)
            .interceptor(AuditLog(StderrAudit))
            .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
            .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
            .time_policy(TimePolicy::User { default: zone })
            .time_zone_picker()
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let defaults = StaticSource(
        [
            ("time.zone", "Europe/Istanbul"),
            ("server.addr", "127.0.0.1:3002"),
            ("mail.from", "Northwind <no-reply@northwind.test>"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    );
    let config = Config::load(vec![Box::new(defaults), Box::new(EnvSource("NORTHWIND"))])
        .await
        .map_err(std::io::Error::other)?;
    let addr = config.get("server.addr").unwrap_or("127.0.0.1:3002").to_string();
    let idle = config.get("session.idle_seconds").and_then(|s| s.parse::<u64>().ok()).unwrap_or(1800);

    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .map_err(std::io::Error::other)?;
    let license = License {
        holder: "Northwind".into(),
        valid_until: Date::new(2027, 12, 31).unwrap_or_default(),
        modules: Default::default(),
        max_users: None,
    };

    let assistant = assistant::assistant(&config)?;
    let diagrams = islands::DiagramStore::default();
    let mailer = zeyn::Mailer::from_config(&config).map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    #[cfg(feature = "pdf")]
    let pdf = zeyn::Pdf::from_config(&config).map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    let mut app = App::new(t!("app.name"))
        .plugin(CompanyStandard { config: config.clone() })
        .theme(Theme::new().radius(6))
        .themes([
            Theme::light(),
            Theme::dark(),
            Theme::named(
                "sand",
                [
                    ("--z-brand", "#8a4b14"),
                    ("--z-bg", "#f3eee4"),
                    ("--z-surface", "#fffdf8"),
                    ("--z-surface-2", "#f8f3ea"),
                    ("--z-hover", "#efe7d8"),
                    ("--z-border", "#e0d6c3"),
                ],
            )
            .label(t!("theme.sand")),
        ])
        .idle_timeout(Duration::from_secs(idle))
        .idle_warning(Duration::from_secs((idle / 2).min(60)))
        .i18n(i18n)
        .service(stock::ProductStore::seeded())
        .service(islands::CounterStore::default())
        .service(diagrams.clone())
        .realtime(islands::realtime(diagrams))
        .mailer(mailer.clone())
        .datagrid_views(MemoryViewStore::default())
        .service(people::LeaveStore::seeded(TimeZone::UTC.today(DateTime::now())))
        .license(FixedLicense(license), Duration::from_secs(300))
        .mcp(Mcp::on())
        .api_docs(ApiDocs::open())
        .home(overview::overview)
        .site_url(config.get("site.url").unwrap_or("https://northwind.example"))
        .robots_txt("User-agent: *\nAllow: /about\nAllow: /dashboard\nDisallow: /\nSitemap: https://northwind.example/sitemap.xml\n")
        .feature(overview::feature())
        .feature(stock::feature())
        .feature(sales::feature())
        .feature(people::feature())
        .feature(gallery::feature())
        .feature(ui::feature())
        .feature(media::feature())
        .feature(pro::feature())
        .feature(islands::feature())
        .feature(assistant::feature())
        .feature(uploads::feature())
        .feature(charts::feature())
        .feature(inputs::feature())
        .islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"));
    #[cfg(feature = "pdf")]
    {
        app = app.service(pdf);
    }
    if let Some(a) = assistant {
        app = app.assistant(a);
    }
    app = if config.get("accounts.enabled").is_some_and(|v| v == "1" || v == "true") {
        accounts(app, &config, mailer).await?
    } else {
        app.auth(DevAuth::superuser("Demo User")).allow_dev_auth()
    };
    app.serve(&addr).await
}

/// Turns on user accounts instead of the development sign in (`NORTHWIND_ACCOUNTS__ENABLED=1`):
/// accounts in memory, confirmation e-mail through the app's mailer, the default account screens
/// with administration, and every signed in user may use the showcase. Set
/// `NORTHWIND_ACCOUNTS__KEY` (32 bytes or more) and, for passkeys, `NORTHWIND_ACCOUNTS__PUBLIC_URL`
/// (such as `http://localhost:3002`). `NORTHWIND_ACCOUNTS__ADMIN_EMAIL` and
/// `NORTHWIND_ACCOUNTS__ADMIN_PASSWORD` add a confirmed administrator at startup.
async fn accounts(app: App, config: &Config, mailer: zeyn::Mailer) -> std::io::Result<App> {
    use zeyn::accounts::{self, Accounts, MailSender, MemoryAccountStore, User};
    let fail = |e: zeyn::Error| std::io::Error::other(format!("{e:?}"));
    let key = config.secret("accounts.key").map_err(fail)?;
    let mut accounts = Accounts::new(MemoryAccountStore::new(), key)
        .sender(MailSender(mailer))
        .options(|o| o.tokens.authenticator_issuer = "Northwind".into())
        .signed_in_permissions([
            "people.leave.import",
            "people.leave.request",
            "people.org.read",
            "sales.order.read",
            "showcase.islands.read",
            "showcase.islands.write",
            "showcase.media.play",
            "stock.product.read",
            "stock.product.write",
        ])
        .role("admins", [accounts::USERS_READ, accounts::USERS_WRITE, accounts::ROLES_WRITE]);
    if let Some(url) = config.get("accounts.public_url") {
        accounts = accounts.public_url(url);
    }
    if let (Some(email), Ok(password)) = (config.get("accounts.admin_email"), config.secret("accounts.admin_password"))
    {
        let mut admin = User::new(email).with_email(email);
        admin.email_confirmed = true;
        let mut admin = accounts.users().create(admin, Some(password.expose())).await.map_err(fail)?;
        accounts.roles().create("admins").await.map_err(fail)?;
        accounts.users().add_to_role(&mut admin, "admins").await.map_err(fail)?;
    }
    Ok(app.accounts(accounts).feature(accounts::ui().admin(true).build()))
}
