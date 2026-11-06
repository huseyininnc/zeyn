# Görüntüleme

Durumu gösteren bileşenler: mesajlar, rozetler, ilerleme, anahtar değer çiftleri, geçmiş ve yer tutucular. Sunucuda render edilir ve kendi istekleri yoktur.

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info`, `success`, `warning` ve `danger`, ya da `Alert::new(tone, text)`. `.title(text)` bir başlık, `.dismissible()` bir kapatma düğmesi ekler. Ton mesajdan önce sesli de okunur, bu yüzden hiçbir zaman yalnızca renkle verilmez.

## Badge

`Badge::new(text).tone(Tone::...)`. Tonlar: `Neutral`, `Info`, `Success`, `Warning`, `Danger` ve `Brand`. Grid'ler değerleri `.badge(|v| tone)` ile rozet olarak gösterir.

## Progress

`Progress::new(label, value, max)` ve isteğe bağlı `.tone(...)`. Etiketi ve değeriyle gerçek bir `progressbar` öğesidir.

## DescriptionList

Anahtar değer çiftleri, bir kaydın alanlarından ya da elle:

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

Değerler grid hücreleri gibi biçimlenir: para, tarih ve seçimler kullanıcının dilinde gösterilir.

## Timeline

`Timeline::new().event(moment, title, detail, tone)` olayları listeler. Zamanlar kullanıcının saat diliminde ve dilinde gösterilir.

## EmptyState ve Skeleton

`EmptyState::new(title).text(text)` boş bir alanı sonraki adımla doldurur: `.link(label, screen, params)` bir `LinkButton` ile başka bir sayfaya gider, `.opens(label, screen, params)` ise uygulamanın bir screen'ini pencere olarak açar. `Skeleton::lines(n)`, bir şey yüklenirken kullanılan yer tutucudur.

## LinkButton

Bir yere gitmek bağlantıdır; bir şey yapmak düğmedir. `LinkButton::new(label, screen, params)`, screen'in kanonik yoluna (örneğin `/products/42`) giden, düğme gibi görünen gerçek bir `<a href>` öğesidir; bu yüzden orta tık, "yeni sekmede aç" ve arama motorları onu olduğu gibi bir bağlantı olarak ele alır. `nav.js` üzerinden sayfa yenilemeden açılır ve Enter onu izler. Görünüm, `.primary()` ana eylemin dolu görünümünü istemedikçe ikincildir. Screen'in bir rotası olmalıdır, yoksa kod derlenmez. `EmptyState::link` ve `Hero::link` bunu kullanır.

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)`, bu tarayıcının uygulama için tuttuğu her istemci tercihini kaldıran bir düğmedir; örneğin bir gizlilik sayfasında `App::client_prefs_table()` listesinin yanında kullanılır. Bir toast onaylar. Sunucunun koyduğu çerezlere (tema, dil, oturum) dokunulmaz. Betikler aynısını `zeyn.prefs.clear()` ile yapabilir.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: bağlantılar tipli screen referanslarıdır, bu yüzden adı değişen bir screen kırık bağlantı bırakmaz.

## ActionButton

`ActionButton::new(label, action, input)` bir action çalıştırır. `.confirm(text)` önce sorar, `.tone(Tone::Danger)` yıkıcı bir işlemi işaretler ve `.on_success(|result, ui| ...)` effect'leri çalıştırır. Girdi sunucuda sabitlenir ve istemciden gelmez; action'ın permission'ı her form gibi sunucuda denetlenir.

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

`Markdown::new(text)` biçimlendirilmiş metni gösterir; örneğin bir yorum, bir ürün açıklaması ya da yardım metni. Metin bir string ya da bir locale anahtarıdır. `markdown` feature'ını gerektirir (`zeyn = { features = ["markdown"] }`), hiçbir bağımlılık eklemez.

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- Başlıklar, paragraflar, listeler, tablolar, kod blokları, alıntılar, ayırıcı çizgiler, satır içi kod, bağlantılar, görseller, kalın ve italik.
- Ham HTML hiçbir zaman olduğu gibi geçirilmez: `<script>` metin olarak görünür, kod blokları da kaçışlanır.
- Bağlantılar yalnızca `http`, `https`, `mailto`, bir çapa ya da göreli bir yolu gösterebilir. `javascript:` ya da `data:` gibi diğerlerinde yalnızca etiket kalır. Kullanıcı bağlantıları `rel="noopener noreferrer nofollow ugc"` alır.
- Görseller yalnızca uygulamanın kendisinden yüklenir (göreli bir yol); diğer görseller alt metnini gösterir.
- `.trusted()`, uygulamanın yazdığı metinler içindir: başlıklar kendilerine bağlantı verilebilmesi için id alır ve bağlantılar `nofollow ugc` özniteliğini kaybeder. Kaçışlama ve bağlantı kuralları aynı kalır.
- 64 KB'ı aşan metin (`MAX_MARKDOWN`) kesilir ve iç içe geçme sınırlandırılır, böylece kötü niyetli girdinin render maliyeti düşük kalır.

