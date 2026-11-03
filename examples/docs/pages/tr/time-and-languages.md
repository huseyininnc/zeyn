# Zaman ve diller

Tarih ve saatler iş uygulamalarında yaygın bir hata kaynağıdır: kullanıcı seyahat edince bir gün kayan doğum günü, saat değişikliğinden sonra bir saat geç başlayan vardiya. Zeyn her değer türüne kendi türünü verir; böylece derleyici bunları birbirinden ayırır.

## Dört tür

| Tür | Nedir | Örnek |
|---|---|---|
| `Date` | Bir takvim günü, dilimsiz | Bir doğum günü, bir fatura tarihi |
| `Time` | Bir duvar saati zamanı, dilimsiz | Çalışma saatleri |
| `DateTime` | Kesin bir an, UTC olarak saklanır | Bir kaydın oluşturulduğu zaman |
| `LocalDateTime` | Bir yerdeki duvar saati tarih ve zamanı | İstanbul'da saat 14:00'te bir randevu |

Bir takvim gününü asla bir an olarak saklamayın. `Date` her dilimde aynı gündür; `DateTime` her yerde aynı andır ve her dilimde farklı bir duvar saati gösterir.

```rust
let day = Date::new(2026, 3, 29).unwrap_or_default();
let at = Time::new(9, 30, 0).unwrap_or_default();
let slot = LocalDateTime::new(day, at);
let zone = TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC);
let moment = DateTime::from_local(slot, zone)?;
let back = moment.local(zone);
```

`DateTime::from_local`, dilimde bulunmayan bir duvar saati için (ilkbahardaki saat değişikliği sırasında olduğu gibi) tahmin yürütmek yerine hata verir.

## Saat

Sistem saatini değil, `ctx.now()` ve `ctx.today()` kullanın. `ctx.today()` kullanıcının diliminde bugündür. Testler, her istek için saati `App::fixed_now(moment)` ile sabitler.

## Saat dilimleri

```rust
App::new(t!("app.name"))
    .time_policy(TimePolicy::User { default: TimeZone::named("Europe/Istanbul").unwrap_or(TimeZone::UTC) })
    .time_zone_picker()
```

| Politika | Anlar şu dilimde gösterilir |
|---|---|
| `TimePolicy::User { default }` | Kullanıcının tarayıcısının bildirdiği dilim; bildirene kadar `default` |
| `TimePolicy::Organization(zone)` | Herkes için tek bir dilim |

`.time_zone_picker()` menüye bir seçici ekler; böylece kullanıcılar dilimi elle seçebilir. Bir single sign on proxy'si de dilimi bir başlıkta gönderebilir (`ForwardAuth::time_zone`). `DateTime` sütunlarındaki grid filtreleri kullanıcının diliminde okunur.

## Seçiciler

Tarih, saat ve tarih saat alanları klavye dostu seçiciler alır. `DateOptions` seçilebilecekleri sınırlar:

```rust
Form::of(visit)
    .row(|r| r.field(Visit::day).field(Visit::arrives))
    .range(Visit::stay_from, Visit::stay_to)
    .date_options(Visit::day, DateOptions::new().no_weekends().week_numbers())
    .date_options(Visit::arrives, DateOptions::new().step_minutes(15))
```

@live dates

## Dil seçimi

Sunucu dili önce `zeyn_locale` çerezinden, sonra tarayıcının `Accept-Language` başlığından, sonra uygulama varsayılanından seçer. `<html lang>` her zaman eşleşir. Menüdeki dil seçici çerezi ayarlar ve sayfayı yeniden yüklemeden yerinde değiştirir.

## Sayılar, para ve tarihler

Değerler sunucuda kullanıcının diline göre biçimlendirilir: Türkçede `1.234,56`, İngilizcede `1,234.56`, tarihler `29.03.2026` veya `3/29/2026` olarak ve para para birimi simgesiyle. Girdiler aynı biçimleri geri okur.

`Money` tam sayı küçük birimleri ve bir para birimini tutar; böylece toplamlar bir kuruş bile kaybetmez:

```rust
let price = Money::tl(2_500_000);
let text = price.format("tr");
```
