# CLI और प्रोजेक्ट

`zeyn` कमांड प्रोजेक्ट बनाता है, feature जोड़ता है, ऐप की जाँच करता है और एजेंट के लिए MCP सर्वर चलाता है।

## कमांड

| कमांड | क्या करता है |
|---|---|
| `zeyn new <name>` | मानक feature संरचना वाला ऐप बनाता है |
| `zeyn new <name> --lib` | कई ऐप में साझा मॉडल, feature और टेक्स्ट की लाइब्रेरी बनाता है |
| `zeyn g feature <name>` | `models`, `services`, `actions` और `screens` फ़ोल्डरों और एक चालू सूची Screen के साथ `src/features/<name>` जोड़ता है |
| `zeyn g screen <feature> <name>` | किसी feature में `screens/<name>.rs` जोड़ता है, उसे रजिस्टर करता है और उसका शीर्षक हर लोकेल फ़ाइल में जोड़ता है |
| `zeyn routes` | feature, Screen, रूट, API और एक्सेस सूचीबद्ध करता है |
| `zeyn check` | कंपाइल करता है और संरचनात्मक समस्याएँ तथा छूटे हुए अनुवाद बताता है |
| `zeyn check --fix` | `src`, `tests` और `examples` में नाम बदले गए Zeyn API को फिर से लिखता है, फिर जाँचता है |
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
- `Dockerfile` और `.dockerignore`: छोटी कंटेनर इमेज।
- आम CI सर्वरों के लिए पाइपलाइन, सभी वही जाँचें चलाती हैं (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`) और ऐसे टैग `vX.Y.Z` को अस्वीकार करती हैं जो `Cargo.toml` से मेल न खाए:
  - GitHub Actions के लिए `.github/workflows/ci.yml`, साथ में `.github/dependabot.yml` (साप्ताहिक Cargo और action अपडेट) और पुल रिक्वेस्ट चेकलिस्ट;
  - GitLab के लिए `.gitlab-ci.yml`;
  - Jenkins के लिए `Jenkinsfile` (Docker एजेंट, `cargo llvm-cov` से कवरेज)।
- `sonar-project.properties`: SonarQube विश्लेषण (Clippy समस्याएँ और `target/lcov.info` से LCOV कवरेज)। GitHub वर्कफ़्लो इसे तब चलाता है जब रिपॉज़िटरी वेरिएबल `SONAR_HOST_URL` और सीक्रेट `SONAR_TOKEN` सेट हों; Jenkins इसे तब चलाता है जब जॉब `SONAR_ENABLED=true` सेट करे।
- `SECURITY.md` और `CHANGELOG.md` (Keep a Changelog)।
- ब्रांच `main` पर एक स्थानीय git रिपॉज़िटरी, जब git इंस्टॉल हो (Windows, macOS या Linux) और फ़ोल्डर पहले से किसी रिपॉज़िटरी के भीतर न हो। कुछ भी कमिट नहीं होता, इसलिए पहला कमिट आपका अपना नाम और ईमेल लेता है। `--no-git` इसे छोड़ देता है।

हर फ़ोल्डर के लिए [प्रोजेक्ट संरचना](structure.md) देखें।

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

## Docker

जनरेट की गई `Dockerfile` दो चरणों में बिल्ड करती है और बिना root अधिकार वाले उपयोगकर्ता के रूप में distroless इमेज पर चलती है। बाइनरी अपनी सेहत `<app> health` से जाँचती है, जिसे `HEALTHCHECK` इस्तेमाल करता है।

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

जनरेट किए गए ऐप की इमेज लगभग 43 MB की है और निष्क्रिय ऐप कुछ MB RAM लेता है।

## MCP सर्वर

`zeyn mcp` stdio पर MCP बोलता है। इसके टूल हैं `zeyn_docs` (विषय के अनुसार फ्रेमवर्क संदर्भ), `zeyn_describe` (मौजूदा डायरेक्टरी के ऐप के feature, Screen, रूट और समस्याएँ), `zeyn_check` (`fix: true` के साथ यह `check --fix` चलाता है), `zeyn_test`, `zeyn_new_feature` और `zeyn_new_screen`। देखें [MCP और AI एजेंट](mcp.md)।