## Embed

`Embed`, başka bir siteden bir sayfayı, örneğin bir video oynatıcıyı ya da bir haritayı, kilitli bir çerçevede gösterir. Başlık zorunludur (ekran okuyucular çerçeveleri başlığıyla duyurur); `.title(...)` olmadan embed yerleştirilemez ve derleyici bunu söyler.

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- Host, uygulamanın `frame-src` listesinde olmalıdır: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` ([Güvenlik](security.md#csrf-csp-and-headers) bölümüne bakın). Adres `https` değilse ya da host listede yoksa hiç çerçeve render edilmez; bir uyarı içeriğin gösterilemediğini söyler.
- Her çerçeve `sandbox="allow-scripts allow-same-origin allow-presentation"`, `referrerpolicy="no-referrer"` ve `loading="lazy"` alır. Açılır pencerelere, üst düzey gezinmeye, formlara ve indirmelere hiçbir zaman izin verilemez. `.allow(...)` tarayıcı özellikleri ekler: `Fullscreen`, `PictureInPicture`, `Autoplay` ve `EncryptedMedia`.
- `.click_to_load(poster)` önce bir düğme gösterir; üzerinde poster görseli (uygulamadaki bir yol ya da `img-src` içindeki bir host; yoksa `""`) ve diğer sitenin adı bulunur. Düğmeye basılana kadar diğer siteyle iletişim kurulmaz; ardından çerçeve onun yerini alır ve odağı alır. Betik `media.js` içindedir, bu yüzden ek bir istek maliyeti yoktur.
- `.notice(t!(...))` o düğmeye uygulamanın bir metnini, örneğin bir içerik uyarısını, koyar. Kaçışlanır, diğer tüm metinler gibi çevrilir ve düğmenin erişilebilir adının parçasıdır; böylece ekran okuyucular kullanıcı karar vermeden önce onu okur. `.click_to_load` olmadan, posteri olmayan düğmeyi açar.
- `.remember(EXTERNAL_VIDEO)`, her sayfada yerine anahtarın ömrü boyunca bir kez sorar. Anahtar, `ClientPref::consent` ile bildirilen ve `App::client_prefs` ile kaydedilen bir onay anahtarıdır (bkz. [İstemci tercihleri](security.md#client-preferences)); aynı anahtara bağlı embed'ler cevabı paylaşır. Sonraki sayfalar, uygulama içi gezinmeler ve diğer sekmeler bu çerçeveleri kendiliğinden, odağı taşımadan yükler (WCAG 2.2 3.2.1 On Focus, 2.4.3 Focus Order); odak çerçeveye yalnızca gerçek bir basıştan sonra geçer. Kaydın süresi dolduğunda ya da denetimlerinden geçemediğinde, uygulama anahtarı kaydetmediğinde, depolama engellendiğinde ve sayfa yeni olduğunda ve JavaScript olmadığında düğme geri gelir. Düz bir string ya da bir choice anahtarı derlenmez.
- Embed edilecek bir adresi tutan alan `#[rule(embed)]` alır. Formlar bir URL ya da yapıştırılmış embed kodu kabul eder, yalnızca `src` değerini tutar, `https` ve listedeki bir host ister, izleme parametrelerini (`utm_*`, `fbclid`, `gclid` ve diğerleri) atar ve HTML'i değil, kanonik adresi saklar. API girdileri kanonik adresi göndermelidir.

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

