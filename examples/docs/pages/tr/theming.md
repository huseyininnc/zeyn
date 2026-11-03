# Tema

Durum: `crates/zeyn/assets/zeyn.css` içinde uygulandı.

Varsayılan UI kit'i her rengi, yazı tipini, boyutu ve köşe yarıçapını `:root` üzerindeki CSS özel özelliklerinden (custom properties) okur. Bir kurum, bunlardan birkaçını geçersiz kılarak görünümü değiştirir. `zeyn.css` dosyasını düzenlemesi ya da seçici (selector) yazması gerekmez.

## 1. Bir tema nasıl uygulanır

Sunucu önce `zeyn.css` dosyasını, ardından uygulamada varsa token geçersiz kılmalarını içeren ek bir stil sayfasını sunar. Geçersiz kılma dosyası yalnızca `:root` blokları içerir:

```css
:root {
  --z-brand: #7a2448;
  --z-font: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --z-radius: 8px;
  --z-density: 0.75;
}
```

Geçersiz kılma dosyası için kurallar:

- Uygulamanın kendi origin'inden sunulmalıdır. Content Security Policy `style-src 'self'` olduğundan satır içi `<style>` blokları ve üçüncü taraf stil sayfaları engellenir.
- Yazı tipleri kullanıcıların makinelerinde önceden kurulu olmalı ya da uygulamanın origin'inden sunulmalıdır. Kit'in kendisi hiçbir web yazı tipi yüklemez.
- `zeyn.css` dosyasından sonra yükleyin; böylece eşit seçiciler sıraya göre kazanır.
- 2. bölümdeki temel token'ları ayarlayın. 3. bölümdeki türetilmiş token'lar otomatik olarak uyum sağlar. Türetilmiş bir token'ı yalnızca hesaplanan değer markanız için işe yaramıyorsa geçersiz kılın.

### Açık, koyu ve uygulama temaları

Her uygulamanın menüde, dil seçicinin yanında bir tema seçicisi vardır. Sistem (`prefers-color-scheme` değerini izler), Açık, Koyu ve uygulamanın tanımladığı her temayı sunar:

```rust
App::new("Billing")
    .theme(Theme::new().brand("#7a2448").radius(6))
    .themes([
        Theme::light(),
        Theme::dark(),
        Theme::named("sand", [("--z-bg", "#f5f1ea"), ("--z-surface", "#fffdf8")]).label(t!("theme.sand")),
    ])
    .default_theme("system")
```

- `App::theme` kurumun görünümüdür ve her temanın altında uygulanır.
- `App::themes` kullanıcıların seçebileceklerini listeler. Verilmezse liste Açık ve Koyu'dur. Her ikisi de listedeyse Sistem sunulur. `Theme::named(...)` açık temanın üzerine kurulur; `.dark_base()` koyu temanın üzerine kurar.
- `App::default_theme`, henüz seçim yapmamış kullanıcıların temasıdır. Ayarlanmazsa `system`.
- `App::theme_picker(false)` seçiciyi gizler; herkes varsayılan temayı alır.
- Tek bir tema uygulamayı kilitler: `.themes([Theme::named("acme", tokens)])` ya da `.theme(Theme::dark())`.

Token'lar uygulama başlarken denetlenir. `App::check` (dolayısıyla `router` ve `serve`) bilinmeyen token adlarını, bildirimlerinden taşabilecek değerleri (`;`, süslü parantezler, `\`, `<`, `>`, `url(`), hatalı tema adlarını, bilinmeyen bir varsayılan temayı ve bir temanın WCAG 2.2 AA'nın altına ittiği 5. bölümdeki her renk çiftini raporlar. Kontrast yalnızca `#rgb` ve `#rrggbb` değerleri için denetlenebilir; diğer renk sözdizimine güvenilir.

Tüm temalar tek bir stil sayfasında, `/_zeyn/theme.css` içinde sunulur; bu sayfa başlangıçta bir kez oluşturulur ve kit'in stil sayfalarından sonra yüklenir. Seçim `zeyn_theme` cookie'sinde saklanır; görünüm deposu olan bir uygulamada (`App::datagrid_views`) oturum açmış kullanıcı için ayrıca bu depoda saklanır, böylece kullanıcıyı yeni bir cihaza kadar izler. Sunucu temayı `<html>` üzerinde işler; bu yüzden ilk boyama zaten doğrudur ve satır içi betiğe gerek kalmaz:

