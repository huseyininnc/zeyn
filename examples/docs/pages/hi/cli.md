# CLI और प्रोजेक्ट

`zeyn` कमांड प्रोजेक्ट बनाता है, feature जोड़ता है, ऐप की जाँच करता है और एजेंट के लिए MCP सर्वर चलाता है।

## कमांड

| कमांड | क्या करता है |
|---|---|
| `zeyn new <name>` | मानक feature संरचना वाला ऐप बनाता है |
| `zeyn new <name> --lib` | कई ऐप में साझा मॉडल, feature और टेक्स्ट की लाइब्रेरी बनाता है |
| `zeyn new <name> --component` | ब्राउज़र स्क्रिप्ट, स्टाइलशीट, टेक्स्ट, उदाहरण ऐप और टेस्ट के साथ कॉम्पोनेंट क्रेट `zeyn-<name>` बनाता है |
| `zeyn g feature <name>` | `models`, `services`, `actions` और `screens` फ़ोल्डरों और एक चालू सूची Screen के साथ `src/features/<name>` जोड़ता है |
| `zeyn g screen <feature> <name>` | किसी feature में `screens/<name>.rs` जोड़ता है, उसे रजिस्टर करता है और उसका शीर्षक हर लोकेल फ़ाइल में जोड़ता है |
| `zeyn routes` | feature, Screen, रूट, API और एक्सेस सूचीबद्ध करता है |
| `zeyn check` | कंपाइल करता है और संरचनात्मक समस्याएँ तथा छूटे हुए अनुवाद बताता है |
| `zeyn check --fix` | `src`, `tests` और `examples` में नाम बदले गए Zeyn API को फिर से लिखता है, फिर जाँचता है |
| `zeyn check --json` | रिपोर्ट को स्थिर कोड, कारण और समाधान के साथ JSON में छापता है |
| `zeyn doctor` | प्रोजेक्ट के बिल्ड होने का तरीका पढ़ता है और बताता है कि क्या उसे धीमा या बड़ा बनाता है, हर निष्कर्ष के लिए एक पंक्ति का समाधान देता है |
| `zeyn mcp` | AI एजेंट के लिए stdio पर MCP सर्वर चलाता है |

इसे crates.io से इंस्टॉल करें:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

इस रिपॉज़िटरी के भीतर CLI को Cargo से चलाएँ:

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## नया ऐप

`zeyn new` ऐसा प्रोजेक्ट लिखता है जो बिल्ड होता है, अपने टेस्ट पास करता है और एजेंट को दिए गए नियमों का पालन करता है:

