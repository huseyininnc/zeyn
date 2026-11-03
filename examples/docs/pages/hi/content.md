# टेक्स्ट और सामग्री

लेख, प्रोफ़ाइल और सूचियों जैसे पढ़ने वाले पृष्ठ कुछ छोटे कंपोनेंट से बनते हैं। ये सर्वर पर रेंडर किया गया सादा HTML हैं, इनमें कोई स्क्रिप्ट नहीं है, और हर टेक्स्ट locale फ़ाइलों से आता है या एस्केप किया जाता है।

@live content

## Heading

`Heading::h2(text)` से `Heading::h6(text)` तक। Screen का शीर्षक पृष्ठ का एकमात्र `h1` है, इसलिए यहाँ `h1` नहीं है: वह स्तर चुनें जो दस्तावेज़ की रूपरेखा से मेल खाता हो, न कि वह आकार जो आप चाहते हैं। `.id("anchor")` से लिंक इस शीर्षक की ओर इशारा कर सकते हैं।

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

एस्केप किए गए टेक्स्ट का एक अनुच्छेद। `.muted()` द्वितीयक टेक्स्ट रंग का और `.small()` छोटे फ़ॉन्ट आकार का उपयोग करता है। लंबे शब्द अगली पंक्ति में चले जाते हैं।

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

एक पंक्ति में छोटी जानकारियाँ, जैसे तारीख, पढ़ने का समय और श्रेणी। विभाजक (मध्य बिंदु, जब तक `.separator("/")` कुछ और न कहे) स्क्रीन रीडर से छिपा रहता है, और खाली हिस्से छोड़ दिए जाते हैं।

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

`rel="prev"` और `rel="next"` के साथ पिछले और अगले आइटम के लिंक। ये पृष्ठ को दोबारा लोड किए बिना खुलते हैं। "Previous" और "Next" शब्द फ़्रेमवर्क के अपने टेक्स्ट से आते हैं; `.prev_title` और `.next_title` उनके नीचे आइटम का शीर्षक जोड़ते हैं। दाएँ से बाएँ लिखी जाने वाली भाषाओं में तीर उलट जाते हैं, और फ़ोन पर दोनों लिंक एक के नीचे एक आ जाते हैं।

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

एक ग्रिड में चाइल्ड तत्व, जिसके कॉलम की संख्या उपलब्ध चौड़ाई के अनुसार बदलती है: कम से कम `min` पिक्सेल (सेट न हो तो 240) के उतने कॉलम जितने समा सकें, और फ़ोन पर एक कॉलम। ब्राउज़र इसे `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` से व्यवस्थित करता है, इसलिए न कोई ब्रेकपॉइंट है न स्क्रिप्ट। इसमें कोई भी नोड रखा जा सकता है: कार्ड, पैनल, ग्रिड। यह नाम इसे `DataGrid` से अलग रखता है, जो डेटा की पंक्तियाँ दिखाता है।

```rust
AutoGrid::new()
    .min(220)
    .gap(4)
    .add(Card::new().title(t!("home.orders")))
    .add(Card::new().title(t!("home.stock")))
    .add(Card::new().title(t!("home.people")))
```

@live autogrid

## List

`<ul>` में एक के नीचे एक पंक्तियाँ। हर `ListItem` का एक शीर्षक होता है और उसमें शुरुआत में एक छोटी छवि (कार्ड छवियों जैसे ही सुरक्षित पते: एक पाथ, `https:` या `data:image/`), एक उपशीर्षक और अंत में छोटा मेटा टेक्स्ट हो सकता है। `.link(screen, params)` के साथ पूरी पंक्ति एक ही लिंक होती है, इसलिए टैब क्रम में वह दिखने वाले फ़ोकस रिंग के साथ एक ही पड़ाव है, और वह पृष्ठ को दोबारा लोड किए बिना खुलती है। लंबा टेक्स्ट अगली पंक्ति में जाता है; फ़ोन पर मेटा टेक्स्ट शीर्षक के नीचे चला जाता है, और दाएँ से बाएँ वाले पृष्ठ पंक्ति को उलट देते हैं।

