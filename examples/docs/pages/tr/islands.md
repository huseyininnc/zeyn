# WASM Islands

Zeyn sayfaları sunucuda işler. Bazı yüzeyler bunun için fazla etkileşimlidir: bir canvas editörü, bir diyagram, ağır bir grafik. Bir **island** (ada), sunucunun aksi halde sunucuda işlenmiş bir sayfaya yerleştirdiği, WebAssembly'ye derlenmiş bir Rust bileşenidir. Çevresindeki her şey düz HTML olarak kalır.

Island'lar `zeyn`'in `islands` Cargo feature'ının arkasındadır. Bunu açmayan uygulamalar bunların hiçbirini derlemez ve island içermeyen sayfalar, feature açık olsa bile hiçbir bedel ödemez (aşağıdaki Performans bölümüne bakın).

## 1. Parçalar

| Parça | Nerede | Rolü |
|---|---|---|
| `zeyn::ui::Island` | sunucu | Yer tutucu düğüm: props, yedek içerik, bağlı action'lar |
| `App::islands(dir)` | sunucu | Derlenmiş dosyaları ve yükleyiciyi sunar |
| `zeyn-island` | paylaşılan ve tarayıcı | `Remote` (çağrılabilir bir action'ın sözleşmesi), `Island::call` (istemci) |
| Paylaşılan bir crate | her ikisi | Props, mesajlar ve remote'lar düz serde türleri olarak |
| Bir island crate'i | tarayıcı | `mount(root)` ve isteğe bağlı olarak `unmount(root)` dışa aktaran `cdylib` |
| `zeyn build-islands` | CLI | Island crate'lerini hash'li dosyalara ve bir manifest'e derler |

`examples/showcase` içindeki gibi tipik bir yerleşim:

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. Paylaşılan türler

Paylaşılan crate `serde` ve özellik (feature) olmadan `zeyn-island` crate'ine bağlıdır; böylece sunucu hiçbir tarayıcı kodu derlemez. Bir remote, bir island'ın çağırabileceği bir action'ı adlandırır ve türlerini sabitler:

```rust
#[derive(Serialize, Deserialize)]
pub struct Count { pub value: i64 }

pub struct SaveCount;

impl zeyn_island::Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
```

## 3. Sunucu tarafı

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>` yalnızca action'ın girdisi `R::Input` ve çıktısı `Result<R::Output>` olduğunda derlenir; böylece iki taraf birbirinden kopamaz. Island yalnızca düğümünün bağladığı action'ları çağırabilir.

`fallback` herhangi bir düğümü kabul eder: bir metin özeti, salt okunur bir grid. Island yüklenmeden önce, JavaScript kapalıyken ve yükleme başarısız olduğunda gösterilir. İçindeki grid'ler ve diğer bileşenler her zamanki gibi çalışır.

Yükleme: `.on_interaction()` yer tutucu üzerindeki ilk işaretçi, dokunma ya da odağı bekler, `.on_idle()` tarayıcı boştayken yükler ve varsayılan, yer tutucu görünüm alanının 200 px yakınına geldiğinde yükler. Yükleme sayfayı asla engellemez.

Çıktı klasörünü bir kez kaydedin: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`. Orada manifest yoksa sunucu bir uyarı loglar ve island'lar yedeklerini gösterir.

## 4. Island tarafı

```rust
#[wasm_bindgen]
pub fn mount(root: web_sys::Element) -> Result<(), JsValue> {
    let island = zeyn_island::Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    // build DOM under root, attach listeners
    zeyn_island::spawn(async move {
        match island.call::<SaveCount>(&Count { value: 3 }).await {
            Ok(saved) => { /* ... */ }
            Err(e) => { /* e.message is localized by the server */ }
        }
    });
    Ok(())
}
```

`Island::call`, island'ın screen'i, parametreleri ve düğüm yoluyla birlikte `POST /_zeyn/data` adresine gönderir; remote'un adı `key` olarak, ayrıca girdi gönderilir. Sayfanın CSRF token'ını `x-zeyn-csrf` içinde ve oturum cookie'sini gönderir. Sunucu her bileşen için olduğu gibi denetler: CSRF, screen'in erişimi, Feature'ın guard'ları, action'ın izni; sonra interceptor'ları ve action'ı zaman sınırıyla çalıştırır. İçeri girmenin başka yolu yoktur: bir island, düğümünün bağlamadığı bir action'a ulaşamaz ve island'larla ilgili hiçbir şey bir denetimi atlatmaz.

Hatalar `CallError { status, code, message }` olarak döner. Yerel hatalarda (`network`, `props`, `input`, `answer`) `status` `0`'dır.

Island zamanlayıcı, animasyon karesi ya da soket tutuyorsa `unmount(root)` dışa aktarın. Yükleyici bunu, yer tutucu belgeden çıktığında (gezinme, kapatılan bir pencere) çağırır.

Çeviriler: island'ın gösterdiği metinleri, sunucu tarafından zaten yerelleştirilmiş olarak props içinde geçirin; böylece island bir katalog taşımaz.

## 5. Derleme

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

Her crate için şunları çalıştırır:

1. Boyut ayarları `--config` ile geçirilerek `cargo build --lib --release --target wasm32-unknown-unknown` (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, hata ayıklama bilgisi yok); böylece uygulamanın kendi release profiline dokunulmaz;
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section` (`wasm-bindgen-cli` sürümü `wasm-bindgen` crate'iyle eşleşmelidir, `0.2.130`);
3. binaryen `PATH` üzerindeyse `wasm-opt -Oz`;
4. `{name}.{hash}.js` ve `{name}_bg.{hash}.wasm` dosyalarını yazar (içeriğin 64 bit FNV hash'i) ve `manifest.json` dosyasını günceller.

`zeyn dev`, uygulamanın bir `islands/` klasörü olduğunda her uygulama derlemesinden önce aynı işi yapar ve `dist/` klasörünü izlemez.

Sunucu yalnızca manifest'te listelenen dosya adlarını `/_zeyn/islands/{file}` adresinde, `application/wasm` veya `text/javascript` ve `Cache-Control: public, max-age=31536000, immutable` ile sunar. Dosyalar istek başına diskten okunur; bu yüzden sunucu belleğine maliyetleri yoktur.

## 6. Content Security Policy

WebAssembly derlemek `script-src` içinde `'wasm-unsafe-eval'` gerektirir. Zeyn bunu **yalnızca** screen'i bir island içeren sayfalara ekler; diğer her sayfa `docs/runtime-protocol.md` içindeki katı politikayı korur. `'wasm-unsafe-eval'` `WebAssembly.compile` ve `instantiate` işlemlerine izin verir, başka hiçbir şeye: `eval`, `new Function` ve satır içi betikler engelli kalır ve modüller yine yalnızca uygulamanın origin'inden gelir.

Bir politika bir belgeye aittir ve sonradan gevşetilemez. Bu yüzden `nav.js`, bir island içeren bir screen'i (kısmi cevap `x-zeyn-islands: 1` taşır) onsuz yüklenmiş bir belgeye yüklerken tam sayfa yüklemesine döner. Bir belge island politikasına sahip olduktan sonra, island'lı ve düz screen'ler arasında geçiş yerinde kalır.

Sunucu island'ları `data-zeyn-island="` özniteliğinden bulur; kaçırılmış kullanıcı verisi o tırnağı asla üretemez, bu yüzden bir kullanıcının yazdığı metin politikayı açamaz.

## 7. Canvas yüzeyleri

`zeyn::ui::Canvas::new(name, description)` çizim için bir island'dır. Açıklama zorunludur: bir canvas yardımcı teknolojiye görünmezdir; bu yüzden sayfa neyi gösterdiğini ve hangi tuşların onu çalıştırdığını söylemelidir. Sunucu bunu island'ın üzerinde bir paragraf olarak işler ve yer tutucunun `aria-describedby` değerini ona yöneltir.

Island tarafında, `canvas` feature'ı ile `zeyn-island`, `canvas::Surface` sağlar:

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| Konu | Yüzeyin yaptığı |
|---|---|
| Grafik | `Prefer::Gl` WebGL 2'yi dener ve 2D'ye döner; `webgpu_available()`, WebGPU'yu `surface.canvas()` üzerinde kendileri kuran sahneler için `navigator.gpu` olup olmadığını söyler |
| Yüksek DPI | Arka bellek `devicePixelRatio` ve öğe boyutunu izler (`ResizeObserver`); 2D çizim önceden ölçeklenir, bu yüzden sahne CSS pikselleriyle çalışır |
| Girdi | İşaretçi basma, hareket ve bırakma (işaretçi yakalama ile), tekerlek (satır ve sayfa deltaları piksellere çevrilir) ve tuşlar, CSS pikseli cinsinden `Input` değerleri olarak |
| Kareler | `requestAnimationFrame` yalnızca sahnenin kullandığı bir girdiden, bir `update`'ten, bir yeniden boyutlandırmadan sonra ya da `Scene::animating` `true` döndürdüğü sürece çalışır. Durağan bir resim CPU harcamaz |
| Varsayılanlar | Sahnenin kullandığı bir girdinin varsayılanı engellenir (oklar sayfayı kaydırmaz); odağın ayrılabilmesi için Tab için `false` döndürün |
| Temizlik | `destroy()` dinleyicileri, gözlemciyi ve canvas'ı kaldırır; `unmount` içinden çağırın |

Canvas; `tabindex="0"`, `role="application"`, etiket ve `aria-describedby` alır.

Showcase diyagram editöründeki gibi erişilebilirlik deseni (`examples/showcase/islands/diagram`):

- **Canvas üzerinde klavye:** `]` ve `[` şekilleri seçer, oklar onları taşır, Delete kaldırır, `+`, `-` ve `0` yakınlaştırır, Escape seçimi temizler. Tuşlar yalnızca canvas odaktayken çalışır.
- **Sıradan kontroller:** bir düğme araç çubuğu aynı işleri canvas olmadan yapar.
- **Metin özeti:** canvas'ın altındaki bir liste her şekli anlatır ve her değişikliği izler; bir `role="status"` satırı seçimi ve kayıt sonuçlarını duyurur.
- **Yedek:** sunucu aynı özeti işler; böylece WebAssembly olmayan kullanıcılar da diyagramı okur.

Editör `SaveDiagram` üzerinden kaydeder ve sunucu aldığını sınırlar (`Diagram::clamped`: en fazla 200 şekil, benzersiz kimlikler, 40 karakterlik etiketler, sınırlı koordinatlar); çünkü island'a asla güvenilmez.

## 8. Performans

| Ne | Sayı |
|---|---|
| Düz sayfa HTML'i, istekler, politika | değişmez: meta yok, yükleyici yok, ek istek yok |
| `nav.js` | +281 bayt (tam yükleme yedeği) |
| `/_zeyn/islands.js` yükleyicisi, yalnızca island sayfalarında | 3,9 KB, 1,7 KB gzip |
| Counter island `.wasm` (wasm-opt yok) | 127,6 KB, 52,8 KB gzip |
| Counter island glue `.js` | 18,0 KB, 4,2 KB gzip |
| Diyagram editörü `.wasm` (canvas yüzeyi, wasm-opt yok) | 169,4 KB, 67,8 KB gzip |
| Canlı düzenlemeli diyagram editörü (`yrs` ekler, bkz. `docs/realtime.md`) | 408,4 KB, 144,3 KB gzip |
| Diyagram editörü glue `.js` | 30,3 KB, 6,1 KB gzip |
| Island başına sunucu belleği | yok; dosyalar diskten akar, props istek başına işlenir |

Counter'ın boyutunun çoğu `serde_json` ve `core`'un biçimlendirme kodundan gelir. Bunları paylaşan island'lar her biri için bir kez öder; wasm-opt genellikle yüzde 10 ile 20 daha düşürür.

## 9. Sınırlar

- Bir pencere (`open_window`) içindeki island, açan sayfanın island politikasına sahip olmasını gerektirir; island'lı pencereleri island sayfalarına yerleştirin ya da screen olarak açın.
- `Closure::forget` ile eklenen closure'lar sayfa kadar yaşar; bir island'ın çok kez mount edilebildiği durumlarda bunları `unmount` içinde serbest bırakın.
- Island'lar sunucuda kendileri hiçbir şey işlemez; arama motorlarının, yüklenmeden önceki ekran okuyucuların ve JavaScript'siz kullanıcıların aldığı şey yedektir. Onu anlamlı yapın.