- `AGENTS.md` और `CLAUDE.md`: प्रोजेक्ट के नियम और एजेंट के लिए फ्रेमवर्क सूची।
- `.mcp.json`: `zeyn` MCP सर्वर पंजीकृत करता है, ताकि एजेंट `zeyn_check` और `zeyn_docs` कॉल कर सके।
- `config/app.json`: डिफ़ॉल्ट। `APP_*` एनवायरनमेंट वेरिएबल इन्हें ओवरराइड करते हैं, जैसे `server.addr` के लिए `APP_SERVER__ADDR`।
- `locales/en.json` और `locales/tr.json`: उपयोगकर्ता को दिखने वाला हर टेक्स्ट।
- `src/features/`: हर feature के लिए एक फ़ोल्डर, नमूना `notes` feature के साथ।
- `rustfmt.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` और `Cargo.toml` में `[lints]`: मानक जिन्हें CI लागू करता है।
  `rustfmt.toml` में `chain_width = 40` है, इसलिए कोड ऊपर से नीचे पढ़ा जाता है: इससे लंबी कॉल की शृंखला में हर पंक्ति पर एक कॉल आती है, औसतन प्रति पंक्ति एक या दो बिंदु। एजेंट और लोग एक लंबी पंक्ति की तुलना में सूची जल्दी समीक्षा करते हैं, और diff उसी एक कॉल को दिखाते हैं जो बदली। इस सेटिंग को रखें।

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)` नहीं।
- `build.rs` और `Cargo.toml` में `[package.metadata.zeyn]` के नीचे `max_file_lines = 300`: `src/` की किसी फ़ाइल में इससे ज़्यादा भरी हुई पंक्तियाँ हों तो `cargo build` रुक जाता है; clippy `too_many_lines` (60, `clippy.toml` में) फ़ंक्शनों के लिए यही करता है। [Project structure](structure.md) देखें।
- `Cargo.toml` में `[profile.dev]`, `.cargo/config.toml` और `.vscode/settings.json`: तेज़ बिल्ड चक्र। निर्भरताएँ `opt-level = 1` के साथ और डीबग जानकारी के बिना बनती हैं, आपका अपना कोड केवल लाइन तालिकाएँ रखता है (बैकट्रेस में फ़ाइल और लाइन दिखती है; स्थानीय चरों के साथ डीबग करने के लिए `CARGO_PROFILE_DEV_DEBUG=full` सेट करें), Windows `rust-lld` से लिंक करता है, और rust-analyzer अपना अलग target फ़ोल्डर इस्तेमाल करता है ताकि वह `cargo build` का इंतज़ार न करे।
- `Dockerfile` और `.dockerignore`: छोटी कंटेनर इमेज।
- आम CI सर्वरों के लिए पाइपलाइन, सभी वही जाँचें चलाती हैं (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`) और ऐसे टैग `vX.Y.Z` को अस्वीकार करती हैं जो `Cargo.toml` से मेल न खाए:
  - GitHub Actions के लिए `.github/workflows/ci.yml`, साथ में `.github/dependabot.yml` (साप्ताहिक Cargo और action अपडेट) और पुल रिक्वेस्ट चेकलिस्ट;
  - GitLab के लिए `.gitlab-ci.yml`;
  - Jenkins के लिए `Jenkinsfile` (Docker एजेंट, `cargo llvm-cov` से कवरेज)।
- `sonar-project.properties`: SonarQube विश्लेषण (Clippy समस्याएँ और `target/lcov.info` से LCOV कवरेज)। GitHub वर्कफ़्लो इसे तब चलाता है जब रिपॉज़िटरी वेरिएबल `SONAR_HOST_URL` और सीक्रेट `SONAR_TOKEN` सेट हों; Jenkins इसे तब चलाता है जब जॉब `SONAR_ENABLED=true` सेट करे।
- `SECURITY.md` और `CHANGELOG.md` (Keep a Changelog)।
- ब्रांच `main` पर एक स्थानीय git रिपॉज़िटरी, जब git इंस्टॉल हो (Windows, macOS या Linux) और फ़ोल्डर पहले से किसी रिपॉज़िटरी के भीतर न हो। कुछ भी कमिट नहीं होता, इसलिए पहला कमिट आपका अपना नाम और ईमेल लेता है। `--no-git` इसे छोड़ देता है।

हर फ़ोल्डर के लिए [प्रोजेक्ट संरचना](structure.md) देखें।

## नया कॉम्पोनेंट क्रेट

`zeyn new <name> --component` क्रेट `zeyn-<name>` लिखता है (नाम के आरंभ में `zeyn-` स्वीकार है; नाम में छोटे अक्षर, अंक और इकहरे हाइफ़न हो सकते हैं, अधिकतम 32 वर्ण)। यह [साझा लाइब्रेरी](polyrepo.md) का एक्सटेंशन मॉडल है, बदलने के लिए तैयार:

