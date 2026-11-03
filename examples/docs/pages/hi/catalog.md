# कार्ड, रेल और खोज

शॉप जैसे सार्वजनिक पृष्ठ पंक्तियों को तालिका की जगह कार्ड के रूप में दिखाते हैं। `CardGrid`, `Rail` और `SearchBox` अपनी पंक्तियाँ उसी तरह की action से लोड करते हैं जैसी DataGrid इस्तेमाल करती है: वह `DataQuery` लेती है और `Page` लौटाती है। पृष्ठांकन, क्रम और खोज टेक्स्ट सर्वर पर चलते हैं, और पढ़ने के लिए action का एक्सेस चाहिए, जैसा DataGrid के लिए होता है।

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## कार्ड ग्रिड

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **कार्ड।** `Card::image(image, title)` एक पंक्ति से एक कार्ड बनाता है, `text`, `badge`, `alt` और `link` (एक Screen और उसके पैरामीटर; पूरा कार्ड एक लिंक है और बिना पृष्ठ रीलोड के खुलता है) के साथ। छवियाँ lazy होती हैं, और `image_size` की चौड़ाई और ऊँचाई के साथ ताकि पृष्ठ उछले नहीं। छवि का वैकल्पिक टेक्स्ट खाली रहता है, क्योंकि उसके पास का शीर्षक कार्ड का नाम बताता है; जब चित्र अधिक कुछ कहता हो तो `alt` सेट करें। पथ, `https:` या `data:image/` के अलावा अन्य छवि पते छोड़ दिए जाते हैं।
- **कार्ड पर और कुछ।** `subtitle` शीर्षक के नीचे एक हल्की दूसरी पंक्ति जोड़ता है (लेखक, श्रेणी)। `corner(badge)` एक बैज को छवि के अंतिम कोने पर रखता है, जबकि आरंभिक कोने पर `badge` रहता है; `badges([...])` पाठ के नीचे बैज की एक पंक्ति जोड़ता है, जैसे टैग। `progress(fraction, label)` छवि के निचले किनारे पर एक पतली पट्टी खींचता है, जो बताती है कि कितना काम हो चुका है: अनुपात 0 से 1 तक होता है, और स्क्रीन रीडर लेबल और प्रतिशत सुनाते हैं (`role="progressbar"` और `aria-valuenow`)। `horizontal()` कार्ड को एक पंक्ति में रखता है, आरंभ में छोटी छवि और उसके बगल में पाठ; ऐसे कार्डों वाला कार्ड ग्रिड या rail उन्हें चौड़े कॉलम देता है। ये सभी `CardGrid` और `Rail` में काम करते हैं।

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **पृष्ठांकन।** `Paging::infinite(n)` (डिफ़ॉल्ट, 24) "और दिखाएँ" बटन दिखाता है जो स्क्रॉल करके दिखाई देने पर भी चलता है; बटन दबाने पर फ़ोकस पहले नए कार्ड पर जाता है। `Paging::pages(n)` पिछला और अगला बटन और पृष्ठ संख्या दिखाता है। पृष्ठ का आकार सर्वर तय करता है; क्लाइंट केवल बताता है कि कहाँ से जारी रखना है।
- **क्रम।** हर `sort_option` एक क्रम है जिसे उपयोगकर्ता select से चुन सकते हैं; पहला वह क्रम है जिसमें कार्ड शुरू होते हैं। सर्वर केवल यही क्रम स्वीकार करता है, और क्लाइंट के भेजे फ़िल्टर और खोज टेक्स्ट हटा देता है।
- **खोज।** `.search(text)` केवल वही पंक्तियाँ दिखाता है जो मुक्त टेक्स्ट ([`DataQuery::q`](#free-text)) से मेल खाती हैं, जैसे परिणाम स्क्रीन का टेक्स्ट।
- **मास्क किए फ़ील्ड।** कार्ड एक क्लोज़र से बनते हैं जो कच्ची पंक्ति देखता है। `#[mask]` फ़ील्ड वाले मॉडल के लिए, कार्ड केवल उन उपयोगकर्ताओं को मिलते हैं जो उन्हें प्रकट कर सकते हैं; बाकी सबको `Forbidden` मिलता है। सार्वजनिक कार्ड के लिए बिना मास्क किए फ़ील्ड वाला मॉडल इस्तेमाल करें।

## Rail

रेल एक कार्ड ग्रिड है जो एक पंक्ति में बिछा होता है और बगल में स्क्रॉल होता है, स्क्रॉल स्नैपिंग और पिछला व अगला बटन के साथ जो एक व्यू की चौड़ाई जितना स्क्रॉल करते हैं (सहजता से, जब तक उपयोगकर्ता ने कम गति न माँगी हो)। उपयोगकर्ता के अंत के पास स्क्रॉल करने पर यह अगला बैच, एक बार में 12, लोड करती है। कार्ड टैब क्रम में लिंक हैं और फ़ोकस मिलने पर व्यू में स्क्रॉल हो जाते हैं।

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## खोज बॉक्स

सुझावों वाला खोज फ़ील्ड, जो लिस्ट बॉक्स पॉपअप के साथ W3C APG combobox पैटर्न का पालन करता है। उपयोगकर्ता के टाइप करने पर यह `instant` मिलीसेकंड रुकता है और सर्वर से सुझाव माँगता है: action टेक्स्ट को `DataQuery::q` के रूप में लेकर चलती है, और हर पंक्ति का सुझाव टेक्स्ट लौटता है (उसका पहला बिना मास्क वाला टेक्स्ट फ़ील्ड, जब तक `suggestion` कुछ और न कहे)। Enter, या कोई सुझाव चुनना, बिना पृष्ठ रीलोड के उस टेक्स्ट के साथ परिणाम स्क्रीन खोलता है।

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

कीबोर्ड: नीचे और ऊपर तीर सुझावों में चलते हैं (नीचे उन्हें खोलता है), Enter चुने हुए सुझाव या टाइप किए टेक्स्ट से खोजता है, Escape सुझाव बंद करता है और फिर फ़ील्ड साफ़ करता है। एक लाइव रीजन स्क्रीन रीडर को बताता है कि कितने सुझाव हैं।

परिणाम स्क्रीन `SearchParams` (`q`, जो मेनू से स्क्रीन खुलने पर खाली होता है) लेती है। सर्वर टेक्स्ट को `MAX_SEARCH_QUERY` (100) अक्षरों तक काटता है, तय करता है कि कितने सुझाव भेजने हैं (8, जब तक `limit` कुछ और न कहे, अधिकतम 20), स्क्रीन का एक्सेस, feature के गार्ड और action की अनुमति जाँचता है, और हर सुझाव कॉल को लेखन दर सीमा (`App::write_limit`) में गिनता है।

## मुक्त टेक्स्ट

`DataQuery::q` मुक्त टेक्स्ट है: वे पंक्तियाँ जिनके टेक्स्ट फ़ील्ड में वह हो, अक्षर के छोटे-बड़े होने की अनदेखी करके। तुर्की के बिंदु वाले और बिना बिंदु वाले `i` (`İ`, `ı`) `i` से मेल खाते हैं, इसलिए "istanbul" को "İSTANBUL" मिल जाता है। `#[mask]` वाले फ़ील्ड में कभी खोज नहीं होती, खाली टेक्स्ट हर पंक्ति से मेल खाता है और 100 अक्षरों से लंबा टेक्स्ट अनदेखा किया जाता है। `DataQuery::apply` और SQL डेटा परतें (बाउंड पैरामीटर के साथ `LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...`) इसे एक ही तरह पढ़ती हैं।