`Image` alt metnine ve boyutuna ihtiyaç duyar; `.alt(...)` ve `.size(width, height)` olmadan yerleştirilemez ve derleyici bunu söyler. Boyut, dosya yüklenirken yeri korur, böylece sayfa kaymaz.

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src`, uygulamadaki bir yol (`/static/logo.png`) ya da uygulamanın `img-src` listesindeki bir hosttaki `https` adresidir (`App::csp(|c| c.img_src([...]))`). Başka her görsel render edilmez ve yerine alt metni gösterilir, böylece tarayıcı engellenmiş bir isteği hiçbir zaman günlüğe yazmaz.
- Her görsel `decoding="async"` alır; `.lazy()` ekranın altındaki görseller için `loading="lazy"` ekler. `srcset` yoğunluk (`2x`) ya da genişlik (`800w`) tanımlayıcıları alır; başka hostlardaki ya da başka tanımlayıcılı girdiler dışarıda bırakılır. Genişlik tanımlayıcıları, görsel genişliğinden `sizes` ekler.
- Tamamen süslemeli bir görsel boş alt metin alır: `.alt(Text::from(""))`.

### Uzun fotoğraf listeleri

Binlerce ürün fotoğrafı olan bir katalogda görsel, yüklenirken bir şey gösterebilir ve yüklenemediğinde toparlanabilir. Hepsi tarayıcının yerleşik özelliklerini kullanır; sayfadaki her görsele medya modülünün tek bir dinleyicisi hizmet eder.

```rust
Image::new(p.photo_800.clone())
    .alt(p.name.clone())
    .size(400, 500)
    .lazy()
    .srcset([(p.photo_400.as_str(), "400w"), (p.photo_800.as_str(), "800w")])
    .sizes("(min-width: 800px) 25vw, 50vw")
    .placeholder(Placeholder::color("#d9dde3"))
    .low(p.photo_40.clone())
    .fallback("/static/no-photo.webp")
    .on_error(t!("shop.no_photo"))
    .fit(Fit::Cover)
```

- `.placeholder(...)` dosya yüklenirken kutuyu betik olmadan doldurur: `Placeholder::color("#d9dde3")` ya da bir tema token'ı (`"--z-surface-2"`), uygulamanın ürettiği çok küçük bir görsel için `Placeholder::blur(data_uri)` (en fazla 600 baytlık base64 bir `data:image/webp`, `avif`, `jpeg` ya da `png` URI'si, asla SVG değil) veya her görselin paylaştığı tek bir dosya için `Placeholder::image(src)`. Bir kuralı çiğneyen yer tutucu dışarıda bırakılır.
- `.low(src)` tam dosyanın altında hemen küçük, düşük çözünürlüklü bir dosya gösterir. Tam dosya aynı anda yüklenir (görsel tembelse tembel olarak) ve hazır olduğunda belirerek gelir; azaltılmış hareket tercihinde geçiş efekti olmadan görünür.
- `.fallback(src)` görsel yüklenemediğinde bir kez başka bir dosyayı dener. Ondan sonra `.on_error(text or component)` gösterilir; o yoksa yer tutucunun üzerinde bir simgeyle birlikte alt metin. Hiçbir şey yeniden denenmez.
- `.priority()` ziyaretçinin gördüğü ilk görseli (en büyük boyama) işaretler: `loading="eager"` ve `fetchpriority="high"`, asla tembel değil. Sayfa başına bir kez kullanın.
- `.sizes(...)` görselin yerleşimde ne kadar geniş olduğunu söyler, böylece tarayıcı doğru `srcset` genişliğini seçer. O olmadan tembel bir görsel `sizes="auto"` alır ve tarayıcı yuvayı ölçer.
- `.fit(Fit::Cover)` kutuyu doldurur ve taşan kısmı keser; `Fit::Contain` dosyanın tamamını gösterir.
- `low`, `fallback` ve `Placeholder::image`, `src` ile aynı adres kurallarına uyar; diğerleri dışarıda bırakılır. Kutu her zaman görselin boyutunu korur, böylece dosyalar yüklenirken ya da yüklenemediğinde hiçbir şey kaymaz.

## ExternalLink

`ExternalLink::new(url, text)` başka bir siteye `rel="noopener noreferrer"` ile bağlanır; böylece diğer site ne sayfa adresini ne de pencere üzerinde bir tutamaç alır. Ekran okuyucular metinden sonra "external site" (harici site) der; görsel olarak bir ok bunu işaretler.

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()`, kullanıcıların yazdığı adresler için `nofollow ugc` ekler. `.new_tab()` yeni bir sekme açar ve bunu ekran okuyuculara bildirir; ayarlanmadıkça bağlantılar aynı sekmede kalır. Yalnızca `https` ve `http` adresleri bağlantı olur; `javascript:` gibi başka her şey düz metin olarak gösterilir.

## Tooltip, popover ve busy katmanı