- `src/lib.rs`: एक नमूना कॉम्पोनेंट (टॉगल बटन), उसका `Plugin` जो `App::module` से ब्राउज़र मॉड्यूल दर्ज करता है, और `CATALOGS` स्थिरांक।
- `assets/<name>.js` और `assets/<name>.css`: मॉड्यूल। क्लास `z-<name>-` से और मार्कर एट्रिब्यूट `data-zeyn-<name>-` से शुरू होते हैं; स्टाइलशीट केवल थीम टोकन इस्तेमाल करती है, इसलिए दोनों थीम काम करती हैं।
- `assets/en.json` और `assets/tr.json`: टेक्स्ट, कुंजी `<name>` के अंतर्गत।
- `examples/demo`: एक ऐप जिसमें बिना कॉम्पोनेंट का शुरुआती पेज और कॉम्पोनेंट वाला एक पेज है। इसे `cargo run --example demo` से चलाएँ।
- `tests/<name>.rs`: कॉम्पोनेंट वाला पेज मॉड्यूल जोड़ता है, बिना कॉम्पोनेंट वाला पेज उसे कभी संदर्भित नहीं करता, रनटाइम बाद में आने वाली सामग्री के लिए मॉड्यूल सूचीबद्ध करता है, फ़ाइलें बिल्ड के साथ URL में परोसी जाती हैं, और ऐप टेक्स्ट के शब्द बदल सकता है।
- लाइब्रेरी जैसी ही प्रोजेक्ट फ़ाइलें: GitHub, GitLab और Jenkins के लिए CI, लिंट और फ़ॉर्मेट नियम, `AGENTS.md` और `CLAUDE.md`।

```sh
zeyn new rating --component
cd zeyn-rating
cargo test
```

## जनरेटर

`zeyn g feature` एक feature बनाता है जिसमें हर मॉडल, सर्विस, action और Screen के लिए एक फ़ाइल होती है:

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

छोटे रूप `zeyn g f` और `zeyn g s` भी यही करते हैं। नया feature तुरंत चलता है: एक मॉडल, मेमोरी में डेटा रखने वाली एक सर्विस, एक खोज action और मेनू प्रविष्टि के साथ एक सूची Screen। जिस feature को किसी फ़ोल्डर की ज़रूरत नहीं, वह उसे हटा देता है (फ़ोल्डर और feature की `mod.rs` में उसकी `mod` पंक्ति)। `components/` कभी नहीं बनाया जाता: जब Screen का कोई हिस्सा कई जगह इस्तेमाल हो या कोई Screen फ़ाइल लंबी हो जाए, तब इसे हाथ से जोड़ें, हर फ़ाइल में `Node` लौटाने वाला एक फ़ंक्शन (`components/<name>.rs`)। Screen को अनुमति `<feature>.<name>.view` मिलती है और उसका शीर्षक `locales/` की हर फ़ाइल में जाता है; अंग्रेज़ी के अलावा भाषाओं को अंग्रेज़ी पाठ और उसे अनुवाद करने का नोट मिलता है। यह ढाँचा अनिवार्य नहीं है: अगर किसी feature में एक ही `screens.rs` है, तो `zeyn g screen` पहले उसे उसकी सामग्री के साथ `screens/mod.rs` में ले जाता है, और अगर `screens/` हटा दिया गया था तो उसे फिर से बनाता है। जनरेटर कभी किसी फ़ाइल को ओवरराइट नहीं करते और snake case से अलग नाम अस्वीकार करते हैं।

## ऐप की जाँच

`zeyn check` कंपाइलर चलाता है और फिर ऐप की अपनी जाँचें (`App::check`): दोहराए गए रूट, दो बार पंजीकृत Screen, ऐसी सेवाएँ जो Screen को चाहिए लेकिन किसी ने पंजीकृत नहीं कीं, अनुपयोगी शॉर्टकट, WCAG AA कंट्रास्ट से नीचे के थीम टोकन और किसी भाषा में छूटी अनुवाद कुंजियाँ। कुछ भी गलत हो तो यह त्रुटि के साथ बाहर निकलता है, इसलिए CI इसे चला सकता है।

यह चेतावनियाँ भी दिखाता है, जो जाँच को कभी विफल नहीं करतीं: ऐसा `#[screen]` या `#[action]` जिसे कोई feature रजिस्टर नहीं करता, और कई आइटम वाली एक ही `screens.rs` या `actions.rs` फ़ाइल जो लगभग 150 पंक्तियों से लंबी हो गई है।

जब Zeyn रिलीज़ किसी API का नाम बदलती है, तो `zeyn check --fix` पुराने नाम फिर से लिखता है, जैसे `Grid` को `DataGrid` और `GridQuery` को `DataQuery`, फिर दोबारा जाँचता है।

