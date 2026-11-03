# Masaüstü uygulamaları

Bir Zeyn uygulaması tek bir masaüstü çalıştırılabilir dosyası olarak dağıtılabilir. `zeyn-desktop` crate'i uygulamanın kendi sunucusunu süreç içinde başlatır ve sistemin WebView'i ile yerel bir pencerede gösterir:

| Platform | WebView | Durum |
|----------|---------|--------|
| Windows 10 ve 11 | WebView2 (Edge) | derlendi ve elle denetlendi |
| macOS | WKWebView | `wry` üzerinden derleniyor, elle denetlenmedi |
| Linux | WebKitGTK 4.1 | `libwebkit2gtk-4.1-dev` ve `libgtk-3-dev` kuruluysa `wry` üzerinden derleniyor, elle denetlenmedi |

Hiçbir şey makineden çıkmaz. Uygulama kendisi uzak servisleri çağırmadığı sürece çevrimdışı çalışır.

## Kullanımı

Crate'i bir feature'ın arkasına ekleyin, böylece normal sunucu derlemesi bir WebView derlemez:

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.2", optional = true }
```

`App::serve` yerine pencereyi başlatın:

```rust
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    zeyn_desktop::Desktop::new(app()?)
        .id("my-app")
        .title("My App")
        .size(1280, 800)
        .run()
}
```

- `id`, kullanıcı başına veri klasörünü adlandırır ve id başına tek örnek tutar. Varsayılanı çalıştırılabilir dosyanın adıdır.
- `title`, sayfa `document.title` ayarlayana kadar gösterilir; ondan sonra pencere başlığı sayfayı izler.
- `size`, mantıksal piksel cinsinden ilk boyuttur. Sonraki açılışlar son boyutu ve konumu yeniden kullanır.

`run`, pencere kapanana kadar bloklar, sunucuyu durdurur ve süreci bitirir. `ZEYN_DESCRIBE=1` ile uygulama açıklamasını yazdırıp döner, böylece `zeyn check` ve `zeyn routes` çalışmaya devam eder.

Windows'ta release derlemesinin konsol penceresini gizlemek için bunu `main.rs` dosyasının en başına koyun:

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` eksiksiz bir örnektir, bir satış noktası uygulamasıdır:

```
cargo run -p zeyn-example-adisyon --features desktop
```

## Çalıştırılabilir dosyayı derlemek

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

`cargo build --release` komutunu bağlantı zamanı optimizasyonu (link time optimization), tek bir codegen unit ve soyulmuş sembollerle çalıştırır, ardından her çalıştırılabilir dosyanın yolunu ve boyutunu yazdırır. Hız `opt-level = 3` düzeyinde kalır.

### Simge, sürüm bilgisi ve manifest (Windows)

Windows bunları exe içindeki kaynaklardan okur. Yalnızca masaüstü derlemesi için bunları gömen bir derleme betiği (build script) ekleyin:

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`:

```rust
fn main() {
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "My App")
            .set("FileDescription", "My App")
            .set_manifest_file("assets/app.exe.manifest");
        res.compile().unwrap();
    }
}
```

Simgenin kaynak kimliği 1 olmalıdır (`set_icon` varsayılanı); pencere ve görev çubuğu onu kullanır. Sürüm `package.version` değerinden gelir. Monitör başına DPI farkındalığı, UTF-8 ve common controls 6 içeren bir manifest için `examples/adisyon/build.rs` ve `examples/adisyon/assets` dosyalarına bakın. MSVC araç zinciri `rc.exe` dosyasını Windows SDK içinde bulur; GNU araç zinciri `windres` kullanır.

## WebView2 çalışma zamanı

Windows 11 ve güncel Windows 10, WebView2 çalışma zamanı (runtime) ile hazır gelir ve kendini kendisi günceller (evergreen runtime). Exe onu içermez, bu da exe'yi küçük tutar.

Eski ya da kilitli makineler için:

- Microsoft'un evergreen önyükleyicisini (`MicrosoftEdgeWebview2Setup.exe`, yaklaşık 2 MB) yükleyicinizle birlikte dağıtın ve çalışma zamanı eksik olduğunda `/silent /install` ile çalıştırın. Çalışma zamanını bir kez indirir.
- İnterneti olmayan makineler için evergreen bağımsız yükleyiciyi (büyük, çevrimdışı) ya da exe'nin yanına konan sabit sürüm çalışma zamanını kullanın. Sabit sürüm kendini güncellemez, bu yüzden güvenlik güncellemeleri sizin sorumluluğunuzdadır.

Çalışma zamanı olmadan pencere açılamaz ve `run` bir hata döndürür.

## Güvenlik modeli

Sunucuya yalnızca uygulamanın kendi penceresi ulaşabilir.

- **Yalnızca loopback.** Dinleyici, sistemin seçtiği bir portta `127.0.0.1` adresine bağlanır. Hiçbir zaman `0.0.0.0` ya da bir LAN adresine bağlanmaz.
- **Başlatma token'ı.** Her başlatma rastgele 256 bitlik bir token üretir. Pencere önce `/_desktop/start?t=<token>` adresini açar; bu adres token'ı `HttpOnly`, `SameSite=Strict` bir çerez olarak ayarlar ve `/` adresine yönlendirir. Diğer her istek token'ı bu çerezde ya da `x-zeyn-desktop` başlığında taşımalıdır, aksi halde `403` alır. Statik varlıklar da korunur. Token yalnızca bellekte yaşar ve sabit zamanlı karşılaştırılır.
- **Host ve Origin.** `Host` başlığı tam olarak `127.0.0.1:<port>` olmalıdır; bu, DNS rebinding'i engeller (adı 127.0.0.1'e çözülen bir site host olarak kendi adını gönderir). Bir `Origin` başlığı varsa tam olarak `http://127.0.0.1:<port>` olmalıdır. İkisi de başlatma ve odak (focus) endpoint'leri dahil her yola uygulanır.
- **Diğer yerel programlar** porta bağlanabilir ama token'ı bilmez. Diğer kullanıcı hesapları, kullanıcı başına veri klasöründeki instance dosyasını okuyamaz.
- **Gezinme.** Pencere yalnızca kendi origin'ini, kendi `blob:` URL'lerini ve `about:blank` adresini gösterir. `http`, `https` ve `mailto` bağlantıları sistemin varsayılan işleyicisinde açılır; diğer her şema (`file:`, özel protokol işleyicileri, `javascript:`) düşürülür. `window.open` hiçbir zaman ikinci bir WebView oluşturmaz.
- **Devtools** debug derlemelerinde açık, release derlemelerinde kapalıdır.
- **İndirmeler** yalnızca uygulamanın origin'inden yapılabilir ve kullanıcının İndirilenler klasörüne gider, hiçbir dosyanın üzerine yazılmaz (`orders.csv`, `orders (1).csv`, ...). Art arda birkaç indirmeye sormadan izin verilir. Diğer izin istekleri (kamera, mikrofon, pano) WebView'in normal istemini alır.

