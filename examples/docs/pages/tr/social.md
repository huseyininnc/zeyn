# Sosyal

Sosyal sayfalar için bileşenler. Her biri sunucuda çizilir ve stil dosyasını ve betiğini yalnızca onu kullanan sayfalarda yükler.

## StoryRing

`StoryRing`, bir kişinin hikayelerinin izleyen için yeni olup olmadığını gösteren, halka içindeki bir avatardır. `StoryState::Unseen` marka renklerinde kalın bir halka, `StoryState::Seen` ince ve nötr bir halka çizer. Durum sunucudan gelir, yani uygulamanın izleyici hakkında sakladığı bilgiden; tarayıcı onu hiç değiştirmez. `StoryRow` halkaları, bir ray gibi yana kayan bir satırda toplar.

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

`.link(screen, params)` verilirse halka, hikayeleri gösteren ekrana giden bir bağlantıdır. Erişilebilir adı kişinin adı ve durumudur, örneğin "Ada Lovelace, görülmemiş hikayeler"; halkanın altındaki görünen ad da aynı biçimde başlar, böylece sesle kontrol kullananlar onu söyleyebilir. İki durum yalnızca renkte değil halkanın kalınlığında da ayrışır; zorunlu renkler kipinde görülmüş halka kesikli çizilir. Resim `Avatar` kurallarına uyar (uygulamanın bir yolu veya izin verilen bir `https` adresi); resim yoksa baş harfler gösterilir.

Halka yalnızca CSS ile çizilir: betik yok, resim yok. `stories.css` (2,2 KB) yalnızca halka içeren sayfalarda yüklenir. Bir satır en fazla `MAX_STORY_RINGS` (100) halka alır. Dokunma, izleme dörtgeni ve Shift+tekerlek satırı kaydırır, halkalar yerine oturur ve klavye odağı alan halka görünür alana kayar. Bir satırın hiçbir halkasında bağlantı yoksa odağı satırın kendisi alır; böylece klavye yine de onu kaydırabilir.

| Tuş | Ne yapar |
|---|---|
| Tab | Sonraki halkaya geçer ve onu görünür alana kaydırır |
| Enter | Odaktaki halkanın hikayelerini açar |
| Left, Right | Odak satırın kendisindeyken: satırı kaydırır |


## Lightbox

Her `Image` `.lightbox()` ile tam ekran açılır: görseli ekranın izin verdiği kadar büyük, açıklaması ve yakınlaştırmasıyla gösteren bir modal iletişim kutusu. `.full(src)` lightbox'ın gösterdiği daha büyük dosyayı verir, sayfa küçük olanı tutar. `.caption(text)` altına bir açıklama koyar (ikisi de lightbox'ı açar). `.video(source, file)` görseli bir medya kaynağındaki videonun posteri yapar ve lightbox o videoyu oynatır. Bir gönderinin ya da `Row`, `Stack` veya `AutoGrid` gibi tek bir kapsayıcının görselleri bir küme oluşturur: ok tuşları ve kaydırma hareketleri aralarında geçiş yapar.

```rust
AutoGrid::new().min(140)
    .add(Image::new(p.thumb.clone()).alt(p.title.clone()).size(300, 300)
        .full(p.photo.clone())
        .caption(p.title.clone()))
    .add(Image::new(clip.poster.clone()).alt(clip.title.clone()).size(320, 180)
        .video("films", clip.file.clone()))
```

Script yokken görsel kendi dosyasına bir bağlantıdır, yani yine açılır. Script varken `lightbox.js` (13 KB, 4,4 KB gzip) ve `lightbox.css` (4 KB) yalnızca lightbox görseli olan sayfalarda yüklenir ve iletişim kutusu ilk açılışta kurulur. İletişim kutusunda yalnızca açık öğe bulunur. Komşularının daha büyük dosyaları görsel olarak önceden çekilir, videolar asla çekilmez. Küçük görsel hemen, bulanık olarak görünür ve büyük dosya yüklenince onu kaplar. Açılış masaüstünde yaklaşık 16 ms sürer.

Büyük dosya görselin adres kurallarına uyar (uygulamanın bir yolu ya da izin verilen bir `https` adresi). Bu kuralları bozarsa görselin kendi dosyası kullanılır. Video dosyası kaynağın geçerli bir medya kimliği olmalıdır. Kaynak her istekte izni denetler ve aralıklı (range) sunum yapar. İletişim kutusu W3C APG iletişim kutusu desenini izler: odak kapat düğmesine geçer, Tab ve Shift+Tab kutunun içinde kalır, kapatınca odak onu açan görsele döner (feed bu arada görseli boşaltmışsa gönderisine). Her görsel bağlantısının adı alt metnidir, alt metin boşsa "Resmi aç" olur. Konum ("2 / 4") duyurulur. Hareket `prefers-reduced-motion` ayarına uyar ve iletişim kutusu açıkken arkadaki sayfa kaymaz.

| Key | What it does |
|---|---|
| Enter | Görselde: lightbox'ı açar |
| Right, Left | Sonraki ve önceki öğe (sağdan sola yazımda ters); yakınlaştırılmış görseli kaydırır |
| Up, Down | Yakınlaştırılmış görseli kaydırır |
| Home, End | İlk ve son öğe |
| +, -, 0 | Yakınlaştırır, uzaklaştırır, yakınlaştırmayı sıfırlar |
| Escape | Lightbox'ı kapatır ve odağı geri verir |
| Tab, Shift+Tab | İletişim kutusundan çıkmadan düğmeleri arasında gezdirir |