```sh
zeyn check --fix
```

हर समस्या का एक स्थिर कोड होता है: ऐप, फ़ीचर, रूट और सेवाओं के लिए `Z01xx`, सुरक्षा के लिए `Z02xx`, सार्वजनिक पेजों के लिए `Z03xx`, मेनू, कमांड और शॉर्टकट के लिए `Z04xx`, डेटा के लिए `Z05xx`, पेज जाँच के लिए `Z06xx` और चेतावनियों के लिए `W01xx`. टेक्स्ट आउटपुट हर समस्या को उसके कारण (`why`), समाधान (`help`), इस दस्तावेज़ के लिंक और कोड के साथ दिखाता है. `zeyn check --json` वही रिपोर्ट एजेंटों और CI के लिए एक JSON दस्तावेज़ के रूप में छापता है, और `ok` के false होने पर त्रुटि के साथ बाहर निकलता है.

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

`zeyn check` ऐप के स्रोत को भी पढ़ता है और फ़ाइल व पंक्ति के साथ इन पर चेतावनी देता है: `W0106` ऐसी स्ट्रिंग जिसमें `src` के बिना `<script>` या `onclick=` जैसा इनलाइन इवेंट हैंडलर हो (CSP दोनों को रोकता है), `W0107` हाथ से लिखा कंपोनेंट जो `escape(..)`, `r.text(..)` या `r.value(..)` के बिना HTML लिखता है, `W0108` ऐसा action जो उसे सीमित करने वाली क्वेरी के बिना पूरा `Vec` लौटाता है, और `W0109` `.trusted()` चिह्नित `DocumentViewer`। गायब `permission` या `public` के लिए चेतावनी की ज़रूरत नहीं: वह कंपाइल ही नहीं होता।

## नए Zeyn संस्करण पर जाना

`zeyn migrate` किसी ऐप को Zeyn के नए संस्करण पर ले जाता है, `ng update` की तरह। यह ऐप का Zeyn संस्करण `Cargo.lock` (या `Cargo.toml`) से पढ़ता है, उस संस्करण और लक्ष्य के बीच का हर माइग्रेशन चरण क्रम से चलाता है, `Cargo.toml` में Zeyn निर्भरताओं को लक्ष्य संस्करण पर ले जाता है और बताता है कि उसने क्या किया: हर बदलाव उसकी फ़ाइल और पंक्ति के साथ, और वे जगहें जिन्हें किसी व्यक्ति को देखना है, हर एक कारण के साथ।

```sh
zeyn migrate --dry-run
zeyn migrate --from 0.0.9 --to 0.0.11 --json
```

एक चरण नाम और मेथड बदलता है (`Hub` से `Socket`, `.realtime(live)` से `.socket(live)`), किसी कॉल का आकार बदलता है (`.sort_by(Order::date, true)` से `.sort_by_desc(Order::date)`), `use` पथ स्थानांतरित करता है और वह दिखाता है जिसे वह सुरक्षित रूप से दोबारा नहीं लिख सकता, जैसे ऐसा `bool` फ़्लैग जो लिटरल नहीं है या हटाए गए पते पर साइन आउट लिंक। यह केवल उन्हीं टोकनों को बदलता है जिन्हें वह बदलता है, इसलिए फ़ॉर्मैटिंग और टिप्पणियाँ बनी रहती हैं, और दूसरी बार चलाने पर कुछ नहीं बदलता। `--dry-run` कुछ नहीं लिखता और एक unified diff छापता है; `--json` रिपोर्ट को `changes`, `todos` और `errors` वाले एक JSON दस्तावेज़ के रूप में छापता है। एग्ज़िट कोड 0 रहता है, जब तक कोई फ़ाइल पढ़ी, पार्स या लिखी न जा सके। `zeyn check --fix` हर चरण के नाम बदलाव लागू करता है। MCP टूल `zeyn_migrate` यही काम एजेंट के लिए करता है।

## बिल्ड की गति और डिस्क का उपयोग

