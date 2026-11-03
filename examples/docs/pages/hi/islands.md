# WASM Islands

Zeyn पृष्ठ सर्वर पर रेंडर करता है। कुछ सतहें इसके लिए बहुत इंटरैक्टिव हैं: कैनवास एडिटर, डायग्राम, भारी चार्ट। **island** WebAssembly में कंपाइल किया गया Rust कंपोनेंट है जिसे सर्वर बाकी सर्वर-रेंडर किए पृष्ठ पर रखता है। उसके आसपास सब कुछ सादा HTML रहता है।

Islands `zeyn` के `islands` Cargo feature के पीछे हैं। जो ऐप इसे चालू नहीं करते वे इसका कुछ कंपाइल नहीं करते, और island के बिना पृष्ठ चालू होने पर भी कुछ नहीं चुकाते (नीचे प्रदर्शन देखें)।

## 1. हिस्से

| हिस्सा | कहाँ | भूमिका |
|---|---|---|
| `zeyn::ui::Island` | सर्वर | प्लेसहोल्डर नोड: props, फ़ॉलबैक सामग्री, बँधी action |
| `App::islands(dir)` | सर्वर | बनी फ़ाइलें और लोडर परोसता है |
| `zeyn-island` | साझा और ब्राउज़र | `Remote` (कॉल योग्य action का अनुबंध), `Island::call` (क्लाइंट) |
| एक साझा crate | दोनों | props, संदेश और remote सादे serde टाइप के रूप में |
| एक island crate | ब्राउज़र | `cdylib` जो `mount(root)` और वैकल्पिक रूप से `unmount(root)` एक्सपोर्ट करता है |
| `zeyn build-islands` | CLI | island crate को हैश वाली फ़ाइलों और मैनिफ़ेस्ट में बनाता है |

एक सामान्य लेआउट, जैसा `examples/showcase` में:

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. साझा टाइप

साझा crate `serde` और बिना feature के `zeyn-island` पर निर्भर है, इसलिए सर्वर कोई ब्राउज़र कोड कंपाइल नहीं करता। remote उस action को नाम देता है जिसे island कॉल कर सकता है और उसके टाइप तय करता है:

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

## 3. सर्वर की ओर

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>` केवल तब कंपाइल होता है जब action का इनपुट `R::Input` हो और आउटपुट `Result<R::Output>`, इसलिए दोनों ओर अलग नहीं हो सकते। island केवल वही action कॉल कर सकता है जिन्हें उसका नोड बाँधता है।

`fallback` कोई भी नोड लेता है: टेक्स्ट सारांश, केवल पढ़ने योग्य ग्रिड। यह island लोड होने से पहले, जब JavaScript बंद हो और जब लोडिंग विफल हो तब दिखता है। इसके ग्रिड और अन्य कंपोनेंट सामान्य रूप से काम करते हैं।

लोडिंग: `.on_interaction()` प्लेसहोल्डर पर पहले पॉइंटर, टच या फ़ोकस का इंतज़ार करता है, `.on_idle()` तब लोड करता है जब ब्राउज़र निष्क्रिय हो, और डिफ़ॉल्ट तब लोड करता है जब प्लेसहोल्डर व्यूपोर्ट के 200 px के भीतर आता है। लोडिंग कभी पृष्ठ को नहीं रोकती।

आउटपुट फ़ोल्डर एक बार पंजीकृत करें: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`। वहाँ मैनिफ़ेस्ट के बिना सर्वर चेतावनी लॉग करता है और islands अपना फ़ॉलबैक दिखाते हैं।

## 4. island की ओर

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

`Island::call` `POST /_zeyn/data` पर island की Screen, params और नोड पथ, `key` के रूप में remote का नाम और इनपुट के साथ पोस्ट करता है। यह पृष्ठ का CSRF टोकन `x-zeyn-csrf` में और सत्र कुकी भेजता है। सर्वर हर कंपोनेंट की तरह जाँचता है: CSRF, Screen का एक्सेस, feature के गार्ड, action की अनुमति, फिर इंटरसेप्टर और action को समय सीमा के साथ चलाता है। अंदर आने का कोई दूसरा रास्ता नहीं है: island ऐसी action तक नहीं पहुँच सकता जिसे उसका नोड नहीं बाँधता, और islands से जुड़ा कुछ भी किसी जाँच को नहीं लाँघता।

त्रुटियाँ `CallError { status, code, message }` के रूप में लौटती हैं। स्थानीय विफलताओं (`network`, `props`, `input`, `answer`) के लिए `status` `0` है।

जब island टाइमर, एनिमेशन फ़्रेम या सॉकेट रखे तो `unmount(root)` एक्सपोर्ट करें। लोडर इसे तब कॉल करता है जब प्लेसहोल्डर दस्तावेज़ छोड़ता है (नेविगेशन, बंद Window)।

