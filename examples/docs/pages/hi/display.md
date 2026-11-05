# डिस्प्ले

वे कंपोनेंट जो स्थिति दिखाते हैं: संदेश, बैज, प्रगति, कुंजी-मान, इतिहास और प्लेसहोल्डर। वे सर्वर पर रेंडर होते हैं और उन्हें अपने कोई अनुरोध नहीं चाहिए।

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

`Alert::info`, `success`, `warning` और `danger`, या `Alert::new(tone, text)`। `.title(text)` शीर्षक जोड़ता है और `.dismissible()` बंद करने का बटन। टोन संदेश से पहले बोला भी जाता है, इसलिए वह कभी केवल रंग से नहीं दिया जाता।

## Badge

`Badge::new(text).tone(Tone::...)`। टोन: `Neutral`, `Info`, `Success`, `Warning`, `Danger` और `Brand`। ग्रिड `.badge(|v| tone)` से मान को बैज के रूप में दिखाते हैं।

## Progress

वैकल्पिक `.tone(...)` के साथ `Progress::new(label, value, max)`। यह अपने लेबल और मान के साथ असली `progressbar` है।

## DescriptionList

कुंजी-मान जोड़े, किसी रिकॉर्ड के फ़ील्ड से या हाथ से:

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

मान ग्रिड सेल की तरह स्वरूपित होते हैं: मुद्रा, तारीख़ें और choice उपयोगकर्ता की भाषा में।

## Timeline

`Timeline::new().event(moment, title, detail, tone)` घटनाएँ सूचीबद्ध करता है। क्षण उपयोगकर्ता के समय क्षेत्र और भाषा में दिखते हैं।

## EmptyState और Skeleton

`EmptyState::new(title).text(text)` खाली जगह को अगले कदम से भरता है: `.link(label, screen, params)` `LinkButton` के माध्यम से दूसरे पेज पर जाता है, और `.opens(label, screen, params)` ऐप की एक स्क्रीन को विंडो के रूप में खोलता है। `Skeleton::lines(n)` कुछ लोड होते समय का प्लेसहोल्डर है।

## LinkButton