`zeyn doctor` प्रोजेक्ट के बिल्ड होने का तरीका पढ़ता है और बताता है कि क्या उसे धीमा या बड़ा बनाता है: target फ़ोल्डर का आकार और उसके सबसे बड़े फ़ोल्डर, लिंकर, dev प्रोफ़ाइल (डीबग जानकारी और निर्भरताओं का ऑप्टिमाइज़ेशन), `rust-version` के मुकाबले rustc और cargo, पुराने टूलचेन के छोड़े आर्टिफ़ैक्ट, rust-analyzer का target फ़ोल्डर और Windows पर यह कि target फ़ोल्डर और `~/.cargo` Defender स्कैन से बाहर हैं या नहीं (अपवाद या Dev Drive; Windows पढ़ने न दे तो यह वही कहता है)। हर निष्कर्ष का एक स्थिर कोड और एक पंक्ति का समाधान होता है। `--json` उन्हें एक दस्तावेज़ के रूप में छापता है, `--target-limit-gb` तय करता है कि कितना आकार बड़ा माना जाए (डिफ़ॉल्ट 20), और एग्ज़िट कोड हमेशा 0 होता है। `zeyn doctor --fix` तेज़ dev प्रोफ़ाइल `Cargo.toml` में और `rust-lld` लिंकर `.cargo/config.toml` में जोड़ता है और कुछ और नहीं बदलता; दूसरी बार चलाने पर कुछ नहीं बदलता। MCP टूल `zeyn_doctor` एजेंट के लिए यही करता है।

## DevTools

`cargo run` (या `zeyn dev`) से चलने वाला डिबग बिल्ड दर्ज करता है कि हर अनुरोध ने क्या किया: कौन सी Screen और Action चली, उसका स्टेटस और समय, Screen को बनने और रेंडर होने में कितना समय लगा, हर SQL स्टेटमेंट उसके समय और प्रभावित पंक्तियों के साथ, और Action ने कौन से इफ़ेक्ट और टॉपिक लौटाए। Alt+Shift+D पिछले 50 अनुरोधों वाला पैनल खोलता है; हर अनुरोध खुलकर अपने स्टेटमेंट दिखाता है। `GET /_zeyn/devtools` यही सूची JSON में लौटाता है, और MCP टूल `zeyn_devtools` इसे एजेंट के लिए पढ़ता है।

स्टेटमेंट उनके आर्ग्युमेंट के बिना रखे जाते हैं, इसलिए कोई मान पैनल तक नहीं पहुँचता। रिलीज़ बिल्ड में DevTools नहीं होते: न कोड, न रूट, न उनके लिए मेमोरी। जो ऐप `App::router` खुद सर्व करता है, या कोई टेस्ट, उन्हें `zeyn::server::devtools(true)` से चालू करता है।

## Docker

जनरेट की गई `Dockerfile` दो चरणों में बिल्ड करती है और बिना root अधिकार वाले उपयोगकर्ता के रूप में distroless इमेज पर चलती है। बाइनरी अपनी सेहत `<app> health` से जाँचती है, जिसे `HEALTHCHECK` इस्तेमाल करता है।

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

जनरेट किए गए ऐप की इमेज लगभग 43 MB की है और निष्क्रिय ऐप कुछ MB RAM लेता है।

## MCP सर्वर

`zeyn mcp` stdio पर MCP बोलता है। इसके टूल हैं `zeyn_docs` (विषय के अनुसार फ्रेमवर्क संदर्भ), `zeyn_describe` (मौजूदा डायरेक्टरी के ऐप के feature, Screen, रूट और समस्याएँ), `zeyn_inspect` (किसी Screen का node ट्री, हर node को रखने वाली Rust पंक्ति के साथ), `zeyn_devtools` (चल रहे debug build का DevTools request log: Screen, action, समय, SQL statements), `zeyn_check` (`fix: true` के साथ यह `check --fix` चलाता है), `zeyn_doctor`, `zeyn_test`, `zeyn_new_feature` और `zeyn_new_screen`। देखें [MCP और AI एजेंट](mcp.md)।
