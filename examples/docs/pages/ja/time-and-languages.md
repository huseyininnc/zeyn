# 時刻と言語

日付と時刻は、業務アプリでバグの原因になりやすいものです。ユーザーが移動すると 1 日ずれる誕生日、時計の切り替えの後に 1 時間遅れて始まるシフトなどです。Zeyn は種類ごとに専用の型を用意しているため、コンパイラがそれらを区別します。

## 4 つの型

| 型 | 意味 | 例 |
|---|---|---|
| `Date` | ゾーンのない暦の日 | 誕生日、請求書の日付 |
| `Time` | ゾーンのない壁時計の時刻 | 営業時間 |
| `DateTime` | UTC で保存される正確な瞬間 | レコードが作成された時刻 |
| `LocalDateTime` | ある場所での壁時計の日付と時刻 | イスタンブールの 14:00 の予約 |

暦の日を瞬間として保存してはいけません。`Date` はどのゾーンでも同じ日です。`DateTime` はどこでも同じ瞬間で、ゾーンごとに異なる壁時計の時刻で表示されます。

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

`DateTime::from_local` は、春の時計の切り替え時のように、そのゾーンに存在しない壁時計の時刻に対しては、推測せずに失敗します。

## クロック

システムクロックではなく、`ctx.now()` と `ctx.today()` を使ってください。`ctx.today()` は、ユーザーのゾーンでの今日です。テストでは `App::fixed_now(moment)` により、すべてのリクエストでクロックを固定します。

## タイムゾーン

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| ポリシー | 瞬間が表示されるゾーン |
|---|---|
| `TimePolicy::User { default }` | ユーザーのブラウザが報告するゾーン。報告があるまでは `default` |
| `TimePolicy::Organization(zone)` | 全員に共通の 1 つのゾーン |

`.time_zone_picker()` はメニューにピッカーを追加し、ユーザーが手動でゾーンを選べるようにします。シングルサインオンのプロキシが、ヘッダー（`ForwardAuth::time_zone`）でゾーンを送ることもできます。`DateTime` 列に対するグリッドのフィルターは、ユーザーのゾーンで解釈されます。

## ピッカー

日付、時刻、日時のフィールドには、キーボード操作に適したピッカーが付きます。`DateOptions` は、選べる範囲を制限します。

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## 言語の選択

サーバーは、`zeyn_locale` Cookie、次にブラウザの `Accept-Language`、最後にアプリの既定値から言語を選びます。`<html lang>` は常にそれに一致します。メニューの言語ピッカーは Cookie を設定し、再読み込みせずにその場でページを差し替えます。

## 数値、金額、日付

値は、サーバー上でユーザーの言語向けに書式化されます。トルコ語では `1.234,56`、英語では `1,234.56`、日付は `29.03.2026` または `3/29/2026`、金額は通貨記号付きです。入力欄も、同じ書式を読み戻します。

`Money` は整数の最小単位と通貨を保持するため、合計で 1 セントも失われることはありません。

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