कहीं जाना लिंक है; कुछ करना बटन है। `LinkButton::new(label, screen, params)` स्क्रीन के कैनोनिकल पथ (जैसे `/products/42`) का असली `<a href>` है जो बटन जैसा दिखता है, इसलिए मिडिल क्लिक, "नए टैब में खोलें" और सर्च इंजन उसे वैसा ही लिंक मानते हैं जो वह है। यह `nav.js` के माध्यम से बिना पेज रीलोड के खुलता है, और Enter उसे अनुसरण करता है। रूप द्वितीयक रहता है जब तक `.primary()` मुख्य कार्रवाई का भरा हुआ रूप न माँगे। स्क्रीन का रूट होना चाहिए, वरना कोड कंपाइल नहीं होता। `EmptyState::link` और `Hero::link` इसका उपयोग करते हैं।

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)` वह बटन है जो इस ब्राउज़र द्वारा ऐप के लिए रखी हर क्लाइंट प्राथमिकता हटा देता है, जैसे किसी गोपनीयता पेज पर `App::client_prefs_table()` की सूची के बगल में। एक toast पुष्टि करता है। सर्वर द्वारा सेट की गई कुकी (थीम, भाषा, सत्र) को छुआ नहीं जाता। स्क्रिप्ट `zeyn.prefs.clear()` से यही कर सकती हैं।

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: लिंक टाइप्ड Screen संदर्भ हैं, इसलिए नाम बदली Screen टूटा लिंक नहीं छोड़ती।

## ActionButton

`ActionButton::new(label, action, input)` एक action चलाता है। `.confirm(text)` पहले पूछता है, `.tone(Tone::Danger)` विनाशकारी बटन को चिह्नित करता है, और `.on_success(|result, ui| ...)` effect चलाता है। इनपुट सर्वर पर तय होता है और क्लाइंट से कभी नहीं आता; action की अनुमति सर्वर पर किसी भी फ़ॉर्म की तरह जाँची जाती है।

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

`Markdown::new(text)` स्वरूपित टेक्स्ट दिखाता है, जैसे टिप्पणी, उत्पाद विवरण या सहायता। टेक्स्ट एक स्ट्रिंग या locale कुंजी है। इसे `markdown` feature (`zeyn = { features = ["markdown"] }`) चाहिए, जो कोई निर्भरता नहीं जोड़ता।

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- शीर्षक, अनुच्छेद, सूचियाँ, तालिकाएँ, कोड ब्लॉक, उद्धरण, रेखाएँ, इनलाइन कोड, लिंक, छवियाँ, बोल्ड और इटैलिक।
- कच्चा HTML कभी आगे नहीं भेजा जाता: `<script>` टेक्स्ट के रूप में दिखता है, और कोड ब्लॉक भी escape किए जाते हैं।
- लिंक केवल `http`, `https`, `mailto`, किसी एंकर या सापेक्ष पथ की ओर इशारा कर सकते हैं। अन्य, जैसे `javascript:` या `data:`, केवल अपना लेबल रखते हैं। उपयोगकर्ता के लिंक को `rel="noopener noreferrer nofollow ugc"` मिलता है।
- छवियाँ केवल ऐप से ही लोड होती हैं (सापेक्ष पथ); अन्य छवियाँ अपना alt टेक्स्ट दिखाती हैं।
- `.trusted()` ऐप के लिखे टेक्स्ट के लिए है: शीर्षकों को उन तक लिंक के लिए id मिलते हैं और लिंक `nofollow ugc` खो देते हैं। escaping और लिंक नियम वही रहते हैं।
- 64 KB (`MAX_MARKDOWN`) से लंबा टेक्स्ट काट दिया जाता है, और नेस्टिंग सीमित है, इसलिए शत्रुतापूर्ण इनपुट रेंडर करना सस्ता रहता है।

## Embed

`Embed` किसी दूसरी साइट का पृष्ठ, जैसे वीडियो प्लेयर या नक्शा, लॉक किए फ़्रेम में दिखाता है। शीर्षक ज़रूरी है (स्क्रीन रीडर फ़्रेम को उसी से घोषित करते हैं); `.title(...)` के बिना embed रखा नहीं जा सकता और कंपाइलर यह बताता है।

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- होस्ट ऐप के `frame-src` में होना चाहिए: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` (देखें [सुरक्षा](security.md#csrf-csp-and-headers))। जब पता `https` न हो या उसका होस्ट सूची में न हो, तो कोई फ़्रेम रेंडर ही नहीं होता; एक चेतावनी बताती है कि सामग्री दिखाई नहीं जा सकती।
- हर फ़्रेम को `sandbox="allow-scripts allow-same-origin allow-presentation"`, `referrerpolicy="no-referrer"` और `loading="lazy"` मिलता है। पॉपअप, टॉप नेविगेशन, फ़ॉर्म और डाउनलोड की अनुमति कभी नहीं दी जा सकती। `.allow(...)` ब्राउज़र सुविधाएँ जोड़ता है: `Fullscreen`, `PictureInPicture`, `Autoplay` और `EncryptedMedia`।
- `.click_to_load(poster)` पहले एक बटन दिखाता है, पोस्टर छवि (ऐप का पथ या `img-src` का होस्ट; कुछ नहीं के लिए `""`) और दूसरी साइट के नाम के साथ। बटन दबाने तक दूसरी साइट से संपर्क नहीं किया जाता; फिर फ़्रेम उसकी जगह लेता है और फ़ोकस पाता है। स्क्रिप्ट `media.js` का हिस्सा है, इसलिए कोई अतिरिक्त अनुरोध नहीं लगता।
- `.notice(t!(...))` उस बटन पर ऐप का एक टेक्स्ट रखता है, जैसे सामग्री चेतावनी। यह escape होता है, हर टेक्स्ट की तरह अनूदित होता है और बटन के सुगम्य नाम का हिस्सा है, इसलिए उपयोगकर्ता के निर्णय से पहले स्क्रीन रीडर उसे पढ़ते हैं। `.click_to_load` के बिना यह बिना पोस्टर वाला बटन चालू कर देता है।
- `.remember(EXTERNAL_VIDEO)` कुंजी के जीवनकाल में एक बार पूछता है, हर पृष्ठ पर नहीं। कुंजी `ClientPref::consent` से घोषित और `App::client_prefs` में पंजीकृत consent कुंजी है (देखें [क्लाइंट प्राथमिकताएँ](security.md#client-preferences)); एक ही कुंजी से बँधे embed उत्तर साझा करते हैं। बाद के पृष्ठ, ऐप के भीतर नेविगेशन और अन्य टैब वे फ़्रेम खुद लोड करते हैं, बिना फ़ोकस हिलाए (WCAG 2.2 3.2.1 On Focus, 2.4.3 Focus Order); फ़ोकस केवल असली दबाव के बाद फ़्रेम पर जाता है। बटन तब लौट आता है जब रिकॉर्ड समाप्त हो गया हो या उसकी जाँचें विफल हों, जब ऐप ने कुंजी पंजीकृत न की हो, जब स्टोरेज अवरुद्ध हो और पृष्ठ नया हो, और JavaScript के बिना। सादी स्ट्रिंग या choice कुंजी कंपाइल नहीं होती।
- वह फ़ील्ड जो embed करने का पता रखता है, `#[rule(embed)]` लेता है। फ़ॉर्म URL या चिपकाया गया embed कोड स्वीकार करते हैं, केवल `src` रखते हैं, `https` और सूचीबद्ध होस्ट माँगते हैं, ट्रैकिंग पैरामीटर (`utm_*`, `fbclid`, `gclid` और अन्य) हटाते हैं और कैनोनिकल पता सहेजते हैं, HTML कभी नहीं। API इनपुट को कैनोनिकल पता भेजना होगा।

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

`Image` को अपना alt टेक्स्ट और आकार चाहिए; `.alt(...)` और `.size(width, height)` के बिना इसे रखा नहीं जा सकता और कंपाइलर यह बताता है। आकार फ़ाइल लोड होते समय जगह बनाए रखता है, इसलिए पृष्ठ हिलता नहीं।

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` ऐप का पथ (`/static/logo.png`) या ऐप के `img-src` के होस्ट पर `https` पता है (`App::csp(|c| c.img_src([...]))`)। कोई अन्य छवि रेंडर नहीं होती और उसकी जगह उसका alt टेक्स्ट दिखता है, इसलिए ब्राउज़र कभी अवरुद्ध अनुरोध लॉग नहीं करता।
- हर छवि को `decoding="async"` मिलता है; `.lazy()` फ़ोल्ड के नीचे की छवियों के लिए `loading="lazy"` जोड़ता है। `srcset` घनत्व (`2x`) या चौड़ाई (`800w`) डिस्क्रिप्टर लेता है; अन्य होस्ट की या अन्य डिस्क्रिप्टर वाली प्रविष्टियाँ छोड़ दी जाती हैं। चौड़ाई डिस्क्रिप्टर छवि की चौड़ाई से `sizes` जोड़ते हैं।
- पूरी तरह सजावटी छवि खाली alt टेक्स्ट लेती है: `.alt(Text::from(""))`।

## ExternalLink

`ExternalLink::new(url, text)` `rel="noopener noreferrer"` के साथ दूसरी साइट से जोड़ता है, इसलिए दूसरी साइट को न पृष्ठ का पता मिलता है न विंडो पर पकड़। स्क्रीन रीडर टेक्स्ट के बाद "external site" सुनते हैं; एक तीर इसे दृश्य रूप से चिह्नित करता है।

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()` उपयोगकर्ताओं के टाइप किए पतों के लिए `nofollow ugc` जोड़ता है। `.new_tab()` नया टैब खोलता है और स्क्रीन रीडर को यह बताता है; सेट किए बिना लिंक उसी टैब में रहते हैं। केवल `https` और `http` पते लिंक बनते हैं; बाकी कुछ भी, जैसे `javascript:`, सादे टेक्स्ट के रूप में दिखता है।

## Tooltip, popover और busy overlay

tooltip एक कंट्रोल का छोटा विवरण है: `ActionButton::tip(text)`, Screen टूलबार में `ToolButton::tip(text)` और `Form::tip(field, text)`, जो फ़ील्ड के लेबल के बाद एक जानकारी बटन जोड़ता है। टेक्स्ट कंट्रोल का `aria-describedby` है, इसलिए स्क्रीन रीडर उसे कंट्रोल के साथ सुनते हैं। यह थोड़ी देर होवर के बाद और फ़ोकस पर तुरंत दिखता है, पॉइंटर उस पर रहने तक बना रहता है (WCAG 1.4.13), Escape से बिना फ़ोकस हिलाए छिपता है, और टच पर लंबे दबाव (जानकारी बटन पर टैप) के बाद दिखता है। tooltip में कभी वह जानकारी नहीं होती जो उपयोगकर्ता के पास होनी ही चाहिए: उसे लेबल, फ़ील्ड के नियमों या पृष्ठ में रखें।

`Popover::new(label).title(title).add(node)` एक बटन है जो अपने पास एक नॉन-मोडल डायलॉग खोलता है, छोटे विवरणों के लिए। फ़ोकस उसके भीतर जाता है; Escape, बंद करने का बटन या बाहर क्लिक उसे बंद करते हैं और फ़ोकस बटन पर लौटता है। संकरी स्क्रीन पर यह बॉटम शीट के रूप में खुलता है। इसके चाइल्ड सामान्य नोड हैं, इसलिए उसके भीतर का फ़ॉर्म हर सर्वर जाँच के साथ अपने नोड पथ पर पोस्ट करता है।

`Busy::new().add(node)` अपनी सामग्री को ढकता है जब उसके भीतर कोई अनुरोध चल रहा हो: सहेजता फ़ॉर्म, action बटन, लोड होता ग्रिड या ट्री। कंटेनर को तुरंत `aria-busy` मिलता है; स्पिनर और "Working" टेक्स्ट (एक status संदेश) केवल 400 ms बाद, या `.delay_ms(n)` के बाद, दिखते हैं, ताकि तेज़ उत्तरों में झिलमिलाहट न हो, और एक बार दिखने पर कम से कम 300 ms रहते हैं। ओवरले पॉइंटर को रोकता है, कीबोर्ड को नहीं, और फ़ोकस वहीं रहता है जहाँ था।

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| कुंजी | कहाँ | क्या करती है |
|---|---|---|
| Tab | tooltip वाला कंट्रोल | फ़ोकस tooltip दिखाता है; छोड़ने पर वह छिपता है |
| Escape | कहीं भी जब tooltip दिख रहा हो | tooltip छिपाती है; फ़ोकस रहता है |
| Enter, Space | popover बटन | popover खोलती हैं; फ़ोकस उसके पहले कंट्रोल पर जाता है |
| Escape | popover के भीतर | उसे बंद करती है; फ़ोकस बटन पर लौटता है |
| Tab | popover के भीतर | उसके कंट्रोल में चलती है; उसे छोड़ने पर वह बंद होता है |

तीनों को लगभग 2.2 KB gzip स्क्रिप्ट चाहिए और कोई अपना अनुरोध नहीं; tooltip 1 नोड है, popover 6 और उसकी सामग्री, busy ओवरले 3।

## ड्रॉप डाउन और स्प्लिट बटन

`MenuButton::new(label)` कमांड का मेनू खोलता है; `MenuButton::split(label, action, input)` अपने मुख्य हिस्से से एक action चलाता है और अपने तीर से मेनू खोलता है। कमांड `.action(label, action, input)` (या सफलता के बाद effect के साथ `.action_then(...)`), Window खोलने जैसे effect के लिए `.item(label, effect)`, और `.separator()` हैं। `.confirm(text)` आखिरी action कमांड चलने से पहले पूछता है।

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

action कमांड अपने पथ वाले नोड हैं और `ActionButton` की तरह फ़ॉर्म एंडपॉइंट पर पोस्ट करते हैं: Screen का एक्सेस, feature के गार्ड, action की अनुमति, इंटरसेप्टर, CSRF और लेखन दर सीमा हर कॉल पर जाँची जाती है, और इनपुट सर्वर पर रहता है। जो कमांड उपयोगकर्ता नहीं चला सकता वे मेनू से हटा दिए जाते हैं (स्प्लिट बटन की मुख्य action की जगह उसे निष्क्रिय किया जाता है), और कॉल किए जाने पर सर्वर फिर भी उन्हें अस्वीकार करता है।

@live menus

| कुंजी | कहाँ | क्या करती है |
|---|---|---|
| Enter, Space, Arrow Down | बटन | मेनू को पहले कमांड पर खोलती हैं |
| Arrow Up | बटन | मेनू को आखिरी कमांड पर खोलती है |
| Arrow Down, Arrow Up | मेनू | अगला और पिछला कमांड, घूमकर |
| Home, End | मेनू | पहला और आखिरी कमांड |
| अक्षर | मेनू | उनसे शुरू होने वाला अगला कमांड |
| Enter, Space | कोई कमांड | उसे चलाती हैं और मेनू बंद करती हैं |
| Escape | मेनू | उसे बंद करती है; फ़ोकस बटन पर लौटता है |
| Tab | मेनू | उसे बंद करती है और आगे बढ़ती है |

मेनू को लगभग 0.8 KB gzip स्क्रिप्ट चाहिए और कमांड चलने तक कोई अनुरोध नहीं; यह 4 नोड और हर कमांड के लिए 2 है।