| İşaretleme | Sonuç |
|---|---|
| `<html>` | Sistem: `prefers-color-scheme` değerini izler |
| `<html data-theme="light">` | Her zaman açık |
| `<html data-theme="dark">` | Her zaman koyu |
| `<html data-theme="light" data-zeyn-theme="sand">` | Açık tema üzerinde `sand` uygulama teması |

Seçicide temayı değiştirmek yalnızca bu öznitelikleri ve `color-scheme` meta etiketini değiştirir, sonra `POST /_zeyn/theme` çağrısını yapar. Hiçbir şey yeniden yüklenmez.

Koyu temaya elle yazılmış bir geçersiz kılma dosyasında kendi değerlerini vermek için kit'in kullandığı seçicileri tekrarlayın:

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### Yoğunluk

`--z-density` boşluk birimini ölçekler. `1` normal, `0.75` kompakttır. `<html data-density="compact">` bir stil sayfası olmadan `0.75` ayarlar. Kontroller ve grid satırları hiçbir zaman 24 px'in altına inmez; bu yüzden kompakt kipte de WCAG 2.2 hedef boyutu karşılanır.

Sanal (virtual) bir grid'in satır yüksekliği sunucu tarafından sabitlenir (`Paging::virtual_rows(px)`, varsayılan 32 px), çünkü çalışma zamanı kaydırma konumlarını bundan hesaplar. Yoğunluk bunu değiştirmez.

## 2. Temel token'lar

Bunlar, bir kurumun ayarlaması beklenen token'lardır.

| Token | Amaç | Açık | Koyu |
|---|---|---|---|
| `--z-brand` | Tek marka rengi. Birincil düğmeler, uygulama adı bandı, geçerli menü öğesi, odaklanmış satır, odak halkası | `#0f6674` | aynı değer; bkz. `--z-accent` |
| `--z-on-brand` | Dolu bir `--z-brand` yüzeyi üzerindeki metin | `#ffffff` | `#ffffff` |
| `--z-font` | Arayüz yazı tipi yığını | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | aynı |
| `--z-font-mono` | Kod, API örnekleri | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | aynı |
| `--z-font-size` | Gövde metni | `0.875rem` (14 px) | aynı |
| `--z-font-size-sm` | Grid hücreleri, etiketler, filtreler | `0.8125rem` (13 px) | aynı |
| `--z-font-size-xs` | Sütun başlıkları, menü grup başlıkları, sayfa bilgisi | `0.75rem` (12 px) | aynı |
| `--z-radius` | Temel köşe yarıçapı, 0 ile 12 px arası. Her bileşen bundan ölçeklenir | `4px` | aynı |
| `--z-density` | Boşluk çarpanı, `1` normal, `0.75` kompakt | `1` | aynı |
| `--z-bg` | Panellerin arkasındaki sayfa arka planı | `#f4f5f6` | `#121416` |
| `--z-surface` | Paneller, grid'ler, pencereler, girdiler | `#ffffff` | `#1a1d20` |
| `--z-surface-2` | Grid başlığı, zebra satırlar, pencere alt bilgisi, salt okunur girdiler | `#f7f8f9` | `#202326` |
| `--z-hover` | Satırlar, menü öğeleri ve düğmeler için üzerine gelme arka planı | `#eceef1` | `#2a2e33` |
| `--z-border` | Ayırıcılar ve panel dış çizgileri | `#dcdfe3` | `#33383e` |
| `--z-border-strong` | Girdi ve düğme dış çizgileri, başlık çizgisi. `--z-surface` karşısında 3:1'e ulaşmalıdır | `#7d848d` | `#7a818a` |
| `--z-text` | Gövde metni ve değerler | `#1a1d21` | `#e4e6e9` |
| `--z-text-muted` | Etiketler, başlıklar, ikincil metin. `--z-surface-2` karşısında 4,5:1'e ulaşmalıdır | `#5b626b` | `#a2a9b1` |
| `--z-danger` | Hatalar, zorunlu işaretleri, tehlike satırları | `#b3261e` | `#f28b82` |
| `--z-warning` | Uyarı satırları | `#8a5300` | `#e6b45a` |
| `--z-success` | Başarı satırları | `#1e6b3a` | `#6fcf8f` |
| `--z-info` | Bilgi bildirimleri | `#2b5d8a` | `#8ab8e6` |

## 3. Türetilmiş token'lar

Temel token'lardan `color-mix()` ve göreli renk sözdizimi (relative color syntax) ile hesaplanır. Bir temel token değiştiğinde kendiliğinden güncellenir.

