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

İki yoğunluk vardır ve token'larla değiştirilir. Rahat (comfortable) varsayılandır: 40 px grid satırları, 36 px başlıklar, 36 px kontroller, 20 px kart iç boşluğu. Kompakt (`<html data-density="compact">`) 32 px satırlar, başlıklar ve kontroller kullanır ve boşluk birimini 0,8 ile ölçekler. Kullanıcılar bunu üst çubuğun hesap menüsünden seçer (`App::header(|h| h.account())`); seçim `zeyn_density` çerezinde tutulur ve sunucu onu `<html>` üzerine işler, böylece ilk boyama doğru olur. Kontroller ve grid satırları hiçbir zaman 24 px'in altına inmez; bu yüzden kompakt kipte de WCAG 2.2 hedef boyutu karşılanır. `Theme::new().compact()` kompakt boşlukları kurumun varsayılanı yapar.

Sanal (virtual) bir grid'in satır yüksekliği sunucu tarafından sabitlenir, çünkü çalışma zamanı kaydırma konumlarını bundan hesaplar. `Paging::virtual_scroll()` yoğunluğu izler (rahatta 40 px, kompaktta 32 px); `Paging::virtual_rows(px)` kendisine verilen yüksekliği korur. Yoğunluk değişikliği sanal grid'lere bir sonraki sayfadan itibaren uygulanır.

## 2. Temel token'lar

Bunlar, bir kurumun ayarlaması beklenen token'lardır.

| Token | Amaç | Açık | Koyu |
|---|---|---|---|
| `--z-brand` | Tek marka rengi. Logo işareti, birincil düğmeler, geçerli menü öğesi, odak halkası | `#0f6674` | aynı değer; bkz. `--z-accent` |
| `--z-on-brand` | Dolu bir `--z-brand` yüzeyi üzerindeki metin | `#ffffff` | `#ffffff` |
| `--z-font` | Arayüz yazı tipi yığını | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | aynı |
| `--z-font-mono` | Kod, API örnekleri | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | aynı |
| `--z-font-size` | Gövde metni | `0.875rem` (14 px) | aynı |
| `--z-font-size-sm` | Grid hücreleri, etiketler, filtreler | `0.8125rem` (13 px) | aynı |
| `--z-font-size-xs` | Sütun başlıkları, menü grup başlıkları, sayfa bilgisi | `0.75rem` (12 px) | aynı |
| `--z-radius` | Temel köşe yarıçapı, 0 ile 12 px arası. Düğmeler, girdiler, menü öğeleri; diğer her yarıçap bundan türetilir | `6px` | aynı |
| `--z-density` | Boşluk çarpanı, `1` normal, `0.75` kompakt | `1` | aynı |
| `--z-bg` | Panellerin arkasındaki sayfa arka planı | `#f5f6f8` | `#0d0f12` |
| `--z-surface` | Paneller, grid'ler, kartlar, girdiler | `#ffffff` | `#15181d` |
| `--z-surface-2` | Grid başlığı ve alt bilgisi, pencere alt bilgisi, salt okunur girdiler | `#f8f9fa` | `#1a1e24` |
| `--z-raised` | Açılır pencereler, menüler, diyaloglar, bildirimler, komut paleti. Koyu temada yükseltilmiş katmanlar gölgelere dayanmak yerine daha açık olur | `#ffffff` | `#1f242b` |
| `--z-hover` | Satırlar, menü öğeleri ve düğmeler için üzerine gelme arka planı | `#eff1f4` | `#232830` |
| `--z-border` | Ayırıcılar ve panel dış çizgileri | `#e2e5e9` | `#2b3139` |
| `--z-border-strong` | Girdi dış çizgileri, onay kutusu kenarlıkları, grafik taban çizgileri. `--z-surface` karşısında 3:1'e ulaşmalıdır | `#858d97` | `#6f7884` |
| `--z-text` | Gövde metni ve değerler | `#15181d` | `#e7eaee` |
| `--z-text-muted` | Etiketler, başlıklar, ikincil metin. `--z-surface-2` karşısında 4,5:1'e ulaşmalıdır | `#5a626c` | `#9ba4af` |
| `--z-danger` | Hatalar, zorunlu işaretleri, tehlike satırları | `#b42318` | `#f28b82` |
| `--z-warning` | Uyarı satırları | `#8a5300` | `#e6b45a` |
| `--z-success` | Başarı satırları | `#1a6b3c` | `#6fcf8f` |
| `--z-info` | Bilgi bildirimleri | `#275d93` | `#8ab8e6` |

## 3. Türetilmiş token'lar

