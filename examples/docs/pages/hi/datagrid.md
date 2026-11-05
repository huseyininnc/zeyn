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
| `.badge(\|v\| tone)` | मान को रंगीन बैज के रूप में दिखाता है |
| `.progress(max)` | 0 से `max` तक की पट्टी |
| `.check_icon()` | `true` के लिए सही का निशान |
| `.link(\|row\| effect)` | सेल को ऐसा लिंक बनाता है जो effect चलाता है |
| `.pin_start()`, `.hidden()` | शुरुआत में पिन, या उपयोगकर्ता के दिखाने तक छिपा |
| `.editable()` | उसी जगह संपादन योग्य, `.edit_with(action)` से सहेजा जाता है |
| `.plain()` | पूर्णांक बिना समूह विभाजक के, कोड और वर्षों के लिए |

संख्याएँ और राशियाँ दाईं ओर संरेखित होती हैं और समान चौड़ाई वाले अंकों में दिखती हैं। पूर्णांक ग्रिड सेल में और `DescriptionList` जैसे प्रदर्शित मानों में उपयोगकर्ता के लोकेल के अनुसार समूहित होते हैं (अंग्रेज़ी में `1,234,567`, तुर्की में `1.234.567`); `#[key]` id कभी समूहित नहीं होते, और `.plain()` एक कॉलम के लिए समूहन बंद करता है।

## पंक्तियाँ

- `.on_open(|row| effect)`: पंक्ति पर डबल क्लिक या Enter।
- `.row_action(label, action, |row| input)`: हर पंक्ति पर एक बटन।
- `.bulk_action(label, action, M::id)`: चुनी गई पंक्तियों पर चलती है।
- `.row_action` या `.bulk_action` के ठीक बाद `.confirm(message)` और `.tone(Tone::Danger)`: वह क्रिया चलने से पहले `ActionButton::confirm` वाले संवाद में ही पूछता है (फ़ोकस पहले रद्द करें पर रहता है), और उसका बटन लाल दिखाता है।
- `.row_menu(|row| ContextMenu)`: पंक्ति का राइट क्लिक मेनू।
- `.row_style(|row| Option<RowStyle>)`: danger, warning या success पंक्तियाँ।
- `.exportable()`: फ़िल्टर की गई पंक्तियों का CSV निर्यात, सर्वर द्वारा स्ट्रीम किया गया।

[DataGrid संदर्भ](grid.md) में सहेजे गए व्यू, master detail, समूहन, ट्री डेटा, इनलाइन संपादन, सेल नेविगेशन, कॉपी और पेस्ट, कॉलम वर्चुअलाइज़ेशन और सुगम्यता है।