| Token | Amaç | Açık | Koyu |
|---|---|---|---|
| `--z-accent` | Bir yüzey üzerinde metin ya da ince çizgi olarak kullanılan marka rengi: bağlantılar, geçerli menü öğesi, odaklanmış satır çubuğu | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, sabit açık tonda marka tonu (hue). Göreli renk sözdizimi olmayan tarayıcılar `color-mix(in oklab, var(--z-brand) 45%, #fff)` alır |
| `--z-primary` | Birincil düğme dolgusu | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Birincil düğme metni | `var(--z-on-brand)` | `#121416` |
| `--z-primary-hover` | Birincil düğme üzerine gelme | marka %86, siyahla karıştırılmış | accent %84, beyazla karıştırılmış |
| `--z-primary-active` | Birincil düğme basılı | marka %74, siyahla karıştırılmış | accent %70, beyazla karıştırılmış |
| `--z-brand-subtle` | Geçerli menü öğesinin ve odaklanmış grid satırının arkasındaki ton | `--z-surface` üzerinde `--z-tint` oranında accent | aynı formül |
| `--z-focus` | Odak halkası | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Satır ve mesaj arka planları | `--z-surface` üzerinde `--z-tint` oranında durum rengi | aynı formül |
| `--z-tint` | İnce tonların gücü | `9%` | `20%` |
| `--z-space` | Boşluk birimi | `calc(4px * var(--z-density))` | aynı |
| `--z-control` | Düğme ve girdi yüksekliği | `max(24px, 8 * --z-space)` (32 px) | aynı |
| `--z-row` | Sayfalı ve sonsuz grid'ler için grid satır yüksekliği | `max(24px, 8 * --z-space)` (32 px) | aynı |
| `--z-gap` | Screen bölümleri arasındaki boşluk | `3 * --z-space` (12 px) | aynı |
| `--z-radius-sm`, `--z-radius-lg` | Küçük parçalar (filtreler, kod), pencereler | `--z-radius` değerinin `0.75x` ve `1.5x` katı | aynı |
| `--z-shadow-2` | Yalnızca pencereler ve bildirimler. Grid satırları asla gölge almaz | yumuşak iki katmanlı gölge | daha koyu |
| `--z-backdrop` | Kipli (modal) pencerenin arkası | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | Okuma yönünün işareti; mantıksal karşılığı olmayan kaymalar için (başlangıç ya da bitiş tarafını işaretleyen iç gölgeler, kaydırma dönüşümleri): `calc(2px * var(--z-dir))` | `1`, `dir="rtl"` altında `-1` | aynı |

Sunucu işaretlemesinin ayarladığı öğe başına token'lar: `--z-width` (pencere genişliği), `--z-flex` (sütun payı), `--z-row-height` (sanal grid satır yüksekliği) ve bileşen kütüphanesi için `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` ile `--z-pop-x` / `--z-pop-y` (açılır pencere konumu).

Grafik seri renkleri `components.css` içinde `--z-chart-1` ile `--z-chart-6` arasındadır; koyu temada daha açık değerler kullanılır. Her biri `--z-surface` karşısında 3:1'e ulaşır. Herhangi bir temel token gibi geçersiz kılın.

## 4. Marka rengi seçmek

- `--z-brand`, açık temada beyaz metin taşır (düğmeler, uygulama adı bandı). `--z-on-brand` karşısında en az 4,5:1 olan bir renk seçin. Orta ile koyu arası çoğu marka rengi geçer. Sarı gibi açık bir marka rengi için `--z-on-brand` değerini de koyu bir renge ayarlayın.
- Koyu temada kit, metin, halkalar ve birincil düğmeler için markayı açar, uygulama adı bandı için dolu markayı korur. Farklı bir marka, ikinci bir renk gerekmeden iki temada da bilinçli görünür.
- Durum renklerini markadan ayrı tutun. Kırmızı hata satırlarının yanındaki kırmızı bir marka, hataları fark etmeyi zorlaştırır; bu durumda `--z-danger` değerini farklı bir kırmızıya kaydırın ya da `--z-brand-subtle` değerini geçersiz kılarak markayı grid'lerden uzak tutun.
- Renklerinizi 5. bölümdeki tabloyu hedef alarak denetleyin. Eşiğin altındaki her çift WCAG 2.2 AA'yı karşılamaz.

## 5. Doğrulanmış kontrast

Çalışan örnek uygulamada Chromium'da ölçüldü: her token işlenmiş sRGB değerine çözüldü (`color-mix()` ve göreli renkler dahil), sonra WCAG 2.x kontrast oranı hesaplandı. Eşikler: metin için 4,5:1, UI bileşen dış çizgileri ve odak göstergeleri için 3:1.