Temel token'lardan `color-mix()` ve göreli renk sözdizimi (relative color syntax) ile hesaplanır. Bir temel token değiştiğinde kendiliğinden güncellenir.

| Token | Amaç | Açık | Koyu |
|---|---|---|---|
| `--z-accent` | Bir yüzey üzerinde metin ya da ince çizgi olarak kullanılan marka rengi: bağlantılar, geçerli menü öğesi, odaklanmış satır çubuğu | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, sabit açık tonda marka tonu (hue). Göreli renk sözdizimi olmayan tarayıcılar `color-mix(in oklab, var(--z-brand) 45%, #fff)` alır |
| `--z-primary` | Birincil düğme dolgusu | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Birincil düğme metni | `var(--z-on-brand)` | `#0d0f12` |
| `--z-primary-hover` | Birincil düğme üzerine gelme | marka %86, siyahla karıştırılmış | accent %84, beyazla karıştırılmış |
| `--z-primary-active` | Birincil düğme basılı | marka %74, siyahla karıştırılmış | accent %70, beyazla karıştırılmış |
| `--z-brand-subtle` | Geçerli menü öğesinin ve odaklanmış grid satırının arkasındaki ton | `--z-surface` üzerinde `--z-tint` oranında accent | aynı formül |
| `--z-focus` | Odak halkası | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Satır ve mesaj arka planları | `--z-surface` üzerinde `--z-tint` oranında durum rengi | aynı formül |
| `--z-tint` | İnce tonların gücü | `9%` | `16%` |
| `--z-text-2xs` ile `--z-text-num-lg` arası, `--z-leading-*` ile | Yazı ölçeği, boyut / satır yüksekliği: `2xs` 11/16 (tuşlar, telefonlarda grafik eksen etiketleri), `xs` 12/16 (grid başlıkları, açıklamalar, rozetler), `sm` 13/20 (hücreler, gezinme, etiketler, düğmeler), `md` 14/20 (gövde, girdiler), `lg` 16/24 (kart, grafik ve diyalog bölüm başlıkları), `xl` 20/28 (pencere başlıkları, telefonlarda sayfa başlıkları), `2xl` 24/32 (sayfa başlığı), `num` 30/36 (KPI değerleri), `num-lg` 40/44 (öne çıkan metrik). Ağırlıklar metin için 400, etiketler, düğmeler ve geçerli öğeler için 500, yalnızca başlıklar ve sayılar için 600'dür | | aynı |
| `--z-space` | Boşluk birimi | `calc(4px * var(--z-density))` | aynı |
| `--z-control` | Düğme ve girdi yüksekliği | `max(24px, 9 * --z-space)` (36 px; kompaktta 32 px) | aynı |
| `--z-row`, `--z-row-head` | Sayfalı ve sonsuz grid'ler için grid satır ve başlık yüksekliği | 40 px ve 36 px (kompaktta 32 px) | aynı |
| `--z-gap` | Kartlar ve screen bölümleri arasındaki boşluk | `4 * --z-space` (16 px) | aynı |
| `--z-s-0_5`, `--z-s-1`, `--z-s-1_5`, `--z-s-2`, `--z-s-3`, `--z-s-4`, `--z-s-5`, `--z-s-6`, `--z-s-8`, `--z-s-10`, `--z-s-12`, `--z-s-16` | Boşluk ölçeği: sayı çarpı `--z-space` (yoğunluk 1'de 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 ve 64 px) | | aynı |
| `--z-radius-sm` | Rozetler, çipler, onay kutuları, klavye tuşları | `max(2px, --z-radius - 2px)` (4 px) | aynı |
| `--z-radius-lg` | Kartlar, grid'ler, açılır pencereler, bildirimler | `--z-radius + 2px` (8 px) | aynı |
| `--z-radius-xl` | Diyaloglar, oturum açma kartı, komut paleti | `--z-radius + 6px` (12 px) | aynı |
| `--z-radius-full` | Avatarlar, anahtarlar (switch), durum noktaları, sayı hapları | `999px` | aynı |
| `--z-shadow-1` | Varsayılan düğmeler, segmentli kontrolün tutamacı | %6 ile `0 1px 2px` | yok |
| `--z-shadow-2` | Açılır pencereler, menüler, araç ipuçları, bildirimler. Grid satırları ve kartlar asla gölge almaz | iki yumuşak katman | 1 px `--z-border` halkası ve derin bir gölge |
| `--z-shadow-3` | Diyaloglar ve komut paleti | iki daha derin katman | 1 px halka ve daha derin bir gölge |
| `--z-dur-1`, `--z-dur-2`, `--z-dur-3`, `--z-dur-out` | Hareket: üzerine gelme renkleri (80 ms), açılır pencerelerin ve bildirimlerin girişi (140 ms), diyalogların, çekmecelerin ve paletin girişi (200 ms), her çıkış (100 ms). `prefers-reduced-motion: reduce` altında 0 olurlar, solmalar en fazla 80 ms sürer | | aynı |
| `--z-ease-out`, `--z-ease-in`, `--z-ease-linear` | Girişlerin ve hareketlerin, çıkışların ve renk değişimlerinin yumuşatma eğrisi (easing) | `cubic-bezier(0.2, 0, 0, 1)`, `cubic-bezier(0.4, 0, 1, 1)`, `linear` | aynı |
| `--z-backdrop` | Kipli (modal) pencerenin arkası | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | Okuma yönünün işareti; mantıksal karşılığı olmayan kaymalar için (başlangıç ya da bitiş tarafını işaretleyen iç gölgeler, kaydırma dönüşümleri): `calc(2px * var(--z-dir))` | `1`, `dir="rtl"` altında `-1` | aynı |

Sunucu işaretlemesinin ayarladığı öğe başına token'lar: `--z-width` (pencere genişliği), `--z-flex` (sütun payı), `--z-row-height` (sanal grid satır yüksekliği) ve bileşen kütüphanesi için `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` ile `--z-pop-x` / `--z-pop-y` (açılır pencere konumu).

Grafik seri renkleri `components.css` içinde `--z-chart-1` ile `--z-chart-8` arasındadır: mavi `#2563d9`, turuncu `#d67400`, macenta `#a3369b`, yeşil `#14866d`, zeytin `#7a6a00`, mor `#6e56cf`, kırmızı `#cc3b33` ve arduvaz grisi `#56636f`; koyu temada daha açık değerler kullanılır (`#5b9bff`, `#f08a3c`, `#e27bd6`, `#38c29f`, `#ece27a`, `#9d8cff`, `#ff7b70`, `#9aa7b5`). Bu sıra ilk beşini döteranopi, protanopi ve tritanopi altında birbirinden ayrı tutar. 1. seri marka rengi değildir, böylece kırmızı ya da yeşil bir marka asla iyi ya da kötü olarak okunmaz; bir uygulama yine de `--z-chart-1: var(--z-brand)` ayarlayabilir. Başlangıç denetimi her birini `--z-surface` karşısında 3:1'e tutar. Herhangi bir temel token gibi geçersiz kılın.

## 4. Marka rengi seçmek

- `--z-brand`, açık temada beyaz metin taşır (birincil düğmeler, logonun harf işareti, oturum açma shell'inin marka paneli). `--z-on-brand` karşısında en az 4,5:1 olan bir renk seçin. Orta ile koyu arası çoğu marka rengi geçer. Sarı gibi açık bir marka rengi için `--z-on-brand` değerini de koyu bir renge ayarlayın.
- Koyu temada kit, metin, halkalar ve birincil düğmeler için markayı açar, logonun harf işareti için dolu markayı korur. Farklı bir marka, ikinci bir renk gerekmeden iki temada da bilinçli görünür.
- Durum renklerini markadan ayrı tutun. Kırmızı hata satırlarının yanındaki kırmızı bir marka, hataları fark etmeyi zorlaştırır; bu durumda `--z-danger` değerini farklı bir kırmızıya kaydırın ya da `--z-brand-subtle` değerini geçersiz kılarak markayı grid'lerden uzak tutun.
- Renklerinizi 5. bölümdeki tabloyu hedef alarak denetleyin. Eşiğin altındaki her çift WCAG 2.2 AA'yı karşılamaz.

## 5. Doğrulanmış kontrast

Aşağıdaki her çift, uygulama başlarken (`App::check`) yerleşik temalar ve her uygulama teması için ve `src/server/theme.rs` içindeki `built_in_themes_reach_wcag_aa_on_every_pair` birim testiyle denetlenir (bu tabloyu yazdırmak için testi `--nocapture` ile çalıştırın). Türetilmiş token'lar (`--z-accent`, birincil renkler ve ince tonlar) `zeyn.css` onları nasıl hesaplıyorsa öyle, Oklab içinde hesaplanır. Eşikler: metin için 4,5:1, UI bileşen dış çizgileri ve odak göstergeleri için 3:1.

| Çift | Açık | Koyu | Gereken |
|---|---|---|---|
| `--z-text` üzerinde `--z-bg` | 16.45 | 15.90 | 4.5 |
| `--z-text` üzerinde `--z-surface` | 17.79 | 14.74 | 4.5 |
| `--z-text` üzerinde `--z-surface-2` | 16.88 | 13.86 | 4.5 |
| `--z-text` üzerinde `--z-raised` | 17.79 | 12.93 | 4.5 |
| `--z-text` üzerinde `--z-hover` | 15.72 | 12.27 | 4.5 |
| `--z-text` üzerinde `--z-brand-subtle` | 15.56 | 11.18 | 4.5 |
| `--z-text` üzerinde `--z-danger-subtle` | 15.49 | 11.60 | 4.5 |
| `--z-text` üzerinde `--z-warning-subtle` | 15.59 | 11.20 | 4.5 |
| `--z-text` üzerinde `--z-success-subtle` | 15.53 | 11.23 | 4.5 |
| `--z-text` üzerinde `--z-info-subtle` | 15.52 | 11.40 | 4.5 |
| `--z-text-muted` üzerinde `--z-bg` | 5.71 | 7.61 | 4.5 |
| `--z-text-muted` üzerinde `--z-surface` | 6.18 | 7.05 | 4.5 |
| `--z-text-muted` üzerinde `--z-surface-2` | 5.86 | 6.63 | 4.5 |
| `--z-text-muted` üzerinde `--z-raised` | 6.18 | 6.19 | 4.5 |
| `--z-text-muted` üzerinde `--z-hover` | 5.46 | 5.87 | 4.5 |
| `--z-text-muted` üzerinde `--z-brand-subtle` | 5.40 | 5.35 | 4.5 |
| `--z-border-strong` üzerinde `--z-surface` | 3.36 | 3.98 | 3 |
| `--z-border-strong` üzerinde `--z-surface-2` | 3.19 | 3.74 | 3 |
| `--z-border-strong` üzerinde `--z-raised` | 3.36 | 3.49 | 3 |
| `--z-accent` üzerinde `--z-surface` | 6.62 | 9.81 | 4.5 |
| `--z-accent` üzerinde `--z-bg` | 6.12 | 10.58 | 4.5 |
| `--z-accent` üzerinde `--z-brand-subtle` | 5.79 | 7.44 | 4.5 |
| `--z-focus` üzerinde `--z-surface` | 6.62 | 9.81 | 3 |
| `--z-focus` üzerinde `--z-bg` | 6.12 | 10.58 | 3 |
| `--z-on-primary` üzerinde `--z-primary` | 6.62 | 10.58 | 4.5 |
| `--z-on-primary` üzerinde `--z-primary-hover` | 8.82 | 11.70 | 4.5 |
| `--z-on-primary` üzerinde `--z-primary-active` | 11.12 | 12.74 | 4.5 |
| `--z-danger` üzerinde `--z-surface` | 6.57 | 7.45 | 4.5 |
| `--z-danger` üzerinde `--z-danger-subtle` | 5.73 | 5.86 | 4.5 |
| `--z-warning` üzerinde `--z-surface` | 6.33 | 9.35 | 4.5 |
| `--z-warning` üzerinde `--z-warning-subtle` | 5.55 | 7.11 | 4.5 |
| `--z-success` üzerinde `--z-surface` | 6.54 | 9.32 | 4.5 |
| `--z-success` üzerinde `--z-success-subtle` | 5.70 | 7.09 | 4.5 |
| `--z-info` üzerinde `--z-surface` | 6.83 | 8.54 | 4.5 |
| `--z-info` üzerinde `--z-info-subtle` | 5.96 | 6.60 | 4.5 |
| `--z-on-brand` üzerinde `--z-brand` | 6.62 | 6.62 | 4.5 |
| `--z-brand` üzerinde `--z-surface` | 6.62 | yok | 4.5 |

`--z-border` süslemedir (zaten metni olan öğeler arasındaki ayırıcılar) ve 3:1 şartına tabi tutulmaz.

## 6. Diğer erişilebilirlik davranışları

- Her etkileşimli öğe, `--z-focus` renginde 2 px'lik bir `:focus-visible` halkası gösterir. Grid satırları halkayı satırın içine çizer; böylece kaydırma kapsayıcısı tarafından asla kesilmez.
- Düğmeler, girdiler, sıralama başlıkları, filtre kutuları, sayfalama düğmeleri ve pencere kapatma düğmesi her yoğunlukta en az 24 x 24 px'tir.
- `prefers-reduced-motion: reduce`, bildirim giriş animasyonunu ve tüm geçişleri kaldırır.
- `forced-colors` kipinde simgeler `CanvasText`, birincil düğme ve odaklanmış satır `Highlight` kullanır; arka plan tonlarına dayanan durum (geçerli menü öğesi, satır durumu) bunun yerine bir dış çizgi ya da kenarlık alır.
