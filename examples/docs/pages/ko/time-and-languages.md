# 시간과 언어

날짜와 시간은 업무 앱에서 흔한 버그의 원인입니다. 사용자가 여행하면 하루가 바뀌는 생일, 시계가 바뀐 후 한 시간 늦게 시작하는 근무가 그 예입니다. Zeyn은 각 종류의 값에 고유한 타입을 부여하므로 컴파일러가 이를 구분해 줍니다.

## 네 가지 타입

| 타입 | 의미 | 예 |
|---|---|---|
| `Date` | 시간대가 없는 달력상의 하루 | 생일, 청구서 날짜 |
| `Time` | 시간대가 없는 벽시계 시각 | 영업 시간 |
| `DateTime` | UTC로 저장되는 정확한 순간 | 레코드가 생성된 시점 |
| `LocalDateTime` | 어떤 장소의 벽시계 날짜와 시각 | 이스탄불에서 14:00의 약속 |

달력상의 하루를 순간으로 저장하지 마십시오. `Date`는 모든 시간대에서 같은 날이고, `DateTime`은 어디서나 같은 순간이며 시간대마다 다른 벽시계 시각으로 표시됩니다.

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

`DateTime::from_local`은 봄철 시계 변경 중처럼 그 시간대에 존재하지 않는 벽시계 시각에 대해 추측하지 않고 실패합니다.

## 시계

시스템 시계가 아니라 `ctx.now()`와 `ctx.today()`를 사용하십시오. `ctx.today()`는 사용자의 시간대 기준 오늘입니다. 테스트는 `App::fixed_now(moment)`로 모든 요청의 시계를 고정합니다.

## 시간대

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| 정책 | 순간이 표시되는 시간대 |
|---|---|
| `TimePolicy::User { default }` | 사용자의 브라우저가 보고하는 시간대이며, 보고하기 전까지는 `default` |
| `TimePolicy::Organization(zone)` | 모두에게 하나의 시간대 |

`.time_zone_picker()`는 메뉴에 선택기를 추가하여 사용자가 직접 시간대를 고를 수 있게 합니다. 싱글 사인온 프록시가 헤더로 시간대를 보낼 수도 있습니다(`ForwardAuth::time_zone`). `DateTime` 열의 그리드 필터는 사용자의 시간대로 읽힙니다.

## 피커

날짜, 시간, 날짜 시간 필드에는 키보드 친화적인 피커가 제공됩니다. `DateOptions`는 선택할 수 있는 범위를 제한합니다.

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## 언어 선택

서버는 `zeyn_locale` 쿠키에서, 그다음 브라우저의 `Accept-Language`에서, 마지막으로 앱 기본값에서 언어를 고릅니다. `<html lang>`은 항상 이와 일치합니다. 메뉴의 언어 선택기는 쿠키를 설정하고 새로 고침 없이 페이지를 그 자리에서 교체합니다.

## 숫자, 금액, 날짜

값은 서버에서 사용자의 언어에 맞게 서식화됩니다. 터키어는 `1.234,56`, 영어는 `1,234.56`이고, 날짜는 `29.03.2026` 또는 `3/29/2026`이며, 금액은 통화 기호와 함께 표시됩니다. 입력은 같은 형식을 다시 읽어 들입니다.

`Money`는 정수 최소 단위와 통화를 유지하므로 합계에서 1센트도 잃지 않습니다.

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