| Çift | Açık | Koyu | Gereken |
|---|---|---|---|
| `--z-text` üzerinde `--z-bg` | 15.49 | 14.77 | 4.5 |
| `--z-text` üzerinde `--z-surface` | 16.91 | 13.54 | 4.5 |
| `--z-text` üzerinde `--z-surface-2` | 15.91 | 12.63 | 4.5 |
| `--z-text` üzerinde `--z-hover` | 14.55 | 10.93 | 4.5 |
| `--z-text` üzerinde `--z-brand-subtle` (odaklanmış satır) | 14.79 | 9.27 | 4.5 |
| `--z-text` üzerinde `--z-danger-subtle` | 14.73 | 9.76 | 4.5 |
| `--z-text` üzerinde `--z-warning-subtle` | 14.82 | 9.31 | 4.5 |
| `--z-text` üzerinde `--z-success-subtle` | 14.76 | 9.33 | 4.5 |
| `--z-text-muted` üzerinde `--z-bg` | 5.65 | 7.78 | 4.5 |
| `--z-text-muted` üzerinde `--z-surface` | 6.17 | 7.13 | 4.5 |
| `--z-text-muted` üzerinde `--z-surface-2` (başlıklar) | 5.80 | 6.65 | 4.5 |
| `--z-text-muted` üzerinde `--z-hover` | 5.31 | 5.76 | 4.5 |
| `--z-text-muted` üzerinde `--z-danger-subtle` (soluk satır) | 5.37 | 5.14 | 4.5 |
| `--z-text-muted` üzerinde `--z-warning-subtle` | 5.41 | 4.90 | 4.5 |
| `--z-accent` üzerinde `--z-surface` (bağlantılar) | 6.62 | 9.33 | 4.5 |
| `--z-accent` üzerinde `--z-bg` | 6.06 | 10.18 | 4.5 |
| `--z-accent` üzerinde `--z-brand-subtle` (geçerli menü öğesi) | 5.79 | 6.39 | 4.5 |
| `--z-on-primary` üzerinde `--z-primary` | 6.62 | 10.18 | 4.5 |
| `--z-on-primary` üzerinde `--z-primary-hover` | 8.82 | 11.26 | 4.5 |
| `--z-on-primary` üzerinde `--z-primary-active` | 11.12 | 12.26 | 4.5 |
| `--z-on-brand` üzerinde `--z-brand` (uygulama adı bandı) | 6.62 | 6.62 | 4.5 |
| `--z-danger` üzerinde `--z-surface` (alan hataları) | 6.54 | 7.09 | 4.5 |
| `--z-danger` üzerinde `--z-danger-subtle` | 5.69 | 5.11 | 4.5 |
| `--z-warning` üzerinde `--z-surface` | 6.33 | 8.90 | 4.5 |
| `--z-warning` üzerinde `--z-warning-subtle` | 5.55 | 6.12 | 4.5 |
| `--z-success` üzerinde `--z-surface` | 6.52 | 8.87 | 4.5 |
| `--z-info` üzerinde `--z-surface` | 6.92 | 8.13 | 4.5 |
| `--z-focus` üzerinde `--z-surface` | 6.62 | 9.33 | 3 |
| `--z-focus` üzerinde `--z-bg` | 6.06 | 10.18 | 3 |
| `--z-focus` üzerinde `--z-brand-subtle` | 5.79 | 6.39 | 3 |
| `--z-border-strong` üzerinde `--z-surface` (girdi dış çizgisi) | 3.78 | 4.30 | 3 |
| `--z-border-strong` üzerinde `--z-surface-2` | 3.55 | 4.01 | 3 |

`--z-border` süslemedir (zaten metni olan öğeler arasındaki ayırıcılar) ve 3:1 şartına tabi tutulmaz.

## 6. Diğer erişilebilirlik davranışları

- Her etkileşimli öğe, `--z-focus` renginde 2 px'lik bir `:focus-visible` halkası gösterir. Grid satırları halkayı satırın içine çizer; böylece kaydırma kapsayıcısı tarafından asla kesilmez.
- Düğmeler, girdiler, sıralama başlıkları, filtre kutuları, sayfalama düğmeleri ve pencere kapatma düğmesi her yoğunlukta en az 24 x 24 px'tir.
- `prefers-reduced-motion: reduce`, bildirim giriş animasyonunu ve tüm geçişleri kaldırır.
- `forced-colors` kipinde simgeler `CanvasText`, birincil düğme ve odaklanmış satır `Highlight` kullanır; arka plan tonlarına dayanan durum (geçerli menü öğesi, satır durumu) bunun yerine bir dış çizgi ya da kenarlık alır.