Guard'ın kapsamadığı durumlar:

- Aynı kullanıcı olarak çalışan bir program süreç belleğini okuyabilir ya da bir hata ayıklayıcı bağlayabilir; hiçbir yerel uygulama buna karşı savunma yapamaz.
- WebView2, `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` ortam değişkenini dikkate alır, bu yüzden kullanıcı için ortam değişkeni ayarlayabilen kişi uzaktan hata ayıklamayı açabilir. Aynı kullanıcı, yukarıdakiyle aynı sınır.
- Uygulamanın kendi kimlik doğrulaması pencere içinde de geçerlidir. Bir masaüstü derlemesindeki `DevAuth`, makinenin başındaki herkese tam erişim verir.

## Davranış

- **Tek örnek.** İlk başlatma, `instance` dosyasını (port ve ayrı bir odak anahtarı) veri klasörüne yazar. İkinci başlatma anahtarı `/_desktop/focus` adresine gönderir; ilk pencere öne gelir ve ikinci süreç yaklaşık 150 ms içinde çıkar. İki başlatma aynı anda başlarsa ikisi de açılabilir.
- **Pencere durumu.** Boyut, konum ve ekranı kaplama (maximized) bayrağı kapanışta `window.json` dosyasına kaydedilir. Artık hiçbir monitörde olmayan kayıtlı bir konum yok sayılır.
- **Kapanış.** Pencereyi kapatmak WebView'i bırakır, sunucuyu düzgünce durdurur (en fazla 2 saniye) ve süreci bitirir. WebView süreçleri de onunla biter.
- **Veri klasörü.** Windows'ta `%LOCALAPPDATA%\<id>`, macOS'ta `~/Library/Application Support/<id>`, Linux'ta `~/.local/share/<id>`. WebView profili (önbellek, local storage) bunun içindeki `webview` klasöründe yaşar, böylece exe Program Files gibi salt okunur bir klasörden de çalışır.
- **Tepsi simgesi yok.** Yerleşik değildir.

## Boyut ve bellek

Windows 11 üzerinde `examples/adisyon` ile, release derlemesi, WebView2 çalışma zamanı 156 ile ölçüldü:

| | |
|---|---|
| `zeyn build-desktop` exe | 6.72 MB (düz release profiliyle 7.25 MB) |
| Uygulama süreci, boşta | 4.3 MB özel (private), 25.6 MB çalışma kümesi (working set) |
| Uygulama süreci ve WebView2 süreçleri, boşta | 7 süreç, yaklaşık 180 MB özel, 370 MB çalışma kümesi |
| `--disable-gpu` ile aynısı | yaklaşık 114 MB özel, 330 MB çalışma kümesi |
| İkinci başlatma, çıkışa kadar | yaklaşık 130 ms |

Baskın olan WebView'dir. Yalnızca GPU süreci yaklaşık 88 MB tutar. Düşük bellekli makinelerdeki basit ekranlar için, yazılım işlemesi pahasına yaklaşık 65 MB tasarruf etmek üzere exe'yi `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` ile başlatın (örneğin bir kısayoldan ya da başlatıcı betikten).
