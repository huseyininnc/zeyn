# Medya

Zeyn medya dosyalarını HTTP range istekleriyle sunar ve `Video` ile `Audio` bileşenlerinde oynatır. Kod çözmeyi yerel `<video>` ve `<audio>` öğeleri yapar; Zeyn bunların üstüne kendi erişilebilir kontrollerini çizer.

## Dosya sunmak

Bir `MediaSource`'u izinle birlikte bir feature'a kaydedin:

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

Tarayıcı `GET /_zeyn/media/{source}/{id}` adresini getirir. Kaynak herhangi bir şey görmeden önce sunucu şunları yapar:

1. 1 ila 128 karakter olmayan, `A-Z a-z 0-9 - _ .` dışında karakter içeren, nokta ile başlayan ya da `..` içeren id'leri reddeder (`404`);
2. kaynağın iznini (`403`) ve feature'ın guard'larını denetler;
3. interceptor'ları çalıştırır (`CallKind::Media`, kaynak adı `screen` içinde).

`open(ctx, id)`, id'yi uygulamanın kendi kataloğunda arar ve bir `Media` döndürür: diskteki bir dosya için `Media::file(path)` (tür uzantıdan, entity tag boyut ve değiştirilme zamanından), bellekteki veri için `Media::bytes(data, type)` ya da herhangi bir `Read + Seek` için `Media::new(reader, len, type)`. Id bir addır, hiçbir zaman yol değildir: yolu id'den değil, kendi aramanızdan oluşturun.

Yanıtlar:

| İstek | Yanıt |
|---|---|
| `Range` yok, başka bir birim ya da birden çok aralık | tüm dosyayla `200` |
| `bytes=a-b`, `bytes=a-`, `bytes=-n` | `Content-Range: bytes a-b/len` ile `206` |
| hatalı biçimli ya da sonun ötesinde başlayan bir bytes aralığı | `Content-Range: bytes */len` ile `416` |
| entity tag ile eşleşmeyen `If-Range` | tüm dosyayla `200` |
| entity tag ile eşleşen `If-None-Match` | `304` |

Her yanıtta `Accept-Ranges: bytes`, `Cache-Control: private, no-cache` ve `X-Content-Type-Options: nosniff` bulunur. Yalnızca ses, video, raster görüntüler, WebVTT ve akış oynatma listeleri satır içi (inline) sunulur; diğer her tür (HTML, SVG, betikler) `application/octet-stream` olarak ve `Content-Disposition: attachment` ile gönderilir, böylece bir dosya hiçbir zaman uygulamanın bir sayfası olarak çalışamaz. Dosyalar engelleyici havuzda (blocking pool) 64 KiB'lık parçalar halinde okunur ve en fazla dört parça kuyrukta bekler; böylece bellek her dosya boyutu için sabit kalır ve ayrılan bir istemci okumayı durdurur. Hız sınırları diğer isteklerdeki gibi uygulanır; medya okumaları yazma sayılmaz.

İçerik güvenlik politikasında (CSP) değişiklik gerekmez: `default-src 'self'` zaten uygulamadan medyaya izin verir. Media Source Extensions kullanan bir uyarlanabilir akış kitaplığı `media-src` içinde `blob:` ister, Zeyn bunu eklemez.

## Herkese açık dosyalar: görseller, logolar, indirmeler

Oturum açmış olsun olmasın herkesin görebileceği dosyalar herkese açık bir kaynağa konur:

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}` izin denetimini atlar (id'ler, guard'lar ve interceptor'lar yine çalışır), çerez ayarlamaz ve `Cache-Control: public, max-age=86400` ile yanıt verir (en fazla `MAX_PUBLIC_MEDIA_CACHE`, bir yıl); böylece tarayıcılar ve uygulamanın önündeki bir CDN onu saklar. Entity tag yine de `304` ile yeniden doğrulamalarına izin verir. Yolu bir `Image`'ın `src` değeri olarak kullanın. Çok sayıda büyük görsel için bir CDN arkasında ayrı bir görsel sunucusu hâlâ daha ucuz seçimdir; buna `App::csp(|c| c.img_src([...]))` ile izin verin.

- `open` her ziyaretçi için aynı baytları döndürmelidir. Herkese açık bir dosyayı hiçbir zaman kullanıcıya, role, tenant'a ya da dile göre değiştirmeyin: bir CDN ya da proxy ilk yanıtı saklar ve herkese verir. Kullanıcıya göre değişen dosyalar `Feature::media` ile kaydedilmiş bir kaynağa aittir.
- Uzun bir `max_age` ile, içerikle değişen id'ler verin; örneğin bir içerik özeti (hash) (`poster-3f9a2c.jpg`). Böylece değişen bir dosya yeni bir adres alır, eski kopyalar süreleri dolana kadar önbelleklerde yaşamaz.
- Herkese açık kaynakların okumaları (`GET` ve `HEAD`) `App::rate_limit` sınırına sayılmaz: bir sayfa düzinelerce afişi yükleyebilir ve birçok mobil ziyaretçi operatör NAT'ı arkasında tek bir adresi paylaşır. Özel kaynaklar yine sayılır ve IP izin ve engelleme kuralları ikisine de uygulanmaya devam eder. Bant genişliği kötüye kullanımını sınırlamak, uygulamanın önündeki bir CDN ya da ters proxy'nin işidir.

## Oynatıcılar

```rust
Video::new("films", t!("films.harbor"))
    .subtitle(t!("films.harbor_sub"))
    .file("harbor.webm")
    .file("harbor.mp4")
    .hls("harbor.m3u8")
    .poster("harbor.jpg")
    .captions("harbor.en.vtt", "en", "English")
    .captions("harbor.tr.vtt", "tr", "Türkçe")