Çift tıklama, çift dokunma ve yakınlaştırma düğmesi de yakınlaştırır, yakınlaştırılmış görsel sürüklenerek kaydırılır.

## Feed

`Feed`, en yeni en üstte olacak şekilde sonsuz bir gönderi akışıdır. Satırlarını `CardGrid`'in kullandığı türden bir eylemle yükler (girişte imleçli bir `DataQuery`, çıkışta bir `Page`); akışın sırası eylemin sırasıdır, grup boyutunu sunucu belirler (`.batch(n)`, ayarlanmazsa 10, en fazla `MAX_FEED_BATCH`, 50) ve `.scope(field, value)` istemcinin değiştiremeyeceği bir süzgeci sabitler. Her satır bir `Post` olur: avatarı ve kullanıcı adıyla yazar, zaman, metin, lightbox'ta açılan en fazla `MAX_POST_MEDIA` (4) görsel, akışın tepkileri ve gönderinin sayıları, yorum sayısı ve bir paylaş düğmesi.

```rust
Feed::of(search_posts)
    .title(t!("social.feed"))
    .post(|p: &Story| Post::new(p.author.clone(), p.at)
        .avatar(p.avatar.clone())
        .text(p.text.clone())
        .media(Image::new(p.thumb.clone()).alt(p.alt.clone()).size(300, 300).full(p.photo.clone()))
        .reactions(p.reactions())
        .comments(p.comments)
        .link(story, StoryParams { id: p.id }))
    .reactions(react_to_post)
    .reaction("like", t!("social.like"), "\u{1f44d}")
    .topic("posts")
```

Akış W3C APG feed desenini izler: her biri odaklanabilen, yazarıyla adlandırılan, metniyle betimlenen ve `aria-posinset` ile numaralanan `<article>` öğelerinden oluşan bir `role="feed"` bölgesi (daha fazlası yüklenebilirken `aria-setsize` -1'dir). Bir grup yüklenirken `aria-busy` ayarlanır. Okuyucu sona yaklaştığında eski gönderiler yüklenir, yedek olarak "Daha fazla göster" bulunur. Her metin kaçışlanır ve bir gönderinin metni satır sonlarını korur.

Yeni gönderiler okuyucunun gördüğünü asla kaydırmaz. `.topic(..)` ile verilen bir konu yenilendiğinde (`ui.refresh("posts")` ile biten bir eylem ya da `.live()` ile gerçek zamanlı hub), akış ilk grubunu ister ve en yeni gönderisinin üstündekileri bekletir. Akışın üstündeki bir düğme "3 yeni gönderi" der, bir durum bölgesi de aynısını söyler; düğmeye basmak onları kaydırma konumunu koruyarak üste ekler ve odağı ilk yeni gönderiye taşır. Bir grubun alabileceğinden fazla yeni gönderi geldiğinde düğme akışı en yeniden yeniden başlatır.

Bellek sınırlı kalır. Yalnızca görünüm alanına yakın gönderiler sayfadadır; diğerleri sahip oldukları yükseklikle HTML olarak saklanır ve kapladıkları yer dolgudur, böylece geri geldiklerinde hiçbir şey zıplamaz. Bir gönderi sayfada olduğu sürece öğesini korur, böylece odak ve satır anahtarı yerinde kalır; uzağa kaydırılan odaklı bir gönderi kendi yuvasının üzerinde sayfada kalır. 1.000 gönderiden sonra okuyucunun çok gerisindeki en eski gönderiler atılır ve düğme en yeniye dönmenin yolunu sunar.

Bir tepki basışı gönderinin anahtarını yollar. Sunucu tepki eylemini (izni, CSRF denetimi ve yazma hız sınırıyla) ancak akışın kendi eylemi o gönderiyi akışın kapsamıyla kullanıcıya döndürdükten sonra çalıştırır; böylece kimse okuyamadığı bir gönderiye tepki veremez. Eylem anahtarı `ReactionPress::target` olarak alır. `feed.js` (15,7 KB, 5,3 KB gzip) ve `feed.css` (3,6 KB) yalnızca akış içeren sayfalarda yüklenir.

| Key | What it does |
|---|---|
| Tab | Bir gönderiye girer, sonra bağlantıları ve düğmeleri arasında gezdirir |
| Page Down, Page Up | Odağı sonraki ya da önceki gönderiye taşır, sonda daha fazlasını yükler |
| Control+End | Odağı akıştan sonraki ilk öğeye taşır |
| Control+Home | Odağı akıştan önceki son öğeye taşır |

Bileşen kataloğunun sosyal demosunda (kimliklerinden türetilen 10.000 gönderi, üçte biri fotoğraflı) Chrome'da 1440 x 900'de, 10'luk gruplar halinde yüklenirken 2.000 gönderi boyunca kaydırılarak ölçüldü: tam CPU'da 49 fps ve p95 kare süresi 44 ms, belgede en fazla 17 gönderi ve 883 öğe, 4,6 MB JavaScript yığını.
