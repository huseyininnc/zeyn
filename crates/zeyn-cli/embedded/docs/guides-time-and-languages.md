# Time and languages

Dates and times are a common source of bugs in business apps: a birthday that moves a day when a user travels, a shift that starts an hour late after a clock change. Zeyn gives each kind of value its own type, so the compiler keeps them apart.

## Four types

| Type | Is | Example |
|---|---|---|
| `Date` | A calendar day, no zone | A birthday, an invoice date |
| `Time` | A wall clock time, no zone | Opening hours |
| `DateTime` | An exact moment, stored in UTC | When a record was created |
| `LocalDateTime` | A wall clock date and time at a place | An appointment at 14:00 in Istanbul |

Never store a calendar day as a moment. A `Date` is the same day in every zone; a `DateTime` is the same instant everywhere and shows a different wall clock time in each zone.

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

`DateTime::from_local` fails for a wall clock time that does not exist in the zone, such as during a spring clock change, instead of guessing.

## The clock

Use `ctx.now()` and `ctx.today()`, never the system clock. `ctx.today()` is today in the user's zone. Tests fix the clock for every request with `App::fixed_now(moment)`.

## Time zones

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| Policy | Moments are shown in |
|---|---|
| `TimePolicy::User { default }` | The zone the user's browser reports, or `default` until it does |
| `TimePolicy::Organization(zone)` | One zone for everyone |

`.time_zone_picker()` adds a picker to the menu so users can choose a zone by hand. A single sign on proxy can also send the zone in a header (`ForwardAuth::time_zone`). Grid filters on `DateTime` columns are read in the user's zone.

## Pickers

Date, time and date time fields get keyboard friendly pickers. `DateOptions` limits what can be picked:

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## Choosing the language

The server picks the language from the `zeyn_locale` cookie, then the browser's `Accept-Language`, then the app default. `<html lang>` always matches. The language picker in the menu sets the cookie and swaps the page in place, without a reload.

With `App::locale_paths()` the language is part of the URL instead: the default language keeps `/orders`, the others get `/tr/orders` or `/ar/orders`. The path alone decides, so a shared link opens in the same language, and each screen still has one route and reads `ctx.locale()`. Menu links keep the language, and the picker opens the same page in the other language without a reload. Arabic and other right to left languages get `<html dir="rtl">`. List the languages of a public page for search engines with `Screen::alternate("tr", "/tr/orders")`.

## Numbers, money and dates

Values are formatted for the user's language on the server: `1.234,56` in Turkish and `1,234.56` in English, dates as `29.03.2026` or `3/29/2026`, and money with its currency symbol. Inputs read the same formats back.

`Money` keeps integer minor units and a currency, so sums never lose a cent:

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
