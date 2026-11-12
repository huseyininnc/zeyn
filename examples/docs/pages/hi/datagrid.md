# DataGrid

`DataGrid` उस action से पंक्तियाँ दिखाता है जो `DataQuery` लेती है और `Page<E>` लौटाती है। पृष्ठांकन, क्रम, फ़िल्टर और योग सर्वर पर चलते हैं, इसलिए दस लाख पंक्तियों वाला ग्रिड ब्राउज़र को उतना ही महँगा पड़ता है जितना सौ पंक्तियों वाला।

## एक ग्रिड

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

क्रम के लिए शीर्षक पर क्लिक करें, फ़िल्टर बॉक्स में टाइप करें, कॉलम हिलाने के लिए शीर्षक खींचें या आकार बदलने के लिए उसका किनारा खींचें। योग हर फ़िल्टर की गई पंक्ति को कवर करते हैं, केवल पृष्ठ को नहीं।

## action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply` मेमोरी में `Vec` को फ़िल्टर, क्रमबद्ध, योग और पृष्ठबद्ध करता है, जो छोटी सूचियों और टेस्ट के लिए ठीक है। डेटाबेस-समर्थित स्टोर वही `DataQuery` (पृष्ठ, क्रम, फ़िल्टर, योग) पढ़ता है और SQL से उत्तर देता है; देखें [डेटा और डेटाबेस](database.md)।

## पृष्ठांकन

| पृष्ठांकन | उपयोग |
|---|---|
| `Paging::pages(n)` | क्रमांकित पृष्ठ, 50 पंक्तियाँ जब तक कुछ और तय न हो |
| `Paging::infinite(n)` | स्क्रॉल करते समय और पंक्तियाँ लोड करता है |
| `Paging::virtual_scroll()` | बहुत बड़ी सूचियाँ; पृष्ठ में केवल दिख रही पंक्तियाँ होती हैं |
| `Paging::none()` | छोटी सूचियाँ जो एक पृष्ठ में समा जाएँ |

क्लाइंट कुछ भी माँगे, सर्वर पृष्ठ का आकार सीमित रखता है।

## कॉलम

| कॉल | परिणाम |
|---|---|
| `.width(px)`, `.flex(n)` | निश्चित चौड़ाई या खाली चौड़ाई का एक हिस्सा |
| `.filter()` | प्रकार के अनुरूप फ़िल्टर बॉक्स: टेक्स्ट, संख्या सीमा, तारीख़ सीमा, चयन सूची |
| `.sum()`, `.avg()`, `.min()`, `.max()` | फ़ुटर में योग। टेक्स्ट कॉलम पर `sum` कंपाइल नहीं होता |
| `.badge(\|v\| tone)` | मान को टोन वाले लेबल के रूप में, उसकी हल्की छटा पर दिखाता है |
| `.status(\|v\| tone)` | मान को स्टेटस के रूप में दिखाता है: सादे टेक्स्ट से पहले टोन का एक बिंदु, बैज से शांत |
| `.progress(max)` | 0 से `max` तक की पट्टी |
| `.check_icon()` | `true` के लिए सही का निशान |
| `.link(\|row\| effect)` | सेल को ऐसा लिंक बनाता है जो effect चलाता है |
| `.pin_start()`, `.hidden()` | शुरुआत में पिन, या उपयोगकर्ता के दिखाने तक छिपा |
| `.editable()` | उसी जगह संपादन योग्य, `.edit_with(action)` से सहेजा जाता है |
| `.plain()` | पूर्णांक बिना समूह विभाजक के, कोड और वर्षों के लिए |
| `.wrap()` | लंबे मान कई पंक्तियों में लिपटते हैं। इसके बिना सेल एक पंक्ति में रहते हैं और ellipsis पर ख़त्म होते हैं, ताकि हर पंक्ति की ऊँचाई एक जैसी रहे |

पंक्तियाँ 40 px की होती हैं (कॉम्पैक्ट घनत्व में 32 px) और केवल रेखाओं से बँटी होती हैं; बहुत चौड़े ग्रिड के लिए `DataGrid::zebra()` हर दूसरी पंक्ति को छायांकित करता है। सेल के लिंक 500 वज़न के टेक्स्ट जैसे दिखते हैं और होवर पर रेखांकित होते हैं।

