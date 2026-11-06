# फ़ॉर्म

`Form::of(value)` किसी मॉडल या entity को संपादित करता है। फ़ील्ड फ़ील्ड संदर्भों से आते हैं, लेबल locale फ़ाइलों से और सत्यापन `#[rule]` एट्रिब्यूट से, इसलिए ब्राउज़र और सर्वर वही नियम जाँचते हैं।

## एक फ़ॉर्म

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

नाम खाली छोड़ें या गलत ईमेल टाइप करें और *भेजें* दबाएँ: ब्राउज़र त्रुटि तुरंत दिखाता है, और सर्वर वही इनपुट उसी संदेश के साथ अस्वीकार करेगा।

## लेआउट

| कॉल | परिणाम |
|---|---|
| `.field(M::f)` | अपनी पंक्ति में एक फ़ील्ड |
| `.row(\|r\| r.field(a).field(b))` | फ़ील्ड साथ-साथ; संकरी स्क्रीन पर वे ऊपर-नीचे लग जाते हैं |
| `.section(title)` | उसके बाद के फ़ील्ड का शीर्षक वाला समूह |
| `.step(title)` | विज़ार्ड चरण; फ़ॉर्म एक बार में एक चरण दिखाता है |
| `.range(M::from, M::to)` | दो तारीख़ें एक रेंज पिकर के रूप में |
| `Form::auto(value)` | हर फ़ील्ड, घोषणा के क्रम में |

## सहेजना

`.submit(label, action)` उस action को नाम देता है जो सहेजती है। action को फ़ॉर्म के मॉडल के लिए `Valid<M>` लेना होगा; इसके अलावा कुछ भी कंपाइल नहीं होता। रनटाइम मान भेजता है, सर्वर हर नियम चलाता है, फिर action।

- नियमों से या action में `ValidationErrors` से आई फ़ील्ड त्रुटियाँ फ़ील्ड के पास दिखती हैं।
- `Error::Conflict` (पुराना `#[version]`) एक संदेश दिखाता है कि किसी और ने रिकॉर्ड बदल दिया है।
- `.on_success(|result, ui| ...)` सफल सेव के बाद effect चलाता है (देखें [Effect और Window](effects.md))।

## कंट्रोल

हर फ़ील्ड प्रकार का एक डिफ़ॉल्ट कंट्रोल है: टेक्स्ट इनपुट, नंबर इनपुट, करेंसी के साथ मुद्रा, तारीख़ और समय पिकर, `bool` के लिए चेकबॉक्स और choice के लिए select। `.control(M::f, Control::...)` दूसरा चुनता है, और `.lookup(M::f, action)` सर्वर पर रिकॉर्ड खोजता है। देखें [कंट्रोल और पिकर](controls.md)।

## निर्भर lookup

`.depends_on(field, source)` lookup `field` के विकल्पों को किसी दूसरे फ़ील्ड पर निर्भर बनाता है, जैसे किसी देश के शहर। हर खोज `source` का मान `LookupQuery::parent` में भेजती है (`q.parent_id()` उसे id के रूप में पढ़ता है), और `source` बदलने पर `field` ख़ाली हो जाता है, ताकि नए मान के लिए उसके विकल्प फिर से लोड हों।

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

इस जोड़ी पर ब्राउज़र का भरोसा नहीं किया जा सकता, इसलिए सबमिट पर सर्वर चुने गए शहर के बारे में भेजे गए देश के साथ lookup action से, उस action की अनुमति के साथ, फिर से पूछता है और जो शहर वह नहीं लौटाता उसे फ़ील्ड त्रुटि के साथ अस्वीकार करता है। id वाले अनुरोधों में भी parent से फ़िल्टर करें, जैसा फ़िल्टर करने के बाद `q.apply` करता है। `App::check_page` ऐसा `depends_on` बताता है जिसका फ़ील्ड फ़ॉर्म का lookup न हो या जिसका स्रोत फ़ॉर्म का कोई दूसरा फ़ील्ड न हो।

## फ़िल्टर फ़ॉर्म

