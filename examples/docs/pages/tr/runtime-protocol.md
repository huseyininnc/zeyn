# Runtime Protocol

Durum: taslak, `zeyn::server` ve `assets/zeyn/*.js` tarafından uygulanır (`/_zeyn/zeyn.js` olarak sunulur).

Bu, Zeyn sunucusu ile tarayıcı çalışma zamanı (runtime) arasındaki sözleşmedir. Ekranlar sunucuda oluşturulur. Tarayıcı runtime'ı, kullanıcı eylemlerini sunucuya gönderen ve dönen yanıtı uygulayan küçük bir betiktir. Uygulama kodu hiçbir zaman JavaScript yazmaz.

## 1. Page load

`GET {route}?params={json}` tam bir HTML belgesi döndürür. `params`, ekran parametrelerinin URL kodlu JSON halidir; ekranın parametresi yoksa atlanır.

`/orders/{id}` gibi yol segmentleri olan bir route, bu `params` alanlarını yoldan (`/orders/42`) okur, diğer alanları `params` içinden alır. Ayrıştırılamayan bir segment değeri 404 döndürür. Değerler yüzde kodludur, noktalar dahil (`%2E`); böylece bir ekran URL'si runtime'a hiçbir zaman dosya gibi görünmez. `Screen::moved_to` yanıtı veren bir ekran, yeni URL'yi `Location` içinde taşıyan `301 Moved Permanently` alır; `Screen::redirect` yanıtı veren bir ekran, `Cache-Control: no-store` ile `303 See Other` alır. `nav.js` ikisini de sayfa yüklemeden izler ve hedefin adresini gösterir, yükleme bir geçmiş girişi eklemediğinde de (geri ve ileri).

Başarısız bir sayfa kendi durum koduyla (404, 403, 500, ...) ve uygulamanın hata ekranıyla (`App::not_found`, `App::server_error`) ya da kabuğun (shell) içinde kısa, yerleşik bir iletiyle yanıtlanır. `x-zeyn-partial: 1` başlığı olan bir istek, yalnızca o ekranı `x-zeyn-partial: 1` ve `x-zeyn-title` ile alır; böylece runtime ekranı sayfa yüklemeden yerine koyar. Hata ayrıntıları tarayıcıya asla ulaşmaz.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

Menü yalnızca geçerli kullanıcının açabileceği ekranları listeler. Geçerli öğede `aria-current="page"` bulunur.

Head içindeki her framework script'i ve stil dosyası `?v=` ile build kimliğini taşır (`meta[name=zeyn-build]`; `ZEYN_BUILD_ID`'den ya da çalıştırılabilir dosyanın hash'inden gelir); `theme.css` tema metninin hash'ini taşır. Güncel değerli bir istek `Cache-Control: public, max-age=31536000, immutable` alır, bu yüzden sıcak bir sayfa yüklemesi bunların hiçbirini istemez; başka her istek (bir dağıtımdan sonra eski bir sekme, `v` yok) `no-cache` ve ETag alır. Çalışma zamanının sonradan import ettiği modüller, onları import eden script'in URL'sinden aynı `v` değerini alır.

### 1.1 Browser modules

Bir sayfa yalnızca ekranının kullandığı tarayıcı kodunu yükler ve çalıştırır. Her sayfa çekirdeği çalıştırır: `zeyn.js` (istekler, efektler, pencereler, formlar, menüler, kısayollar, olay delegasyonu ve modül yükleyici), `nav.js` ve `enterprise.js` (`fetch` üzerindeki yanıt kancaları ve oturumun boşta kalma süresi); hepsi birlikte 90 KB ham, 29 KB gzip. Geri kalan her şey bir bileşenin modülüdür: `/_zeyn/{name}.js` ve bazıları için `/_zeyn/{name}.css`. Modüller `crates/zeyn/src/server/modules.rs` içindeki `MODULES` listesinde, HTML'lerinin taşıdığı işaretlerle tanımlanır: öznitelik adları (`data-zeyn-date`) ve sınıflar (`z-select`), tam ad olarak eşlenir.