संख्याएँ और राशियाँ दाईं ओर संरेखित होती हैं और समान चौड़ाई वाले अंकों में दिखती हैं। पूर्णांक ग्रिड सेल में और `DescriptionList` जैसे प्रदर्शित मानों में उपयोगकर्ता के लोकेल के अनुसार समूहित होते हैं (अंग्रेज़ी में `1,234,567`, तुर्की में `1.234.567`); `#[key]` id कभी समूहित नहीं होते, और `.plain()` एक कॉलम के लिए समूहन बंद करता है।

## पंक्तियाँ

- `.on_open(|row| effect)`: पंक्ति पर डबल क्लिक या Enter।
- `.row_action(label, action, |row| input)`: हर पंक्ति पर एक बटन।
- `.bulk_action(label, action, M::id)`: चुनी गई पंक्तियों पर चलती है।
- `.row_action` या `.bulk_action` के ठीक बाद `.confirm(message)` और `.tone(Tone::Danger)`: वह क्रिया चलने से पहले `ActionButton::confirm` वाले संवाद में ही पूछता है (फ़ोकस पहले रद्द करें पर रहता है), और उसका बटन लाल दिखाता है।
- `.row_menu(|row| ContextMenu)`: पंक्ति का राइट क्लिक मेनू।
- `.row_style(|row| Option<RowStyle>)`: danger, warning या success पंक्तियाँ।
- `.exportable()`: फ़िल्टर की गई पंक्तियों का CSV निर्यात, सर्वर द्वारा स्ट्रीम किया गया।

## रन टाइम पर पता चलने वाले कॉलम

`DataGrid::dynamic(action)` उन नतीजों के लिए है जिनके कॉलम पंक्तियों के साथ आते हैं, जैसे SQL प्रीव्यू। action एक `DynamicPage` लौटाता है जिसमें कॉलम (नाम, लेबल और `Integer`, `Decimal` या `Date` जैसा एक `DynamicKind`) और JSON में पंक्तियों का एक पेज होता है:

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

पेजिंग, वर्चुअल स्क्रॉलिंग, कीबोर्ड का उपयोग और उपयोगकर्ता की भाषा में संख्याओं और तारीखों का फ़ॉर्मैट टाइप वाले grid की तरह ही काम करते हैं। नाम, लेबल और सेल एस्केप किए जाते हैं, और सर्वर कॉलम, पंक्तियों और टेक्स्ट की लंबाई की सीमा तय करता है। यह grid न सॉर्ट करता है न फ़िल्टर, और इसकी पंक्तियाँ केवल पढ़ने के लिए हैं। [DataGrid संदर्भ](grid.md) देखें।

## खोज और बैंड

`.search()` ग्रिड के ऊपर एक खोज बॉक्स रखता है। एक्शन को टेक्स्ट `DataQuery::q` के रूप में मिलता है, जिसे
`DataQuery::apply` टेक्स्ट फ़ील्ड से मिलाता है, मास्क किए गए फ़ील्ड से कभी नहीं।
`.band(label, |b| b.column(..).column(..))` कॉलम को एक हेडर सेल के नीचे समूहित करता है।
[DataGrid संदर्भ](grid.md) देखें।

## उसी जगह रिफ़्रेश

आख़िर में कॉल किया गया `.listens(topic)` topic का नाम लिए जाने पर ग्रिड को उसी जगह फिर से लोड कराता है ([Topic](effects.md#topics) देखें): `ui.refresh(topic)` के बाद वह अपने क्रम और फ़िल्टर के साथ मौजूदा पेज फिर से लोड करता है, और `.drives(topic)` वाला फ़िल्टर कंट्रोल बदलने के बाद नए Screen params के साथ पहले पेज से शुरू करता है। ग्रिड हर अनुमति जाँच के साथ `POST /_zeyn/grid` से फिर से लोड होता है, और फ़िल्टर से बदला गया Screen param Screen फ़ंक्शन के ज़रिए ग्रिड तक पहुँचता है, उदाहरण के लिए scope के रूप में:

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

[DataGrid संदर्भ](grid.md) में सहेजे गए व्यू, master detail, समूहन, ट्री डेटा, इनलाइन संपादन, सेल नेविगेशन, कॉपी और पेस्ट, कॉलम वर्चुअलाइज़ेशन और सुगम्यता है।