Tooltip, tek bir kontrolün kısa açıklamasıdır: `ActionButton::tip(text)`, bir screen araç çubuğunda `ToolButton::tip(text)` ve alanın etiketinden sonra bir bilgi düğmesi ekleyen `Form::tip(field, text)`. Metin kontrolün `aria-describedby` değeridir, bu yüzden ekran okuyucular onu kontrolle birlikte duyar. Kısa bir bekleme sonrası, odakta ise hemen görünür, imleç üzerindeyken kalır (WCAG 1.4.13), Escape ile odağı taşımadan gizlenir ve dokunmatikte uzun basışla görünür (bilgi düğmesine dokunuş). Tooltip hiçbir zaman kullanıcının mutlaka bilmesi gereken bilgiyi taşımaz: bunu etikete, alanın kurallarına ya da sayfaya koyun.

`Popover::new(label).title(title).add(node)`, küçük ayrıntılar için yanında modal olmayan bir diyalog açan bir düğmedir. Odak içine taşınır; Escape, kapatma düğmesi ya da dışına tıklama onu kapatır ve odak düğmeye döner. Dar ekranlarda alttan açılan bir sayfa olarak açılır. Alt öğeleri sıradan node'lardır, bu yüzden içindeki bir form kendi node yoluna, tüm sunucu denetimleriyle birlikte gönderilir.

`Busy::new().add(node)`, içindeki bir istek çalışırken içeriğini kaplar: kaydedilen bir form, bir action düğmesi, yüklenen bir grid ya da tree. Kapsayıcı hemen `aria-busy` alır; döndürücü ve "Working" metni (bir durum mesajı) yalnızca 400 ms sonra ya da `.delay_ms(n)` sonrasında görünür, böylece hızlı yanıtlar göz kırpmaz, bir kez görününce en az 300 ms kalır. Katman fareyi engeller, klavyeyi engellemez ve odak olduğu yerde kalır.

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| Tuş | Nerede | Ne yapar |
|---|---|---|
| Tab | tooltip'i olan bir kontrol | Odak tooltip'i gösterir; ayrılmak gizler |
| Escape | tooltip görünürken herhangi bir yerde | Tooltip'i gizler; odak kalır |
| Enter, Boşluk | popover düğmesi | Popover'ı açar; odak ilk kontrolüne geçer |
| Escape | popover içinde | Kapatır; odak düğmeye döner |
| Tab | popover içinde | Kontrolleri arasında gezdirir; ayrılmak kapatır |

Üçü yaklaşık 2,2 KB gzip betik gerektirir ve kendi istekleri yoktur; tooltip 1 node, popover içeriğiyle birlikte 6, busy katmanı 3 node'dur.

## Açılır menü ve bölünmüş düğmeler

`MenuButton::new(label)` bir komut menüsü açar; `MenuButton::split(label, action, input)` ana kısmından bir action çalıştırır ve menüyü okundan açar. Komutlar `.action(label, action, input)` (ya da başarıdan sonra effect'li `.action_then(...)`), pencere açmak gibi effect'ler için `.item(label, effect)` ve `.separator()` ile eklenir. `.confirm(text)`, son action komutu çalışmadan önce sorar.

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

Action komutları kendi yollarına sahip node'lardır ve bir `ActionButton` gibi form endpoint'ine gönderilir: screen'in erişimi, feature'ın guard'ları, action'ın permission'ı, interceptor'lar, CSRF ve yazma hız sınırı her çağrıda denetlenir ve girdi sunucuda kalır. Kullanıcının çalıştıramayacağı komutlar menüde yer almaz (bölünmüş düğmenin ana action'ı bunun yerine devre dışı bırakılır) ve çağrılsalar bile sunucu yine reddeder.

@live menus

| Tuş | Nerede | Ne yapar |
|---|---|---|
| Enter, Boşluk, Aşağı ok | düğme | Menüyü ilk komutta açar |
| Yukarı ok | düğme | Menüyü son komutta açar |
| Aşağı ok, Yukarı ok | menü | Sonraki ve önceki komut, başa dönerek |
| Home, End | menü | İlk ve son komut |
| Harfler | menü | Onlarla başlayan sonraki komut |
| Enter, Boşluk | bir komut | Çalıştırır ve menüyü kapatır |
| Escape | menü | Kapatır; odak düğmeye döner |
| Tab | menü | Kapatır ve ilerler |

Menü yaklaşık 0,8 KB gzip betik gerektirir ve bir komut çalışana kadar istek yapmaz; 4 node artı komut başına 2 node'dur.