बिना सबमिट बटन वाला फ़ॉर्म Screen को फ़िल्टर कर सकता है: `.drives(topic)` उसके फ़ील्ड को Screen params बना देता है। जब कोई फ़ील्ड बदलता है (सेलेक्ट, तारीख की सीमा, Enter दबाने पर टेक्स्ट फ़ील्ड), उसका मान Screen के उसी नाम वाले param में जाता है, पता साथ में अपडेट रहता है, और `.listens(topic)` वाले node उसी जगह रिफ़्रेश होकर मान अपने params से पढ़ते हैं ([Topic](effects.md#topics) देखें)। संख्याएँ और boolean वैसे ही भेजे जाते हैं और ख़ाली फ़ील्ड `null` के रूप में; सर्वर params को Screen के typed params के रूप में पढ़ता है, इसलिए जो मान फ़िट न हो वह अस्वीकार होता है।

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

`submit` वाला फ़ॉर्म drive नहीं कर सकता; उसके बजाय सहेजने के बाद `on_success` में `ui.refresh(topic)` से रिफ़्रेश करें।

## केवल पढ़ने योग्य फ़ील्ड

`#[readonly]` फ़ील्ड फ़ॉर्म में दिखते हैं लेकिन संपादित नहीं हो सकते, जैसे स्टॉक स्तर जिसे केवल मूवमेंट बदलते हैं।

## फ़ाइल अपलोड

`File`, `Option<File>` और `Vec<File>` फ़ील्ड `FileStore` द्वारा रखी फ़ाइलें रखते हैं। नियम एक बार `const` के रूप में घोषित करें, स्टोर को अनुमति के साथ feature पर पंजीकृत करें, और फ़ॉर्म पर वही नियम इस्तेमाल करें:

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

एक अनुमति नाम उन्हीं उपयोगकर्ताओं को अपलोड और डाउनलोड दोनों देता है। पाठकों को वे फ़ाइलें खोलने देने के लिए जिन्हें वे जोड़ नहीं सकते, इसे बाँटें: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`।

कंट्रोल एक ड्रॉप ज़ोन के भीतर नेटिव फ़ाइल इनपुट है। हर फ़ाइल प्रगति पट्टी के साथ अपने आप अपलोड होती है, चलते समय रद्द की जा सकती है, विफल होने पर दोबारा कोशिश की जा सकती है और पूरी होने पर हटाई जा सकती है। छवियाँ चुनी गई फ़ाइल के object URL से बना पूर्वावलोकन दिखाती हैं। प्रगति और परिणाम polite लाइव रीजन में घोषित होते हैं। शोकेस में `/uploads` पर एक काम करता उदाहरण है; इस दस्तावेज़ साइट में कोई साइन इन उपयोगकर्ता नहीं है, इसलिए यह कोई लाइव अपलोड नहीं दिखाती।

| कुंजी | क्या करती है |
|---|---|
| Tab | फ़ाइल इनपुट पर, फिर हर फ़ाइल के डाउनलोड लिंक और उसके Cancel, Retry या Remove बटन पर जाती है |
| फ़ाइल इनपुट पर Enter या Space | सिस्टम फ़ाइल पिकर खोलती हैं (ड्रैग और ड्रॉप का कीबोर्ड रास्ता) |
| Cancel, Retry, Remove पर Enter या Space | उसे चलाती हैं; Cancel के बाद फ़ोकस Retry पर जाता है, Remove के बाद अगली फ़ाइल पर या इनपुट पर लौटता है |

`POST /_zeyn/files/{store}` के लिए सर्वर क्या करता है:

1. CSRF हेडर, स्टोर की अनुमति और feature के गार्ड जाँचता है, और इंटरसेप्टर चलाता है (`CallKind::Upload`, नई फ़ाइल आईडी के साथ ऑडिट किया गया); अपलोड लेखन सीमा में गिने जाते हैं;
2. सीमा से अधिक घोषित `Content-Length` अस्वीकार करता है (`413`), फिर पहले 4 KiB पढ़ता है और सामग्री से प्रकार पहचानता है; एक्सटेंशन उसी प्रकार का होना चाहिए और प्रकार `accept` में होना चाहिए, वरना `415`। HTML, SVG, XML और स्क्रिप्ट कभी स्वीकार नहीं होते। ब्राउज़र का content type अनदेखा किया जाता है;
3. बाकी को स्टोर में अधिकतम 64 KiB के टुकड़ों में चार की चैनल से स्ट्रीम करता है, बाइट गिनते हुए; सीमा पार होना, टूटा कनेक्शन या 30 सेकंड बिना डेटा के अपलोड रोक देते हैं और स्टोर जो लिख चुका था उसे हटा देता है;
4. feature का `FileScanner` चलाता है; अस्वीकार की गई फ़ाइल, या विफल होने वाला स्कैनर, फ़ाइल को हटा देता है और उत्तर `422` होता है;
5. `{id, name, size, type}` के साथ `201` उत्तर देता है। आईडी सर्वर के 128 यादृच्छिक बिट हैं; नामों से पथ और कंट्रोल वर्ण साफ़ कर दिए जाते हैं।

डाउनलोड (`GET /_zeyn/files/{store}/{id}`) आईडी का आकार, अनुमति, गार्ड और टेनेंट जाँचते हैं, और मीडिया सर्विंग से गुज़रते हैं (देखें `docs/media.md`): रेंज, entity टैग और `nosniff`। केवल रास्टर छवियाँ, ऑडियो और वीडियो इनलाइन हैं; बाकी सब `application/octet-stream` के रूप में `Content-Disposition: attachment` के साथ, उद्धृत ASCII नाम और UTF-8 नाम सहित, `Content-Security-Policy: sandbox` के अंतर्गत भेजा जाता है।

`DiskStore` एक फ़ोल्डर में `{id}.bin` और `{id}.json` रखता है और अपलोड पहले `{id}.part` में लिखता है। `MemoryStore` फ़ाइलें बाइट सीमा के साथ मेमोरी में रखता है, टेस्ट और डेमो के लिए। फ़ॉर्म मान केवल फ़ाइलों को नाम देता है: जिस सेव action को पक्का करना हो कि फ़ाइल मौजूद है, वह `store.meta(id)` कॉल करती है, जैसा शोकेस करता है।

बजट: 256 MiB का अपलोड सर्वर हीप को लगभग 0.4 MiB बढ़ाता है (`tests/files_memory.rs`); कंट्रोल 10 DOM नोड और हर फ़ाइल के लिए 5 रेंडर करता है; `files.js` 12 KB कच्चा और 4 KB gzip, `files.css` 3 KB कच्चा और 1 KB gzip जोड़ते हैं।