अनुवाद: island जो टेक्स्ट दिखाता है उन्हें अपने props में दें, सर्वर द्वारा पहले से स्थानीयकृत, ताकि island कोई कैटलॉग न भेजे।

## 5. बिल्ड

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

हर crate के लिए यह चलाता है:

1. `cargo build --lib --release --target wasm32-unknown-unknown`, आकार सेटिंग `--config` के रूप में देकर (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, कोई डिबग जानकारी नहीं), इसलिए ऐप की अपनी रिलीज़ प्रोफ़ाइल अछूती रहती है;
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section` (`wasm-bindgen-cli` संस्करण `wasm-bindgen` crate से मेल खाना चाहिए, `0.2.130`);
3. `wasm-opt -Oz` जब binaryen `PATH` पर हो;
4. `{name}.{hash}.js` और `{name}_bg.{hash}.wasm` लिखता है (सामग्री का 64 बिट FNV हैश) और `manifest.json` अपडेट करता है।

`zeyn dev` हर ऐप बिल्ड से पहले यही करता है जब ऐप में `islands/` फ़ोल्डर हो, और `dist/` की निगरानी नहीं करता।

सर्वर केवल मैनिफ़ेस्ट में सूचीबद्ध फ़ाइल नाम परोसता है, `/_zeyn/islands/{file}` पर, `application/wasm` या `text/javascript` और `Cache-Control: public, max-age=31536000, immutable` के साथ। फ़ाइलें प्रति अनुरोध डिस्क से पढ़ी जाती हैं, इसलिए वे सर्वर मेमोरी नहीं लेतीं।

## 6. Content Security Policy

WebAssembly कंपाइल करने के लिए `script-src` में `'wasm-unsafe-eval'` चाहिए। Zeyn इसे **केवल** उन पृष्ठों में जोड़ता है जिनकी Screen में island हो; हर अन्य पृष्ठ `docs/runtime-protocol.md` की सख़्त नीति रखता है। `'wasm-unsafe-eval'` `WebAssembly.compile` और `instantiate` की अनुमति देता है और कुछ नहीं: `eval`, `new Function` और इनलाइन स्क्रिप्ट अवरुद्ध रहते हैं, और मॉड्यूल अब भी केवल ऐप के origin से आते हैं।

नीति दस्तावेज़ की होती है और बाद में ढीली नहीं की जा सकती। इसलिए जब `nav.js` island वाली Screen (आंशिक उत्तर में `x-zeyn-islands: 1` होता है) को ऐसे दस्तावेज़ में लोड करता है जो उसके बिना लोड हुआ था, तो वह पूरे पृष्ठ लोड पर लौटता है। जब दस्तावेज़ के पास island नीति हो जाती है, island और सादी Screen के बीच जाना उसी जगह रहता है।

सर्वर islands को एट्रिब्यूट `data-zeyn-island="` से खोजता है; escape किया उपयोगकर्ता डेटा कभी उद्धरण नहीं बना सकता, इसलिए उपयोगकर्ता का टाइप किया टेक्स्ट नीति को चालू नहीं कर सकता।

## 7. कैनवास सतहें

`zeyn::ui::Canvas::new(name, description)` चित्र बनाने के लिए island है। विवरण आवश्यक है: कैनवास सहायक तकनीक के लिए अदृश्य है, इसलिए पृष्ठ को बताना होगा कि वह क्या दिखाता है और उसे कौन सी कुंजियाँ चलाती हैं। सर्वर इसे island के ऊपर अनुच्छेद के रूप में रेंडर करता है और प्लेसहोल्डर के `aria-describedby` को उसकी ओर इंगित करता है।

island की ओर, `canvas` feature के साथ `zeyn-island` `canvas::Surface` देता है:

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| चिंता | सतह क्या करती है |
|---|---|
| ग्राफ़िक्स | `Prefer::Gl` WebGL 2 आज़माता है और 2D पर लौटता है; `webgpu_available()` बताता है कि `navigator.gpu` मौजूद है या नहीं, उन दृश्यों के लिए जो `surface.canvas()` पर WebGPU खुद सेट करते हैं |
| हाई DPI | बैकिंग स्टोर `devicePixelRatio` और एलिमेंट आकार (`ResizeObserver`) का पालन करता है; 2D ड्रॉइंग पहले से स्केल की जाती है, इसलिए दृश्य CSS पिक्सेल में काम करता है |
| इनपुट | पॉइंटर डाउन, मूव और अप (पॉइंटर कैप्चर के साथ), व्हील (पंक्ति और पृष्ठ डेल्टा पिक्सेल में बदले गए) और कुंजियाँ, CSS पिक्सेल में `Input` मान के रूप में |
| फ़्रेम | `requestAnimationFrame` केवल उस इनपुट के बाद जो दृश्य ने इस्तेमाल किया, `update`, आकार बदलने, या जब तक `Scene::animating` `true` लौटाए। स्थिर चित्र CPU नहीं लेता |
| डिफ़ॉल्ट | जो इनपुट दृश्य इस्तेमाल करता है उसका डिफ़ॉल्ट रोका जाता है (तीर कुंजियाँ पृष्ठ स्क्रॉल नहीं करतीं); Tab के लिए `false` लौटाएँ ताकि फ़ोकस बाहर जा सके |
| सफ़ाई | `destroy()` listener, observer और कैनवास हटाता है; इसे `unmount` से कॉल करें |