`List::new()` उसे दिए गए आइटम रखता है। `List::of(action)` उसी तरह के action से पंक्तियाँ लोड करता है जिसका उपयोग DataGrid करता है, और [कार्ड ग्रिड](catalog.md#card-grid) की तरह पेज बनाता है: जब तक `paging` कुछ और न कहे, "Show more" बटन के साथ 20 पंक्तियाँ, सॉर्ट विकल्प, `.search(text)` से सर्वर साइड खोज, और हर पेज के लिए action की पहुँच की जाँच।

```rust
List::new()
    .title(t!("team.title"))
    .item(ListItem::new("Ada Lovelace").subtitle(t!("team.math")))
    .item(ListItem::new("Alan Turing").subtitle(t!("team.computing")).meta(t!("team.lead")))

List::of(search_people)
    .title(t!("people.title"))
    .item(|p: &Person| {
        ListItem::new(p.name.clone())
            .image(p.photo.clone())
            .subtitle(p.city.clone())
            .meta(p.joined.format(&locale))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

किसी व्यक्ति या टीम की 32, 40 या 64 पिक्सेल की गोल तस्वीर (`AvatarSize::Small`, `Medium`, `Large`)। नाम आवश्यक है: यही वैकल्पिक टेक्स्ट है और उन आद्याक्षरों का स्रोत है जो तब दिखते हैं जब कोई तस्वीर न हो, या तस्वीर ऐसे होस्ट पर न हो जिसकी अनुमति ऐप का `img-src` देता है (`Image` जैसा ही नियम)। आद्याक्षर भाषा के बड़े अक्षरों के नियम का पालन करते हैं, इसलिए तुर्की `i` बदलकर `İ` हो जाता है। जब नाम अवतार के ठीक बगल में लिखा हो, तो `.decorative()` अवतार को स्क्रीन रीडर से छिपा देता है ताकि वे नाम दो बार न पढ़ें।

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

ऐसी सामग्री जो पाठक के खोलने तक छिपी रहती है, जैसे किसी प्रश्न का उत्तर। यह एक मूल प्रकटीकरण तत्व है (`<details>` और `<summary>`), इसलिए यह बिना किसी स्क्रिप्ट के कीबोर्ड और स्क्रीन रीडर के साथ काम करता है, और खुलने तक सामग्री पढ़ने के क्रम से बाहर रहती है। लेबल ऐप से आता है।

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

अक्षरों की एक पट्टी, जिसमें हर अक्षर एक ऐसी Screen का लिंक है जिसके params में वह अक्षर है, जैसे वे लोग जिनके नाम उस अक्षर से शुरू होते हैं। अक्षर उपयोगकर्ता की भाषा से उसी भाषा के अपने क्रम में आते हैं (`zeyn::ui::alphabet`): तुर्की में `Ç Ğ I İ Ö Ş Ü` अपनी जगह पर हैं, स्पेनिश में `Ñ` है, रूसी और अरबी की अपनी वर्णमालाएँ हैं, और बाकी हर भाषा को `A` से `Z` तक मिलते हैं। दिखाई जा रही Screen का अक्षर `aria-current="page"` से चिह्नित होता है (तुर्की `i`, `İ` को चिह्नित करता है), और `.other()` किसी और चीज़ से शुरू होने वाले नामों के लिए अंत में एक `#` लिंक जोड़ता है। लिंक पृष्ठ को दोबारा लोड किए बिना खुलते हैं और फ़ोन पर कई पंक्तियों में चले जाते हैं।

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

किसी action से आई पंक्तियों के सात दिन वाले कॉलम, जैसे किसी सप्ताह की कक्षाएँ, शिफ़्ट या कार्यक्रम। action उसी तरह का है जिसका उपयोग DataGrid करता है और उसी तरह जाँचा जाता है। `.day` किसी पंक्ति को उसके दिन से जोड़ता है: एक `Date`, एक `Option<Date>` (बिना तारीख वाली पंक्तियाँ छोड़ दी जाती हैं) या 1 (सोमवार) से 7 (रविवार) तक की सप्ताह के दिन की संख्या; इसके अलावा कुछ भी कंपाइल नहीं होता। हर पंक्ति एक `ListItem` के रूप में दिखती है, इसलिए वह किसी Screen से लिंक हो सकती है। सप्ताह उपयोगकर्ता के पहले सप्ताह दिवस से शुरू होता है (तुर्की में सोमवार, अमेरिकी अंग्रेज़ी में रविवार), आज का दिन चिह्नित होता है, बिना पंक्तियों वाले दिन यह बताते हैं, और संकरी स्क्रीन पर दिन एक के नीचे एक आ जाते हैं। सर्वर action के क्रम में अधिकतम `limit` पंक्तियाँ (सेट न हो तो 100, अधिकतम 500) लोड करता है; action को सप्ताह की पंक्तियाँ दें, उदाहरण के लिए तारीख पर एक फ़िल्टर के साथ।

```rust
WeekSchedule::of(this_week_classes)
    .title(t!("classes.this_week"))
    .day(|c: &Class| c.date)
    .item(|c: &Class| {
        ListItem::new(c.name.clone())
            .meta(c.starts.to_string())
            .link(class, ClassParams { id: c.id })
    })
```

@live week
