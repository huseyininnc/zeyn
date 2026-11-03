# 时间与语言

日期和时间是业务应用中常见的错误来源：用户出差时生日偏移了一天，时钟调整后班次晚了一小时开始。Zeyn 为每种值提供各自的类型，因此编译器能将它们区分开来。

## 四种类型

| 类型 | 含义 | 示例 |
|---|---|---|
| `Date` | 一个日历日，无时区 | 生日、发票日期 |
| `Time` | 一个挂钟时间，无时区 | 营业时间 |
| `DateTime` | 一个精确的时刻，以 UTC 存储 | 记录的创建时间 |
| `LocalDateTime` | 某地的挂钟日期和时间 | 伊斯坦布尔 14:00 的预约 |

切勿将日历日存储为时刻。`Date` 在每个时区都是同一天；`DateTime` 在各处是同一时刻，在每个时区显示不同的挂钟时间。

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

对于在该时区中不存在的挂钟时间（例如春季时钟调整期间），`DateTime::from_local` 会失败，而不是去猜测。

## 时钟

请使用 `ctx.now()` 和 `ctx.today()`，不要使用系统时钟。`ctx.today()` 是用户所在时区的今天。测试通过 `App::fixed_now(moment)` 为每个请求固定时钟。

## 时区

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| 策略 | 时刻显示所用的时区 |
|---|---|
| `TimePolicy::User { default }` | 用户浏览器上报的时区，在其上报之前使用 `default` |
| `TimePolicy::Organization(zone)` | 所有人使用同一个时区 |

`.time_zone_picker()` 会在菜单中添加一个选择器，让用户可以手动选择时区。单点登录代理也可以通过头信息发送时区（`ForwardAuth::time_zone`）。`DateTime` 列上的表格筛选按用户的时区解读。

## 选择器

日期、时间和日期时间字段提供对键盘友好的选择器。`DateOptions` 限制可选择的内容：

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## 选择语言

服务器依次从 `zeyn_locale` Cookie、浏览器的 `Accept-Language`、应用默认值中选取语言。`<html lang>` 始终与之一致。菜单中的语言选择器会设置该 Cookie，并就地替换页面，无需重新加载。

使用 `App::locale_paths()` 时，语言改为 URL 的一部分：默认语言保持 `/orders`，其他语言使用 `/tr/orders` 或 `/ar/orders`。只由路径决定语言，因此分享的链接会以相同语言打开；每个 screen 仍然只有一个路由，并通过 `ctx.locale()` 读取语言。菜单链接会保留语言，语言选择器会在不重新加载的情况下以另一种语言打开同一页面。阿拉伯语和其他从右到左书写的语言会得到 `<html dir="rtl">`。用 `Screen::alternate("tr", "/tr/orders")` 向搜索引擎列出公共页面的各个语言版本。

## 数字、金额与日期

数值在服务器上按用户的语言格式化：土耳其语为 `1.234,56`，英语为 `1,234.56`，日期为 `29.03.2026` 或 `3/29/2026`，金额带有其币种符号。输入框也按相同的格式读回。

`Money` 保存整数的最小单位和币种，因此求和永远不会丢失一分钱：

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