कैनवास को `tabindex="0"`, `role="application"`, लेबल और `aria-describedby` मिलता है।

सुगम्यता पैटर्न, जैसा शोकेस डायग्राम एडिटर (`examples/showcase/islands/diagram`) में:

- **कैनवास पर कीबोर्ड:** `]` और `[` आकृतियाँ चुनते हैं, तीर उन्हें हिलाते हैं, Delete हटाता है, `+`, `-` और `0` ज़ूम करते हैं, Escape चयन साफ़ करता है। कुंजियाँ केवल तब काम करती हैं जब कैनवास पर फ़ोकस हो।
- **सामान्य कंट्रोल:** बटनों का टूलबार कैनवास के बिना वही करता है।
- **टेक्स्ट सारांश:** कैनवास के नीचे की सूची हर आकृति का वर्णन करती है और हर बदलाव का पालन करती है; एक `role="status"` पंक्ति चयन और सेव परिणाम घोषित करती है।
- **फ़ॉलबैक:** सर्वर वही सारांश रेंडर करता है, इसलिए WebAssembly के बिना उपयोगकर्ता फिर भी डायग्राम पढ़ते हैं।

एडिटर `SaveDiagram` से सहेजता है, और सर्वर जो पाता है उसे सीमित करता है (`Diagram::clamped`: अधिकतम 200 आकृतियाँ, अद्वितीय आईडी, 40 अक्षरों के लेबल, सीमित निर्देशांक) क्योंकि island पर कभी भरोसा नहीं किया जाता।

## 8. प्रदर्शन

| क्या | संख्या |
|---|---|
| सादा पृष्ठ HTML, अनुरोध, नीति | अपरिवर्तित: कोई meta नहीं, कोई लोडर नहीं, कोई अतिरिक्त अनुरोध नहीं |
| `nav.js` | +281 बाइट (पूरे लोड का फ़ॉलबैक) |
| `/_zeyn/islands.js` लोडर, केवल island पृष्ठ | 3.9 KB, 1.7 KB gzip |
| काउंटर island `.wasm` (wasm-opt के बिना) | 127.6 KB, 52.8 KB gzip |
| काउंटर island glue `.js` | 18.0 KB, 4.2 KB gzip |
| डायग्राम एडिटर `.wasm` (कैनवास सतह, wasm-opt के बिना) | 169.4 KB, 67.8 KB gzip |
| लाइव संपादन वाला डायग्राम एडिटर (`yrs` जोड़ता है, देखें `docs/realtime.md`) | 408.4 KB, 144.3 KB gzip |
| डायग्राम एडिटर glue `.js` | 30.3 KB, 6.1 KB gzip |
| प्रति island सर्वर मेमोरी | कोई नहीं; फ़ाइलें डिस्क से स्ट्रीम होती हैं, props प्रति अनुरोध रेंडर होते हैं |

काउंटर के आकार का अधिकांश `serde_json` और `core` के फ़ॉर्मैटिंग कोड का है। जो islands इन्हें साझा करते हैं वे हर एक एक बार इनकी कीमत चुकाते हैं; wasm-opt आमतौर पर 10 से 20 प्रतिशत और घटाता है।

## 9. सीमाएँ

- Window (`open_window`) के भीतर के island के लिए खोलने वाले पृष्ठ के पास island नीति होनी चाहिए; island वाली Window को island पृष्ठों पर रखें या उन्हें Screen के रूप में खोलें।
- `Closure::forget` से जुड़े क्लोज़र पृष्ठ जितने समय तक जीते हैं; जब island कई बार माउंट हो सकता हो तो उन्हें `unmount` में छोड़ें।
- Islands खुद सर्वर पर कुछ रेंडर नहीं करते; फ़ॉलबैक वही है जो सर्च इंजन, लोड से पहले स्क्रीन रीडर और बिना JavaScript के उपयोगकर्ता पाते हैं। उसे सार्थक बनाएँ।