Audio::new("music", t!("songs.dawn")).subtitle(t!("songs.artist")).file("dawn.mp3")
```

Kontroller: oynat ve duraklat, bir arama kaydırıcısı, süre, sessize alma, bir ses kaydırıcısı, bir oynatma hızı menüsü, bir altyazı düğmesi ve (birden çok altyazı dosyası varsa) bir dil menüsü; video için ayrıca resim içinde resim (tarayıcı destekliyorsa) ve tam ekran. Kaydırıcılar WAI-ARIA slider desenini izler: oklar 5 saniye (ya da ses düzeyinin yüzde 5'i) kadar hareket ettirir, Page Up ve Page Down onda biri kadar, Home ve End uçlara gider. Menüler menu button desenini izler. Odak bir oynatıcının içindeyken `k` ya da Space oynatır ve duraklatır, `j` ve `l` 10 saniye atlar, `m` sessize alır, `c` altyazıyı açıp kapatır ve `f` tam ekranı açıp kapatır; bu tuşlar sayfanın başka yerinde hiçbir şey yapmaz.

Altyazılar aynı kaynaktan sunulan WebVTT dosyalarıdır. Oynatıcı etkin ipucunu (cue) düz metin olarak çizer (cue işaretlemesi atılır): video için görüntünün üstüne, ses için başlığın altına; böylece altyazılar her tarayıcıda ve tam ekranda aynı görünür.

## Kaldığı yerden devam

Oynatıcı kullanıcının durduğu yeri kaydeder: oynarken her 15 saniyede bir, duraklatıldığında, sayfa gizlendiğinde ya da kapatıldığında ve medya bittiğinde `0` olarak. Kayıtlı nokta uçlardan herhangi birine 5 saniyeden fazla uzaksa oradan devam eder.

- Kaynak `position` ve `save_position` uyguladığında konumlar sunucuya gider: `GET /_zeyn/media/position?source=&id=` `{"at": seconds, "kept": true}` yanıtını verir ve `{source, id, at}` ile `POST /_zeyn/media/position` kaydeder (`204`). İkisi de id'yi, izni ve guard'ları denetler ve oturum açmış bir kullanıcı gerektirir; POST ayrıca CSRF başlığı ister ve hız sınırları için yazma sayılır. `MAX_POSITION_SECONDS` üzerindeki ya da sıfırın altındaki konumlar `422` döner.
- Aksi halde (kaynağın varsayılan metotları ya da anonim bir kullanıcı) okuma `{"at": null, "kept": false}` yanıtını verir ve oynatıcı konumu tarayıcının local storage'ında `zeyn.media.{source}/{id}` anahtarı altında tutar. Depolama hataları (gizli pencereler, engellenmiş depolama) yok sayılır.

Oturum açmış kullanıcılar için sunucu konumları daha iyi seçimdir: kullanıcıyı her cihaza taşır ve uygulama bunları bir "izlemeye devam et" satırı için okuyabilir. Local storage kod gerektirmez ama tek bir tarayıcıda kalır ve onu kullanan herkes tarafından paylaşılır.

## HLS ve DASH

`hls(id)` ve `dash(id)` uyarlanabilir bir akış ekler. Safari HLS'yi yerel olarak oynatır, bu yüzden oynatma listesini kullanır. Diğer tarayıcılar Zeyn'in içermediği bir JavaScript kitaplığı (hls.js ya da dash.js gibi) ister. Aşamalı (progressive) dosyalara dönmeden önce oynatıcı kök öğesinde iptal edilebilir bir `zeyn:media-adaptive` olayı gönderir:

```js
document.addEventListener('zeyn:media-adaptive', (e) => {
  const { media, kind, url } = e.detail;
  if (kind !== 'hls' || !window.Hls?.isSupported()) return;
  e.preventDefault();
  const hls = new window.Hls();
  hls.loadSource(url);
  hls.attachMedia(media);
});
```

Bu betiği uygulamanızın bir dosyasından yükleyin (satır içi betikler içerik güvenlik politikasıyla engellenir). Hiçbir dinleyici `preventDefault()` çağırmadığında `file` girdileri sırayla oynatılır. Oynatma listesi ve segment dosyaları aynı kaynaktan sunulur, bu yüzden id'leri onda listelendiği sürece göreli URL'leri çalışır.

## Yüklenen dosyalar

`Form::file` ile yüklenen dosyalar, store'un izninden, feature'ın guard'larından ve bir tenant denetiminden sonra `GET /_zeyn/files/{store}/{id}` ile medya dosyalarıyla aynı yanıtlarla sunulur (aralıklar, entity tag'ler, `nosniff`, yalnızca raster görüntüler, ses ve video için satır içi). `examples/docs/pages/forms.md` içindeki dosya yükleme bölümüne bakın.