| Modül | Bileşenler |
|---|---|
| `components.js` | her kontrol modülünün import ettiği ortak bölüm: yardımcılar, açılır pencere ve liste kutusu çekirdekleri, seçim kutuları (uygulama kabuğunun seçicileri), onay pencereleri, uyarılar, içerik yolu |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js`, `menu.js` | paneller, sekmeler ve bölünmüş alanlar; tarih ve saat seçiciler (grid tarih filtreleri dahil); liste kutuları, aramalı seçimler ve çoklu seçimler; sayılar, büyüyen metin alanları, maskeler ve parolalar; satır düzenleyici; adımlı formlar; ağaçlar; ipuçları ve açılır kutular; menü ve bölünmüş düğmeler; meşgul katmanları; yan menünün katlanabilir grupları |
| `grid.js` | gridler; `zeyn.js` onu kendisi import ettiği için `<link rel="modulepreload">` olarak bağlanır |
| `palette.js`, `pro.js` | komut paleti ve komut düğmeleri; menü çubukları, araç kutuları, durum çubukları, yerleştirme panelleri ve özellik denetçileri |
| `colors.js`, `length.js`, `inspector.js` | renk seçiciler; birim seçicili uzunluklar; adaların özellik denetçileri |
| `forms.js`, `reveal.js`, `import.js` | formların taslakları ve iş akışı geçişleri; maskeli değerleri gösterme; içe aktarmalar |
| `catalog.js`, `media.js`, `files.js`, `charts.js` ve stil dosyaları | kart gridleri, raylar ve arama kutuları; oynatıcılar, gömülü içerik ve görseller; yüklemeler; grafikler, mini grafikler, KPI kartları ve bullet grafikler |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | paylaşım düğmeleri, filtre çipleri, tepkiler, hero slaytları, tercih deposu, kart önizlemeleri (ilk üzerine gelme), asistan (sohbet panelleri ya da başlatıcının ilk basışı) |
| `topics.js`, `vlist.js` | topic dinleyen node'lar ve onları süren kontroller (`data-zeyn-listens`, `data-zeyn-drives`); sanal listeler (`data-zeyn-vlist`) |
| `inbox.js`, `kanban.js`, `calendar.js` and their stylesheets, `diff.css`, `code.css`, `barcode.css` | gelen kutusu ve bildirim zili; Kanban panoları; takvimler; betik gerektirmeyen farklar, kod görünümleri ve barkodlar (kod görünümünün kopyalama düğmesi `share.js` içindedir) |
| `codeedit.js`, `codeedit.css` | kod düzenleyiciler (`data-zeyn-code`): renklendirilmiş kopya, satır numaraları, öneriler ve hata işaretleri; `/_zeyn/data` adresine yalnızca yazmada bir duraksamadan sonra istek gönderir |
| `range.js`, `quantity.js`, `gallery.js`, `countdown.js` ve `commerce.css` | aralık kaydırıcıları, adet seçiciler, ürün galerileri ve geri sayımlar; tüm mağaza bileşenlerinin stil dosyası (fiyatlar, varyant seçiciler, facet panelleri, değerlendirme özetleri) |
| `imgcompare.js` | resim karşılaştırmaları (`ImageCompare`): sürgüyü gösterir ve değerini `--z-imgcompare` değişkenine yazar |
| `thread.js` ve `thread.css`, `stream.js`, `composer.js` ve `composer.css`, `typing.js` ve `typing.css` | mesaj thread'leri; thread'ler ile asistanın paylaştığı akan Markdown ve JSON satırları; sohbet composer'ları; yazıyor göstergeleri |
| `feed.js`, `lightbox.js` and their stylesheets, `stories.css` | gönderi akışları; resim ve videoların lightbox'ı; hikâye halkaları ve sıraları, betik gerektirmez |

Tam bir sayfa, head içinde her sayfanın kullandığı stil dosyalarını (`zeyn.css`, `components.css`, `content.css`, `pro.css`, `enterprise.css`), işaretleri HTML'inde bulunan modüllerin stil dosyalarını ve onların scriptlerini, import ettikleri modüllerden sonra bağlar. `zeyn.js` aynı tabloyla (`[name, script selector, style selector]`) sunulur ve sonradan gelen içerik için eksik olanı yükler: `nav.js` bir ekranı yerine koymadan önce `{ root, waits }` ile `zeyn:need` olayını gönderir ve yeni stil dosyalarını bekler, pencereler açılmadan önce aynı şekilde bekler; `zeyn:content`, grid sayfaları ve grid filtreleri taşıdıklarının scriptlerini yükler. Tarayıcı her URL için tek bir örnek tutar, bu yüzden bir modül bir kez çalışır. Bir modül çalıştığında belgede bulduklarını kurar, ardından `zeyn:content` olayını dinler; sayfada olmayan bileşenleri asla taramaz ve onlar için dinlemez. Modüller ortak bölümü kendi URL'lerinin `?v=` değeriyle import eder. `tests/ui_modules.rs`, bir sayfa kullanmadığı bir modülü bağladığında ya da bir modül stil dosyası işaretlerinin dışındaki bir sınıfa stil verdiğinde başarısız olur.

Showcase üzerinde ölçüldü (ham, gzip ile ağda): yalnızca metin olan bir sayfa 269 KB (80 KB) yerine 132 KB (46 KB) script, bir grid sayfası 357 KB (106 KB) yerine 223 KB (74 KB), bir form sayfası 269 KB (80 KB) yerine 186 KB (65 KB) yükler; her sayfa 177 KB (44 KB) yerine 151 KB (35 KB) stil yükler. Bileşen kataloğu gibi her bileşeni taşıyan bir sayfa öncekinden daha çok dosya yükler (41 yerine 68 istek, ham 774 KB yerine 836 KB), çünkü her modül ayrı bir dosyadır.

## 2. Screen markup

Her ekran kökü kendi adını ve parametrelerini taşır:

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

Pencereler (Window) `class="z-window" role="dialog" aria-modal="true"` ve bir `--z-width` stili kullanır.

Her gövde öğesi bir düğüm (node) elemanına sarılır. Düğümün yolu (path), aşağıdaki endpoint'lerde kullanılan adrestir. Kapsayıcılar (`Row`, `Stack`, `Panel`, `Card`, `Tabs`, `Split`) her çocuğu aynı şekilde sarar ve çocuğun yolu kendi indeksini ekler: 1 numaralı gövde öğesinin ilk çocuğunun üçüncü çocuğu `"1.0.2"` olur.

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

Endpoint'ler `node` değerini noktalı bir dize olarak ya da gövde öğeleri için düz bir sayı (`0`) olarak alır. Yollar en fazla 16 basamak seviyesindedir; başka her şey `422` ile reddedilir.

## 3. Attributes the runtime reads

| Attribute | On | Meaning |
|---|---|---|
| `data-zeyn-effect` | button, `tr` | Tıklamada çalıştırılacak JSON effect (satırlarda: çift tıklamada veya Enter ile) |
| `aria-keyshortcuts` | button | Klavye kısayolu, örneğin `Control+N` |
| `data-zeyn-export` | button | Aynı ekrandaki grid'in dışa aktarma biçimi (`csv`, `xlsx`) |
| `data-zeyn-grid` | grid root | Bir grid'i işaretler; ayrıca `data-zeyn-mode`, `data-zeyn-paging`, `data-zeyn-page-size`, `data-zeyn-total`, `data-zeyn-next`, `data-zeyn-offset` taşır |
| `data-field`, `aria-sort`, `aria-colindex` | `th` | Sütun alan adı, geçerli sıralama, konum |
| `data-filter`, `data-options` | `th` | Filtre girişi türü (`text`, `number`, `money`, `date`, `datetime`, `time`, `choice`, `bool`) ve seçim seçenekleri `[{value,label}]` |
| `data-pin` | `th` | Sabitlenmiş sütunlar için `start` veya `end` (seçim ve satır eylemi sütunları da dahil) |
| `data-edit`, `data-optional` | `th` | Kaydetme yetkisi olan kullanıcılar için düzenlenebilir bir sütunun giriş türü |
| `data-zeyn-texts`, `data-zeyn-columns`, `data-zeyn-view` | grid root | Yerelleştirilmiş denetim metinleri, tüm sütunlar `[{field,label,hidden,pin,width}]`, kaydedilmiş görünüm |
| `data-zeyn-views` | grid root | Görünümler sunucuda kaydediliyorsa `server` |
| `data-zeyn-version-field`, `data-zeyn-exportable` | grid root | Satır içi düzenlemeler için satır sürüm alanı adı; grid dışa aktarılabiliyorsa `csv` |
| `data-zeyn-key`, `data-zeyn-version`, `data-zeyn-menu` | `tr` | Satır anahtarı, satır sürümü, bağlam menüsü öğeleri |
| `data-v` | `td` | Düzenlenebilir bir hücrenin giriş değeri |
| `data-zeyn-detail`, `aria-expanded` | `tr` | Satırın `{screen, params}` biçimindeki ayrıntı ekranı; açık olup olmadığı |
| `data-zeyn-expand` | button in a row | Satırın ayrıntısını, grubunu veya çocuklarını açar ya da kapatır |
| `data-zeyn-level`, `aria-level`, `data-zeyn-path` | `tr` in a `treegrid` | Gruplanmış veya ağaç grid'inde derinlik (en üstte 0); satır açılırken gönderilecek grup değerlerinin veya üst anahtarların yolu |
| `z-group` | `tr` | Bir grup başlığı satırı; `z-branch-more` bir dalın "Daha fazla yükle" satırıdır |
| `data-zeyn-run` | button in a row | Çalıştırılacak satır öğesi (`a0` ilk satır eylemidir) |
| `data-agg` | `tfoot td` | Sütunun gösterdiği toplamlar (`sum avg min max count`) |
| `data-zeyn-bulk` | div before the grid | `data-zeyn-bulk-run` düğmeleri olan seçim çubuğu |
| `data-zeyn-menu` | screen root | Ekranın bağlam menüsü |
| `data-zeyn-native-menu="off"` | `html` | Metin girişleri dışında tarayıcının bağlam menüsü kapalıdır |
| `data-zeyn-rows` | `tbody` | Satırların gittiği yer |
| `data-zeyn-page` | pager button | `prev` veya `next` |
| `data-zeyn-page-info` | span | `1-50 of 230` gibi metin; grid kökündeki yerelleştirilmiş şablonlardan oluşturulur |
| `data-zeyn-text-range`, `data-zeyn-text-range-total`, `data-zeyn-text-loaded` | grid root | Yerelleştirilmiş şablonlar: `{from}-{to}`, `{from}-{to} of {total}`, `{count} rows loaded` |
| `data-zeyn-more` | div | Sonsuz kaydırma nöbetçisi, "Daha fazla yükle" düğmesini içerir |
| `data-zeyn-submit` | form | Gönderme eyleminin adı |
| `data-zeyn-editor` | input | Özel bir alan türünün istemci tarafı düzenleyicisi |

Effect'ler:

| Effect | JSON | Runtime does |
|---|---|---|
| Open window | `{"effect":"open_window","screen":"product_form","params":{...}}` | Ekranı (bölüm 4.1) modal bir `<dialog>` içine yükler |
| Navigate | `{"effect":"navigate","screen":"...","params":{...}}` | Sayfaya yönlendiren `/_zeyn/route/{screen}?params=` adresini yükler; route'unun segmentleri varsa yol biçiminde yönlendirir |
| Toast | `{"effect":"toast","message":"Saved"}` | 4 saniye boyunca bir toast gösterir |
| Close and refresh | `{"effect":"close_and_refresh"}` | En üstteki diyaloğu kapatır ve altındaki ekrandaki her grid'i yeniden yükler |

## 4. Endpoints

Tüm `POST` istekleri JSON gönderir, `zeyn-csrf` meta etiketindeki belirteçle birlikte `x-zeyn-csrf` başlığını taşımak zorundadır; aksi halde `403` ile reddedilir.

Oturumsuz açılan herkese açık bir ekran, önbelleğe alınabilmesi için boş bir `zeyn-csrf` meta etiketi ile ve `Set-Cookie` olmadan gelir. İlk `POST`'tan önce runtime `GET /_zeyn/csrf` çağrısını yapar; bu çağrı `Cache-Control: no-store` ile `{ "token": "..." }` yanıtını verir, tarayıcıda yoksa `zeyn_csrf` çerezini ayarlar ve `Sec-Fetch-Site` değeri `same-origin` olmayan istekleri reddeder. Belirteç meta etiketine yazılır; çağrı başarısız olursa `POST` belirteçsiz gider ve diğerleri gibi reddedilir. Hiçbir şey sessizce yeniden denenmez.

Uygulama içi gezinme (`nav.js`) bir ekranı `x-zeyn-partial: 1` ile ister ve yalnızca HTML'ini alır; sayfa başlığı `x-zeyn-title` içindedir. Ekran head etiketleri ayarlıyorsa (açıklama, canonical URL, önizleme görseli, robots, JSON-LD), gövde bunları tutan `<template data-zeyn-head>` ile başlar; `nav.js` belge head'indeki her `[data-zeyn-head]` elemanını kaldırır, şablonun etiketlerini oraya taşır ve şablonu siler. Head etiketi olmayan bir ekran şablon göndermez, bu yüzden eski etiketler yalnızca kaldırılır.

### 4.1 Screen fragment

`GET /_zeyn/screen/{screen}?params={json}` yalnızca ekranın HTML'ini, sayfa kabuğu olmadan döndürür. Pencereler ve satır ayrıntıları için kullanılır: `data-zeyn-detail` olan bir satırı açmak, ekranını hemen altındaki yeni bir `tr.z-detail` içine yükler (satırı kaplayan tek bir hücre, içinde bir `role="region"`), kapatmak bu satırı kaldırır. Ekranın izni (permission) diğer ekranlar gibi denetlenir; ayrıntı içindeki grid'ler sunucuyla ayrıntı ekranının kendi adı ve parametreleriyle konuşur.

### 4.2 Grid rows

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

Filtreler `contains`, `starts_with`, `equals`, `from`, `to`, `any_of` ve `is` alanlarını ayarlayabilir (bkz. `docs/grid.md`). `view.columns`, satırların oluşturulacağı sütun düzenidir; `head: true` ile yanıtta ayrıca başlık satırı olan `head` bulunur.

Sonsuz kaydırma için `page` değeri `{ "type": "after", "cursor": "...", "limit": 100 }` olur ve ilk grup için `cursor` `null` olur.

`200` yanıtı:

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

`foot`, toplamlar istenmişse toplam satırıdır, aksi halde `null`.

`total` ve `next` `null` olabilir. Runtime, sayfalı grid'lerde satırları değiştirir, sonsuz kaydırmada ise sona ekler.

Gruplanmış ve ağaç grid'leri (`<table role="treegrid">`) bir dalı yüklemek için `POST /_zeyn/grid` ile `path` gönderir: açılan satırın grup değerleri (veya üst anahtarları), satırın `data-zeyn-path` değerinde bulunduğu gibi. Yanıt aynı biçimdedir; `rows` grup satırlarını veya bir seviye daha derindeki satırları tutar ve `total` bunları sayar. Runtime bunları açılan satırın arkasına koyar ve satır kapanırken onu izleyen her daha derin satırı kaldırır. Grid'in grup seviyelerinden (ağaçta 32'den) uzun bir yol `404` döndürür.

### 4.2.1 More grid endpoints

| Endpoint | Body | Does |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | Kullanıcının görünümünü kaydeder (`null` siler). `204`; görünüm deposu yoksa `404` |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | Bir satır öğesini çalıştırır (`a0` satır eylemi, `m2.1` menü yolu). Sunucu satırı anahtarıyla yükler ve girdiyi oluşturur. `{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | Seçilen anahtarlarla toplu bir eylem çalıştırır; `all` ise `query.filters` ile eşleşen her satırla çalıştırır. `{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | Satır içi bir düzenlemeyi kaydeder; `values`, düzenlenen sütunları ve satır sürümünü bir formun gönderdiği gibi tutar. `{row}`; düzenlenemeyen sütunlar için `403`, `409`, `422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | Eşleşen her satırı CSV olarak akıtır |
| `POST /_zeyn/menu` | `{screen, params, item}` | Ekranın bağlam menüsündeki bir eylem öğesini çalıştırır |

### 4.2.2 Virtual scrolling

`data-zeyn-paging="virtual"` olan grid'ler, kaç satır olursa olsun tüm satırlar üzerinde tek bir kaydırılabilir liste gösterir. Sunucu, `data-zeyn-total` ile ilk bloğu (`data-zeyn-page-size` satır, ofset 0) oluşturur: ilk 64 satır tabloda, kalanı ondan sonra gelen `<template data-zeyn-rows-later>` içinde durur; grid 40 veya daha fazla sütun gösteriyorsa tüm satırlar template içindedir. Grid kökünde ayrıca `data-zeyn-row-height (piksel) ve `--z-row-height` CSS değişkeni bulunur.

Runtime:

- Tablo gövdesini, iç ara parçası `total * rowHeight` piksel yüksekliğinde olan bir kaydırma kapsayıcısına koyar; yükseklik 8.000.000 pikselle sınırlıdır. Sınırın üstünde kaydırma konumu satır indeksine oranla eşlenir (`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`), böylece 1.000.000 satırda da sona ulaşılır.
- DOM'da yalnızca görünen satırları ve alt ile üstteki 20'şer satırı tutar; bunları `transform: translateY(...)` ile konumlandırır. Satırların yüksekliği sabittir; uzun metin üç noktayla kesilir.
- `data-zeyn-page-size` satırlık blokları `POST /_zeyn/grid` ile ve `page: { "type": "offset", "offset": blockStart, "limit": blockSize }` kullanarak yükler; ilk çağrıdan sonra `with_total: false` olur. Önbellekte en fazla 20 blok ve en çok 100.000 hücre tutar (en az son kullanılan çıkar; 63 sütunlu bir grid için 7 blok, hiçbir zaman 3'ten az değil), `reload` için saklanan blokları aynı sınıra sayar ve onları ilk önce atar, kaydırma yönündeki sonraki bloğu önceden getirir ve görünümden çıkan bloklar için istekleri iptal eder (`AbortController`).
- Bir blok yüklenirken görünen satırları asla temizlemez. Eksik satırlar, blokları gelene kadar `aria-busy="true"` olan bir yer tutucu satır gösterir; böylece ekran yanıp sönmez.
- Sıralama ve filtre değişiklikleri önbelleği düşürür ve en üste kaydırır.
- Klavye: Ok tuşları, Page Up, Page Down, Home ve End etkin satırı taşır ve görünüme kaydırır. Tabloda `aria-rowcount` = total, her satırda `aria-rowindex` bulunur.
- Konum metni (`data-zeyn-page-info`) `data-zeyn-text-range-total` değerini kullanır.

Kart ızgaraları (`zeyn::ui::CardGrid`; şeritler veya satır listeleri değil) 200 karta ulaştığında bir kaydırma kapsayıcısı yerine sayfanın kendi kaydırmasında pencereleme yapar: `catalog.js` listeyi `data-zeyn-window` ile işaretler, içinde görüntü alanına en fazla 800 px uzaklıktaki satırları tutar ve üstteki ile alttaki satırları dolguya çevirir, her satıra ilk kartların en uzununun yüksekliğini verir (`grid-auto-rows`) ve her kartta `aria-posinset` ile `aria-setsize` değerlerini ayarlar (daha fazla sayfa gelebilecekken `-1`). Pencerenin dışındaki odaklı bir kart `z-cards-kept` ile DOM'da kalır ve kendi hücresinin üzerine yerleştirilir. "Show more" ile veya başka bir sayfadan gelen kartlar aynı pencereye katılır. Sayfa ms başına 3 px'ten hızlı kayarken kartlar görselleri bekletilerek gelir (`data-zeyn-park`; `src` ve `srcset` bir kenarda tutulur) ve 120 ms boyunca kaydırma olmayınca görsellerini geri alır.

### 4.3 Form submit

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

Değerler kullanıcının yazdığı gibi gönderilir: girişler ve seçimler için dizeler, onay kutuları için boolean'lar. Sunucu bunları alan türüne göre ayrıştırır.

Sunucu, ekranın yüklediği modelden başlar ve yalnızca formda bulunan ve salt okunur olmayan alanların üzerine yazar. İstemciden gelen diğer değerler yok sayılır. Anahtarlar hiçbir zaman istemciden gelmez.

Yanıtlar:

- `200 { "effects": [ ... ] }`: kaydedildi; effect'leri sırayla çalıştırın.
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`: her iletiyi kendi alanının altında gösterin (`#{scope}-{field}-error`), girişte `aria-invalid="true"` ayarlayın ve ilk geçersiz girişe odaklanın. `field` değeri boş olan hatalar formun en üstünde gösterilir.

### 4.4 Errors

Herhangi bir endpoint şu yanıtları verebilir:

| Status | Body | Meaning |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | İzin eksik, guard reddetti veya CSRF belirteci hatalı |
| `404` | `{ "error": "not_found", "message": "..." }` | Bilinmeyen ekran veya düğüm |
| `409` | `{ "error": "conflict", "message": "..." }` | Kaydı başka biri değiştirdi |
| `422` | bkz. 4.3 | Doğrulama |
| `504` | `{ "error": "timeout", "message": "..." }` | Çağrı sunucu sınırından uzun sürdü |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | Ayrıntılar yalnızca sunucu günlüğündedir |

Runtime, `message` değerini bir hata toast'ı olarak gösterir.

### 4.5 Language

`{ "locale": "tr" }` ile `POST /_zeyn/locale`, seçimi `zeyn_locale` çerezinde saklar ve `204` yanıtı verir. Runtime sayfayı yeniden yükler. Çerez yoksa sunucu önce `Accept-Language` başlığına, sonra uygulamanın varsayılanına göre seçer. `<html lang>` her zaman seçilen yerel ayarla eşleşir.

Uygulamanın birden fazla yerel ayarı varsa kabuk bir dil seçici (`select[data-zeyn-locale]`) oluşturur.

### 4.6 Theme

Uygulama gizlemediği veya tek bir tema sunmadığı sürece kabuk bir tema seçici (`select[data-zeyn-theme-picker]`) oluşturur. Her seçenek `data-scheme` (`light`, `dark`, Sistem için boş) ve uygulama temaları `data-custom` taşır. Değişiklikte runtime `<html>` üzerinde `data-theme` ve `data-zeyn-theme` ile `color-scheme` meta etiketini hemen ayarlar, ardından `{ "theme": "dark" }` ile `POST /_zeyn/theme` gönderir. Sunucu `zeyn_theme` çerezi ile `204` yanıtı verir; uygulamanın sunmadığı bir tema için `404` verir. Sayfalar bu çerezdeki temayla oluşturulur, bu yüzden yeniden yükleme ve yanıp sönme olmaz. Bkz. `docs/theming.md`.

### 4.7 Enterprise endpoints

`enterprise.js` ve `enterprise.css`, runtime ile birlikte yüklenir. Ayrıntılar `docs/enterprise.md` içindedir.

| Endpoint | Purpose |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | Maskelenmiş bir alanın ham değeri, `{value}`; maske kaldırma izni gerekir, her zaman denetim kaydına yazılır |
| `POST /_zeyn/reason {screen, reason}` | `reason` olarak işaretlenmiş bir ekran için gerekçe; `HttpOnly` çerezle `204` |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV önizlemesi (`commit: false`) veya içe aktarma |
| `POST /_zeyn/keepalive` | Boşta uyarısından sonra "Oturumu açık tut"; `204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | Canlılık ve hazırlık; health istek sınırına tabi değildir, readiness yalnızca `probe_ips` adreslerinden erişilir |
| `GET /_zeyn/jobs` | Jobs iznine sahip kullanıcılar için iş durumu |

Her yanıt `x-zeyn-build` taşır. Runtime'ın üzerinde işlem yaptığı retler: `428 reason_required` (bir gerekçe ister ve isteği yineler) ve `401 reauth_required` (kullanıcıdan yeniden oturum açmasını ister ve isteği yineler).

## 5. Security

- Her yanıt şunları ayarlar: `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`, `X-Content-Type-Options: nosniff` ve `Referrer-Policy: same-origin`. `App::csp`, adı verilen `https` sunucularını `img-src`, `media-src` (ve `blob:`), `frame-src`, `connect-src` ve `font-src` yönergelerine ekler; diğer yönergeler hiçbir zaman değişmez. Başlık metni açılışta bir kez oluşturulur.
- Satır içi betik veya satır içi olay işleyicisi yoktur. Satır içi `style` öznitelikleri yalnızca CSS özel özellikleri (`--z-width`, `--z-flex`) ve sabit sütun genişlikleri için kullanılır; bu yüzden politika `style-src-attr 'unsafe-inline'` ekler. Betikler sıkı kalır.
- CSRF belirteci `HttpOnly`, `SameSite=Strict` bir çerezde saklanır ve `x-zeyn-csrf` başlığıyla sabit zamanlı karşılaştırılır.
- Sayfalar, `totp` formu ve `GET /_zeyn/csrf` belirteci çerezdeki haliyle asla taşımaz: her yanıt yeni bir rastgele pad ve ardından belirtecin bu pad ile XOR'lanmış halini (128 onaltılık hane) alır, böylece gzip onu sızdıramaz (BREACH). Sunucu karşılaştırmadan önce maskeyi açar ve düz 64 haneli belirteci hâlâ kabul eder.
- 1 KB veya daha büyük dinamik yanıtlar (sayfalar, kısmi ekranlar, grid blokları, dışa aktarmalar) tarayıcı kabul ediyorsa 2. seviyede gzip ile sıkıştırılır ve `Vary: Accept-Encoding` taşır. Asla sıkıştırılmayanlar: zaten bir `Content-Encoding` taşıyan yanıtlar (bir kez sıkıştırılıp önbelleğe alınan framework varlıkları), aralıklar (`Content-Range` ve `Range` içeren her istek), video, ses, SVG dışındaki görseller, `application/octet-stream`, arşivler ve PDF'ler, `text/event-stream`, `application/x-ndjson` (asistanın akışı) ve WebSocket yükseltmeleri.
- Her çağrı sırasıyla şunları denetler: kimlik doğrulama, ekranın erişimi, Feature'ın guard'ları, düğüm eyleminin erişimi; ardından interceptor'ları ve eylemi çalıştırır.

## 6. Runtime behavior

- JavaScript olmadan sayfalar yine de her grid'in ilk sayfasıyla oluşturulur. Formlar ve sayfalama runtime'a ihtiyaç duyar.
- Klavye: ok tuşları grid satırları arasında gezinir, Enter bir satırı açar, Escape en üstteki diyaloğu kapatır, `aria-keyshortcuts` kısayolları metin girişleri dışında her yerde çalışır. Grid'lerde: Boşluk satırı seçer, F2 düzenler, Ctrl+Z son düzenlemeyi geri alır, Shift+F10 veya bağlam menüsü tuşu satırın ya da başlığın menüsünü açar, Shift+tıklama birden çok sütuna göre sıralar, Sağ ve Sol (sağdan sola sayfalarda yansıtılmış olarak) bir satırın ayrıntısını, grubunu veya çocuklarını açar ve kapatır, bir çocuk satırdaki Sol ise üst satıra gider. Bir satırda Sağ, hücrelerine geçer: oklar, Home, End, Ctrl+Home, Ctrl+End, Tab ve Shift+Tab hücre imlecini taşır, Shift bir aralığı büyütür (hücrelerde `aria-selected`), Ctrl+C aralığı sekmeyle ayrılmış metin olarak kopyalar ve Ctrl+V düzenlenebilir hücrelere yapıştırır; değişen her satır için bir `POST /_zeyn/grid/edit` gönderilir. 40 veya daha fazla sütunu olan grid'ler sayfada yalnızca görünümdeki sütunları tutar (sabit genişlikli bir `colgroup`, `colspan` taşıyan `td.z-col-gap` ara parçaları, hücrelerde `aria-colindex`).
- Sağ tık, tanımlı olan yerde Zeyn'in menüsünü açar (başlıklar, `data-zeyn-menu` olan satırlar, `data-zeyn-menu` olan ekranlar); başka her yerde tarayıcının kendi menüsü kalır.
- Odak, diyalog açıldığında diyalogun içine taşınır ve kapandığında açana geri döner.
- Tıklamanın karesinde bekleme durumu: `nav.js` bir ekran yüklenirken hedef bağlantıyı geçerli olarak işaretler ve `html[data-zeyn-loading]` ayarlar, pencereler ve form gönderimleri de onu ayarlar; gridler bir sıralama, sayfa ya da filtre tuşuna basılır basılmaz satırlarını soluklaştırır (`aria-busy`) (filtreler başka tuşlar için 150 ms bekler ve yeni bir istek eskisini iptal eder). Pencerenin üstündeki ince çubuk yalnızca yanıt 150 ms'den uzun sürdüğünde görünür; sabittir, bu yüzden hiçbir şey kaymaz ve `prefers-reduced-motion` ile hareket etmez.
- İstemci tarafı denetimler (`required`, `minlength`, `maxlength`, `min`, `max`, `type="email"`) göndermeden önce tarayıcının kısıt doğrulama API'siyle çalışır. Sunucu her zaman yeniden doğrular.

## 7. Component library

`zeyn::ui` bileşenleri bölüm 1.1'deki modüllerle ve `/_zeyn/components.css` stil dosyasıyla sunulur. Runtime gibi bunlar da oturum açmayı gerektirmez.

### 7.1 Endpoints

Hepsi JSON ve CSRF başlığıyla `POST`'tur ve önce ekranın erişimini ve Feature'ın guard'larını denetler.

| Endpoint | Body | Answer |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | Bir düğümün, grid'lerinin ilk sayfasıyla birlikte HTML'i. Tembel (lazy) sekmeler tarafından kullanılır. |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`: screen'in tek bir çalıştırmasından render edilen birkaç node; yenilenen ya da sürülen bir topic'in dinleyicileri için. En fazla 32 farklı yol; screen'de olmayan bir yol `404` döndürür. Screen'in erişimi, guard'ları ve interceptor'ları bir kez çalışır ve params screen'in tipli params'ı olarak okunur. |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | Düğümün `key` veri kaynağından JSON: arama seçenekleri (`key` alan adıdır), ağaç çocukları (`key` boştur), adım denetimleri (`key` `_step` olur). Kaynağın eylem izni denetlenir ve interceptor'lar, grid'lerde olduğu gibi çalışır. Eyleme uymayan girdi `404` ile reddedilir. |
| `/_zeyn/data` ve `screen` = `_site.{feature}` | yukarıdaki gibi | Bir site üst bilgisi slotundaki bileşen için aynısı: `node` slottur (0 start, 1 center, 2 end, 3 bottom), ardından düğümün içindeki sırası, sonra alt öğe sıraları. Feature'ın guard'ları ve action'ın izni denetlenir. |
| `/_zeyn/parse` | `{ kind, text, label }` | Sunucunun yazılmış bir tarihi veya saati kullanıcının dilinde, saat diliminde ve saat biçiminde nasıl okuduğu: `{ ok, text, iso, utc, zone }` veya `{ ok: false, message }`. `kind` değeri `date`, `time`, `datetime`, `local` veya `presets` (bugün ve hızlı aralıklar) olur. |

### 7.2 Values the forms submit

- Lookup'lar kimlikleri metin olarak gönderir: `"7"`, birden çok için `"3,7,9"`.
- Lines, `"{field}": "rows"` gönderir ve her hücre için bir değeri `"{field}.{line}.{column}"` olarak, ayrıca satırın geldiği yüklü satırın indeksi olan `"{field}.{line}._i"` değerini gönderir (yeni satırlar için boş). Sunucu her satırı yüklü satırından yeniden oluşturur; böylece yalnızca gösterilen, düzenlenebilir sütunlar değişir.
- Bir satırın doğrulama hataları `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }` biçimini kullanır.

### 7.3 Events

| Event | On | Detail |
|---|---|---|
| `zeyn:content` | `document` | Yeni içeriği olan eleman (gezinme, tembel paneller). Runtime gereken modülleri yükler ve yüklü her modül içerdiklerini kurar. |
| `zeyn:errors` | the form | Runtime onları gösterdikten sonra `422` hataları. Satır düzenleyici ve adımlı formlar bunu kullanır. |
| `zeyn:effect` | `document` | Runtime'ın tanımadığı bir effect: `{ effect, source, run }`. `run(next)` başka bir effect çalıştırır. `confirm` tarafından kullanılır. |
| `zeyn:topics` | `document` | `{ topics }`: başka bir betikten yenilenecek topic'ler (bir bildirimde `live.js`). |
| `zeyn:reload` | bir öğe | İçindeki grid'leri yerinde yeniden yükler: geçerli sayfayı, ya da `{ reset: true }` ile ilk sayfayı. |
| `zeyn:density` | `document` | Yok. Kullanıcı hesap menüsünden başka bir yoğunluk seçti (`<html>` üzerindeki `data-density` zaten değişti); pencereli kart grid'leri satırlarını yeniden ölçer. |

`{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` effect'i modal bir uyarı diyaloğunda sorar ve `then` değerini yalnızca kullanıcı onayladıktan sonra çalıştırır.

Effect `{"effect":"refresh","topics":["sales"]}` topic'lerin dinleyicilerini yeniler. `[data-zeyn-listens="a,b"]` bir dinleyiciyi işaretler (`data-zeyn-node` üst öğesindeki node); `[data-zeyn-drives="a"]` topic süren bir kontrolü işaretler: bir filtre formu (`data-zeyn-kinds` hangi params'ın sayı ya da boolean olduğunu söyler), bir arama kutusu (alanının adı `q`'dur) ya da filtre çipleri (her çip params'ını `data-zeyn-params` içinde taşır). Sürmek, kontrolün değerlerini screen kökünün `data-zeyn-params` değerine birleştirir, screen ana screen ise adresi bunlarla değiştirir ve dinleyicileri yeniler. `topics.js` bir animasyon karesi içinde adlandırılan topic'leri tek seferde işler: grid tutan bir dinleyici `zeyn:reload` alır, bir screen'in diğerleri tek bir `POST /_zeyn/nodes` içinde gider ve aynı screen için daha yeni bir istek eskisini iptal edip node'larını devralır. Odak ve açık `<details>` id, test id, `data-zeyn-key` satırı ve konumla korunur.

Sanal bir liste `data-zeyn-row-height`, `data-zeyn-block` ve `data-zeyn-total` taşıyan `section[data-zeyn-vlist]` öğesidir; `vlist.js` blokları `{ page: { type: "offset", offset }, with_total: true }` ile `POST /_zeyn/grid` üzerinden ister ve her `<li>`'yi `(aria-posinset - 1) * height` konumuna yerleştirir. Başka bir alana bağlı lookup'lar `data-depends-on="field"` taşır ve o alanın değerini `/_zeyn/data` girdisinde `parent` olarak gönderir.

İstemci tercihleri (`zeyn::ClientPref`), sayfaların yalnızca `data-zeyn-remember` ya da `data-zeyn-prefs-clear` taşıyan bir eleman içerdiğinde içe aktardığı `/_zeyn/state.js` tarayıcı modülünde yaşar. `GET /_zeyn/prefs`, `Cache-Control: no-store` ile `{ preferences }` döndürür; bu, uygulamanın onay kancasının tercih değerlerinin oturumun ötesinde tutulmasına izin verip vermediğidir. Sunucu saklanan hiçbir değeri okumaz.

### 7.4 Media

`Video` ve `Audio`, bileşen kitaplığından sonra bağlanan `/_zeyn/media.js` ve `/_zeyn/media.css` ile sunulur. Bkz. `docs/media.md`.

Oynatıcılar kaynaklarını ve afişlerini (önce `data-zeyn-poster-low`, sonra `data-zeyn-poster`) ilk kez görünüme girdiklerinde, sayfa için tek bir `IntersectionObserver` aracılığıyla ayarlar; `data-zeyn-preview` taşıyan bir kök, dörtte biri görünür olduğu sürece sessiz ve döngüde oynar, `prefers-reduced-motion`, `prefers-reduced-data` ya da Save-Data aksini söylemedikçe ve ziyaretçi kontrollerini kullanana kadar. Yükleme seçenekleri olan görseller, yer tutucuyu betik olmadan gösteren bir kutuda (`.z-img-host`) durur ve `media.js`'i yükleyen işaret olan `data-zeyn-img` özniteliğini taşır. Belgedeki tek bir capture dinleyicisi bu görsellerin `load` ve `error` olaylarını işler: yüklenen görsel kutuyu `z-img-done` (düşük dosyanın üzerinde belirerek geçiş için ayrıca `z-img-fade`) olarak işaretler; başarısız olan bir kez `data-zeyn-fallback` dener, ardından kutunun `<template data-zeyn-img-error>` öğesini ya da bir simgeyle alt metni gösterir. Modül çalışmadan önce sonlanmış görseller, modül başladığında bir kez denetlenir.

| Endpoint | Does |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | Kayıtlı bir medya kaynağının dosyası, aralık (range) isteklerini destekler: `200`, `206`, `304`, `416`. |
| `GET /_zeyn/media/position?source=&id=` | Oturum açmış kullanıcının kaldığı yer olan `{ at, kept }`; kaynak konum tutmuyorsa `kept` `false` olur (oynatıcı bu durumda yerel depolamayı kullanır). |
| `POST /_zeyn/media/position` | CSRF başlığıyla `{ source, id, at }` konumu kaydeder: `204`, hatalı konum için `422`. |

### 7.5 Islands

`islands` özelliğiyle, `div[data-zeyn-island]` içeren bir ekran `<meta name="zeyn-islands">` (island adı ile `{ js, wasm }` URL'leri), `/_zeyn/islands.js` ve `'wasm-unsafe-eval'` içeren bir politika ile sunulur; diğer sayfalar değişmez. Bir island içeren kısmi yanıt `x-zeyn-islands: 1` taşır ve belgede island politikası yoksa `nav.js` onu tam olarak yükler.

| Attribute or endpoint | Meaning |
|---|---|
| `data-zeyn-island`, `data-zeyn-load`, `data-zeyn-props` | Island adı, `visible`, `interaction` veya `idle`, props JSON'u |
| `data-zeyn-island-fallback` | Yedek içerik, island bağlanınca gizlenir |
| `data-zeyn-island-state` | Yükleyici tarafından ayarlanır: `loading`, `ready`, `failed` |
| `GET /_zeyn/islands/{file}` | Manifestte listelenen derlenmiş dosya, bir yıl önbelleğe alınır |
| `POST /_zeyn/data` with `key` = remote name | Island'dan tipli bir çağrı, 7.1'deki tüm denetimlerle |

Bkz. `docs/islands.md`.

### 7.6 Realtime

`realtime` özelliğiyle, `div[data-zeyn-live="topic"]` sunucu konuyu bildirdiğinde yenilenen düğümleri işaretler. Birini içeren sayfalar `/_zeyn/live.js` dosyasını bağlar; birini içeren kısmi yanıt `x-zeyn-live: 1` taşır ve `nav.js` betiği içe aktarır. Bir eleman üzerindeki `zeyn:reload`, içindeki grid'leri yerinde yeniden yükler.

| Endpoint | Does |
|---|---|
| `GET /_zeyn/live/topics?t=a,b&csrf=` | WebSocket; metin çerçeveleri `{ "topic": "a" }` |
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=` | WebSocket; ikili çerçeveler `0x01` + Y güncellemesi (v1), iki yönde |

El sıkışmalar, yükseltmeden önce origin'i, CSRF belirtecini çerezle karşılaştırarak, oturum açmış kullanıcıyı ve izinleri denetler. Bkz. `docs/realtime.md`.

Oynatıcı, aşamalı (progressive) dosyalara dönmeden önce kökünde iptal edilebilir `zeyn:media-adaptive` olayını (`{ media, kind, url, hls, dash }`) gönderir; bir akış kitaplığı devralmak için `preventDefault()` çağırır.

## 8. Desktop class shell

`/_zeyn/pro.js` ve `/_zeyn/pro.css` bileşen kitaplığından sonra yüklenir. Runtime gibi bunlar da oturum açmayı gerektirmez.

### 8.1 Commands and the palette

Komutlar Rust'ta tanımlanır (`Command::open`, `Command::navigate`, `Command::effect`, `Command::action`) ve `Feature::command` ile kaydedilir. Kabuk, kullanıcının çalıştırabileceği her komut için (Feature guard'ları geçer ve komutun erişimi izin verir) `div[data-zeyn-commands]` içinde gizli bir düğme oluşturur:

| Attribute | Meaning |
|---|---|
| `data-zeyn-command` | Komut kimliği |
| `data-zeyn-effect` | Bir effect komutunun effect'i, tıklamada runtime tarafından çalıştırılır |
| `data-zeyn-command-run` | Bir eylem komutunun kimliği, `POST /_zeyn/command` ile çalıştırılır |
| `aria-keyshortcuts` | Normalleştirilmiş kısayol (`Control+Alt+N`); runtime'ın kısayol işleme mantığı düğmeye tıklar |
| `data-hint`, `data-icon` | Kullanıcılara gösterildiği haliyle kısayol, simge adı |
| `data-text-*` on the container | Yerelleştirilmiş palet metinleri |

`App::check`, okunamayan kısayolları, tek karakterli kısayolları (WCAG 2.1.4), paletin kendi kısayolunu, iki komutun kullandığı kısayolları veya kimlikleri, tarayıcının sayfaya hiç iletmediği kısayolları (`BROWSER_RESERVED`: `Ctrl+N`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Alt+F4` ve macOS biçimleri) ve komut `.override_browser()` çağırmıyorsa tarayıcının kullandığı kısayolları (`BROWSER_DEFAULTS`: yazdırma, kaydetme, bulma, adres çubuğu, yeniden yükleme, yakınlaştırma, geliştirici araçları, kopyala ve yapıştır, `F1` ile `F12` arası) reddeder. Araç çubuğu düğmeleri çalışma zamanında aynı kurallara uyar: böyle bir kısayol bağlanmaz ve nedeni günlüğe yazılır. Serbest kombinasyonlar `Alt+Shift+<letter>` ve `Ctrl+Alt+<letter>` biçimindedir.

`Alt+Shift+P` (macOS'ta Option+Shift+P) paleti varsayılan olarak açar; `App::palette_shortcut("Ctrl+Shift+P")` bunu, bilerek tarayıcının kullandığı bir tuşa bile olsa, değiştirir. Kısayol `data-palette-keys` içindedir ve harfler fiziksel tuşa göre eşleşir; böylece Option ve AltGr bozmaz. Palet ayrıca menüdeki `button[data-zeyn-palette-open]` ile de açılır. ARIA combobox ve listbox içeren modal bir `<dialog>`'dur. Uygulamanın komutlarını, geçerli ekranın (bir pencere açıksa en üstteki pencerenin) araç çubuğu düğmelerini, menü çubuğu komutlarını ve bağlam menüsü öğelerini ve menüdeki ekranları listeler. Eşleştirme bulanıktır (fuzzy), büyük/küçük harfi ve aksanları yok sayar; en son çalıştırılan sekiz komut başta gelir (`localStorage` anahtarı `zeyn.palette.recent`). Escape paleti kapatır ve odak önceki yerine döner.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | Girdisi sunucuda sabitlenmiş bir eylem komutunu çalıştırır: CSRF, Feature guard'ları, eylemin izni ve interceptor'lar. `{ effects }`; bilinmeyen kimlikler ve effect komutları için `404` |
| `POST /_zeyn/search` | `{ query }` | Kullanıcının çalıştırabileceği her `Feature::search` değerini (CSRF, Feature guard'ları, eylemin izni, interceptor'lar) sorgu 100 karakterde kesilerek çalıştırır. `{ hits: [{ label, group, url }] }`, en fazla 20, yalnızca uygulama yolları |

Kapsayıcıda `data-search` varsa palet sorguyu son tuştan 150 ms sonra gönderir (iki karakter veya daha fazla) ve sonuçları komutların altında listeler. Yalnızca en son sorgunun yanıtı çizilir. Bir sonucu seçmek, `url` önüne temel yolu ekleyerek `zeyn:navigate` gönderir; içindeki `#anchor`, ekran değiştirildikten sonra o elemana kaydırır.

Runtime, diğer betiklerin işi onun üzerinden çalıştırabilmesi için `document` üzerinde `zeyn:run` olayını dinler: `{ effects, source }` effect'leri çalıştırır, `{ screen, item }` bir ekran bağlam menüsü öğesini çalıştırır, `{ error }` bir hata toast'ı gösterir.

### 8.2 Menu bar, toolbox, property inspector, status bar

| Markup | Behavior |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG menü çubuğu. Üst öğeler, dolaşan sekme durağı (roving tab stop) olan `button[role=menuitem][aria-haspopup=menu]` düğmeleridir; her biri, kapalıyken `hidden` olan bir `ul[role=menu]` denetler. Öğeler 8.1'deki komut özniteliklerini taşır. Sol ve Sağ menüler arasında gezinir, Aşağı, Enter ve Boşluk açar, Yukarı son öğede açar, Home ve End atlar, bir harf o harfle başlayan sonraki öğeye gider, Escape kapatır ve çubuğa döner, Tab ve dışarı tıklama kapatır. Kullanıcının çalıştıramayacağı menüler ve öğeler oluşturulmaz. |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | `button.z-tool[aria-pressed]` düğmelerinden oluşan geçiş grubu. Oklar, Home ve End dolaşan sekme durağını taşır; bir tıklama, Enter veya Boşluk bir aracı açar, `zeyn:tool` (`detail.tool`) gönderir ve `data-text-selected` değerini durum çubuğuna yazar. |
| `section.z-inspector` | `Form::section` grupları olan bir form (`role=group` gövdeli `details.z-form-section`), inspector'ın düğüm yolunda `POST /_zeyn/form` ile kaydedilir. Hatalı alanlar içeren bölümler, tarayıcının denetimlerinden önce ve bir `422` sonrasında açılır. |
| `section.z-inspector[data-zeyn-inspector=name]` | Her alan türü için bir `template[data-kind]` olan bir ada denetçisi. `[data-zeyn-island=name]` içinden gelen `zeyn:inspect` (`detail: { title, schema, values }`) şemayı `innerHTML` ile değil, DOM çağrılarıyla çizer; `data-limits` sınırını aşan şema yok sayılır, daha önce görülen bir şema alanlarını korur. Geçerli ve değişmiş bir değer ada köküne `zeyn:property` (`detail: { name, value }`) olarak gider; geçersiz bir değer alanın hata satırını doldurur ve `aria-invalid` koyar. |
| `div[data-zeyn-statusbar]` | Başta ve sonda öğeler ile `[data-zeyn-status-live]` (`role=status`, `aria-live=polite`). Ekranın herhangi bir elemanından gelen `zeyn:status` (`detail.text`), canlı metnin yerine geçer. |

### 8.3 Docking panels

`div.z-dock[data-zeyn-dock]`, dört `section.z-dock-area[data-area]` (`left`, `center`, `bottom`, `right`) ve üç `div.z-dock-bar[role=separator][data-edge]` ayırıcıdan oluşan bir grid'dir. Boyutlar `--z-dock-left`, `--z-dock-right` (genişliğin yüzdesi) ve `--z-dock-bottom` (yüksekliğin yüzdesi) özel özellikleridir, 10 ile 60 arası. Her alanda sürüklenebilir `button.z-dock-tab[data-panel]` düğmelerinden oluşan bir `role=tablist`, bir seçenekler düğmesi ve çocuk düğümleri tutan `role=tabpanel` panelleri bulunur (yollar herhangi bir kapsayıcıdaki gibidir; bir panel taşındığında asla değişmezler). Açık paneli olmayan yan alanlar gizlenir.

Klavye: Sol ve Sağ (sağdan sola sayfalarda yansıtılmış olarak), Home ve End bir alanın açık sekmeleri arasında gezinir; Delete odaktaki paneli kapatır; Shift+F10 veya bağlam menüsü tuşu alanın seçeneklerini açar (başka alana taşı, kapat, kapalı panelleri göster); bir ayırıcı üzerindeki oklar yüzde 5 yeniden boyutlandırır, Home ve End sınırlara gider. `MenuList::panels` ile yapılan bir menü çubuğu girdisi her paneli `menuitemcheckbox` olarak listeler. Değişiklikler `[data-zeyn-dock-live]` içinde duyurulur.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: oturum açmış kullanıcının kaydedilmiş düzeni veya `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`; `layout: null` siler |

`layout`, sekme sırasıyla `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }` biçimindedir. Sunucu CSRF'yi, ekranın erişimini ve Feature'ın guard'larını denetler, yalnızca dock'un kendi panellerini tutar (her biri bir kez) ve boyutları sınırlar, ardından tenant, ekran ve düğüm altında uygulamanın görünüm deposunda (`App::datagrid_views`) saklar. Depo yoksa endpoint `404`, kullanıcı yoksa `403` yanıtı verir; bu durumda tarayıcı düzeni `localStorage` içinde tutar.

## 9. AI agents: MCP and WebMCP

Dışarıdaki AI ajanları uygulamaya, ikisi de varsayılan olarak kapalı olan ve `App::mcp` ile açılan iki kapıdan ulaşır:

| Setting | Endpoint MCP | WebMCP tools | Writes |
|---|---|---|---|
| `Mcp::off()` (default) | `404` | `404` | no |
| `Mcp::read_only()` | on | on | no |
| `Mcp::on()` | on | on | form saves and API actions that write |

`.server(false)` ve `.webmcp(false)` bir kapıyı kapatır. `zeyn new` ile oluşturulan yeni projeler `Mcp::read_only()` ile başlar.

Neden kapalı: endpoint, geçerli bir `Authorization` başlığına sahip herkesin uygulamayı bir model üzerinden yönetmesine izin verir ve bir model okuduğu verilerle yönlendirilebilir. Framework her uygulamayı ajanlara hazır hale getirir (araçlar koddan üretilir, yazılacak bir şey yoktur), ancak bir uygulamanın bu kapıyı açıp açmayacağı ve ajanların yazıp yazamayacağı, uygulamayı işletenlerin kararıdır. Salt okunur mod tek satır uzaklıktadır.

### 9.1 Tools

Araçlar, oturum açmış kullanıcı için uygulamadan oluşturulur:

- her grid için bir arama aracı (`{screen}_{node}_search`, girdi: bir `DataQuery`) ve yazma açıkken, parametresiz açılan her route'lu ekranın her formu için bir kaydetme aracı (`{screen}_{node}_save`, girdi: yazıldığı gibi form değerleri);
- yayımlanmış her API eylemi için bir araç (`api_{feature}_{action}`). Girdisi `DataQuery` olan bir eylem okur; diğer her API eylemi yazma sayılır.

Açıklama; ekran başlığından, ekranın ve eylemin `///` doc yorumunun ilk paragrafından ve `ai = "..."` yönlendirmelerinden oluşturulur. Kullanıcının kullanamayacağı grid'ler ve formlar ile `ai = false` olarak işaretlenmiş ekranlar ve eylemler hiçbir zaman listelenmez ve çağrıldıklarında reddedilir.

`#[action(confirm)]`, bir modelin ancak kullanıcı bunu uygulamada onayladıktan sonra çalıştırabileceği bir eylemi işaretler. `/_zeyn/mcp` bu tür araçları ne listeler ne de çalıştırır (o kanalda kimse onaylayamaz). WebMCP araçları modal bir diyalogda sorar (`zeyn:confirm`) ve ardından form ile birlikte `confirmed: true` gönderir; yerleşik asistan ise kendi sohbetinde sorar (bkz. `docs/assistant.md`).

### 9.2 Endpoints

| Endpoint | Does |
|---|---|
| `GET /_zeyn/tools?screen&params` | Görünümdeki ekran için `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }`. `zeyn.js` bunları `document.modelContext` ile kaydeder |
| `POST /_zeyn/mcp` | JSON-RPC: `initialize`, `tools/list`, `tools/call`. Bir `Authorization` başlığı ve oturum açmış bir kullanıcı gerekir |
| `POST /_zeyn/form` with `via_agent: true` | Bir WebMCP form kaydı; WebMCP ile `Mcp::on()` gerekir. `confirmed: true` yalnızca kullanıcı onayladıktan sonra |

Her ajan çağrısı kullanıcı olarak çalışır: ekranın erişimi, guard'lar, eylemin izni, interceptor'lar, istek sınırları ve zaman sınırı. Ajan çağrıları her zaman denetim kaydına yazılır (`via_agent: true`) ve maskelenmiş alanlar, kullanıcı gösterebilse bile ajanlar için her zaman maskeli kalır. OpenAPI her işlemi `x-zeyn-ai` ile işaretler: `ai = false` için `false`, aksi halde `{ hint, confirm, writes }`.
