# पाठ और सामग्री

लेख, प्रोफ़ाइल और सूचियों जैसे पढ़ने वाले पृष्ठ कुछ छोटे कंपोनेंट से बनते हैं। ये सर्वर पर रेंडर होने वाला सादा HTML हैं, इनमें कोई स्क्रिप्ट नहीं है, और हर पाठ locale फ़ाइलों से आता है या escape किया जाता है।

@live content

## Heading

`Heading::h2(text)` से `Heading::h6(text)` तक। screen का शीर्षक पृष्ठ का एकमात्र `h1` है, इसलिए यहाँ `h1` नहीं है: वह स्तर चुनें जो रूपरेखा में अगला आता है, वह आकार नहीं जो आप चाहते हैं। `.id("anchor")` से लिंक इसकी ओर इशारा कर सकते हैं।

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

escape किए हुए पाठ का एक अनुच्छेद। `.muted()` द्वितीयक पाठ का रंग और `.small()` छोटा फ़ॉन्ट आकार उपयोग करता है। लंबे शब्द अगली पंक्ति में चले जाते हैं।

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

एक पंक्ति में छोटी जानकारियाँ, जैसे तारीख़, पढ़ने का समय और श्रेणी। विभाजक (जब तक `.separator("/")` कुछ और न कहे, एक मध्य बिंदु) स्क्रीन रीडर से छिपा रहता है, और खाली हिस्से छोड़ दिए जाते हैं।

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

पिछले और अगले आइटम के लिंक, जिन पर `rel="prev"` और `rel="next"` होता है। ये पृष्ठ को फिर से लोड किए बिना खुलते हैं। "पिछला" और "अगला" शब्द framework के पाठों से आते हैं; `.prev_title` और `.next_title` इनके नीचे आइटम का शीर्षक जोड़ते हैं। दाएँ से बाएँ वाली भाषाओं में तीर पलट जाते हैं, और फ़ोन पर दोनों लिंक ऊपर-नीचे आ जाते हैं।

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

ऐसे ग्रिड में चाइल्ड, जिसके कॉलमों की संख्या उपलब्ध चौड़ाई के अनुसार बदलती है: कम से कम `min` पिक्सल (तय न करने पर 240) चौड़े जितने कॉलम समा सकें, और फ़ोन पर एक कॉलम। ब्राउज़र इसे `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` से बिछाता है, इसलिए कोई ब्रेकपॉइंट और कोई स्क्रिप्ट नहीं है। इसमें कोई भी नोड रखा जा सकता है: कार्ड, पैनल, ग्रिड। यह नाम इसे `DataGrid` से अलग रखता है, जो डेटा की पंक्तियाँ दिखाता है।

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

एक `<ul>` में एक के नीचे एक पंक्तियाँ। हर `ListItem` का एक शीर्षक होता है और उसमें शुरू में एक छोटी छवि (कार्ड की छवियों जैसे ही सुरक्षित पते: एक पथ, `https:` या `data:image/`), एक उपशीर्षक और अंत में छोटा मेटा: पाठ, या `RelativeTime` जैसा इनलाइन कॉम्पोनेंट। `.link(screen, params)` के साथ पूरी पंक्ति एक लिंक बन जाती है, इसलिए यह टैब क्रम में एक ही पड़ाव है, इसका फ़ोकस रिंग दिखाई देता है, और यह पृष्ठ को फिर से लोड किए बिना खुलती है। लंबा पाठ अगली पंक्ति में चला जाता है; फ़ोन पर मेटा पाठ शीर्षक के नीचे आ जाता है, और दाएँ से बाएँ वाले पृष्ठों में पंक्ति उलट जाती है।

`List::new()` उसे दिए गए आइटम रखती है। `List::of(action)` पंक्तियाँ उसी प्रकार के action से लोड करती है जो DataGrid उपयोग करता है, और [कार्ड ग्रिड](catalog.md#card-grid) की तरह पृष्ठांकन करती है: जब तक `paging` कुछ और न कहे, 20 पंक्तियाँ और एक "Show more" बटन, क्रम के विकल्प, `.search(text)` से सर्वर-पक्ष की खोज, और हर पृष्ठ के लिए action की पहुँच की जाँच।

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
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

किसी व्यक्ति या टीम की गोल तस्वीर, 32, 40 या 64 पिक्सल की (`AvatarSize::Small`, `Medium`, `Large`)। नाम अनिवार्य है: यह वैकल्पिक पाठ है और उन आद्याक्षरों का स्रोत है जो तब दिखते हैं जब कोई तस्वीर न हो, या जब तस्वीर ऐसे होस्ट पर हो जिसे ऐप का `img-src` अनुमति नहीं देता (`Image` जैसा ही नियम)। आद्याक्षर भाषा के बड़े अक्षरों का पालन करते हैं, इसलिए तुर्की का `i` `İ` बन जाता है। जब नाम अवतार के ठीक बगल में लिखा हो, तो `.decorative()` अवतार को स्क्रीन रीडर से छिपा देता है ताकि वे नाम दो बार न पढ़ें।

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

वह सामग्री जो पाठक के खोलने तक छिपी रहती है, जैसे किसी प्रश्न का उत्तर। यह एक नेटिव डिस्क्लोज़र (`<details>` और `<summary>`) है, इसलिए यह बिना किसी स्क्रिप्ट के कीबोर्ड और स्क्रीन रीडर के साथ काम करता है, और सामग्री खुलने तक पढ़ने के क्रम से बाहर रहती है। लेबल ऐप से आता है।

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## Chips

छोटे गोल लिंक की एक पंक्ति जो कई पंक्तियों में लपेटी जाती है। `Chips::links(label)`, किसी लेख के टैग जैसे स्क्रीन के सादे लिंक, label से नामित `nav` में रखता है। `Chips::filter(label, screen, params)` किसी स्क्रीन param पर toggle बटन बनाता है: हर chip उसी स्क्रीन का लिंक है जिसका मान param में जोड़ा या हटाया जाता है, इसलिए सर्वर प्राप्त params से `aria-pressed` रेंडर करता है और ब्राउज़र में कुछ भी नहीं रखा जाता। chip दबाने पर नया पता `nav.js` के माध्यम से बिना पेज रीलोड के खुलता है, और फ़ोकस व स्क्रॉल स्थिति chip पर ही रहते हैं। `.multiple()` कॉल किए बिना एक समय में एक ही मान दबा होता है; params में जो मान विकल्प नहीं हैं उन्हें अनदेखा किया जाता है। कीबोर्ड: Tab chip के बीच जाता है, Enter या Space एक को दबाता है। दबा हुआ chip एक टिक चिह्न भी दिखाता है, इसलिए स्थिति केवल रंग पर निर्भर नहीं रहती। स्क्रिप्ट (`chips.js`, 1 KB से कम) केवल फ़िल्टर chip वाले पेजों पर लोड होती है।

```rust
#[screen(route = "/articles", public)]
pub fn articles(p: ArticleParams) -> Screen {
    Screen::new(t!("blog.articles")).body(
        Chips::filter(t!("blog.tags"), articles, |tags| ArticleParams { tags: tags.join(",") })
            .current(p.tags.split(','))
            .option("guide", t!("blog.guides"))
            .option("news", t!("blog.news"))
            .multiple(),
    )
}
```

@live chips


## Hero

पेज के शीर्ष पर एक बड़ा खंड: शीर्षक, एक पंक्ति का टेक्स्ट और बटन जैसी कार्रवाइयों के साथ पृष्ठभूमि की छवि। यह डिफ़ॉल्ट रूप से स्थिर है। `.slide(title)` एक और स्लाइड जोड़ता है, और `.text`, `.image` तथा `.action` हमेशा अंतिम स्लाइड को सेट करते हैं। कई स्लाइड होने पर यह W3C APG carousel पैटर्न का पालन करता है: स्लाइड चुनने वाले बटन, हर स्लाइड "2 / 3" नाम का समूह, और चुनी गई स्लाइड की विनम्र घोषणा। स्लाइड केवल तब बदलती हैं जब उपयोगकर्ता चुनता है, जब तक ऐप `.rotate(seconds)` (4 से 30) कॉल न करे। तब रोकने और चलाने का बटन सबसे पहले आता है (WCAG 2.2.2), पॉइंटर hero पर होने या फ़ोकस उसके भीतर होने और टैब छिपा होने पर घूमना रुक जाता है, और यदि उपयोगकर्ता कम गति पसंद करता है तो यह कभी शुरू नहीं होता; स्लाइडों के चलते समय उनकी घोषणा नहीं की जाती। स्क्रिप्ट के बिना पहली स्लाइड दिखती है।

छवि `Image` के नियमों का पालन करती है (ऐप का कोई पथ या ऐप के `img-src` के किसी होस्ट पर `https` पता)। यह टेक्स्ट के पीछे सजावट है, इसलिए इसका वैकल्पिक टेक्स्ट खाली है: जो महत्वपूर्ण है उसे शीर्षक और टेक्स्ट में रखें, जो कंट्रास्ट के लिए गहरे ग्रेडिएंट पर रहते हैं। पहली स्लाइड की छवि तेज़ पहली पेंटिंग के लिए उच्च प्राथमिकता से लोड होती है, बाकी लेज़ी। स्क्रिप्ट (`hero.js`, 1.6 KB gzip) केवल स्लाइड वाले पेजों पर लोड होती है।

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero


## Reactions

ऐप द्वारा चुने गए reaction बटनों की एक पंक्ति, हर एक की एक key, एक नाम, emoji जैसा एक प्रतीक और उसकी गिनती। `Reactions::of(action, target)` उस आइटम का नाम देता है जिसके बारे में reactions हैं; target स्क्रीन तय करती है और उसे ब्राउज़र से कभी नहीं पढ़ा जाता। बटन दबाने पर reaction `POST /_zeyn/data` को भेजा जाता है, जो CSRF जाँचता है, लिखने की दर सीमा में गिना जाता है, स्क्रीन खोलता है, action की अनुमति जाँचता है और action चलने से पहले सेट के बाहर के reaction को अस्वीकार करता है। action को एक `ReactionPress` (target, reaction, pressed) मिलता है, वह उसे सहेजता है और नया `ReactionState` (गिनतियाँ और उपयोगकर्ता के अपने reactions) लौटाता है, और गिनतियाँ व `aria-pressed` स्थितियाँ वहीं अपडेट हो जाती हैं। हर बटन reaction के नाम और उसकी गिनती से नामित toggle button है; प्रतीक स्क्रीन रीडर से छिपा रहता है। `.state(...)` पहले रेंडर की गिनतियाँ देता है, इसलिए वे स्क्रिप्ट के बिना दिखती हैं। स्क्रिप्ट (`reactions.js`, 1.3 KB gzip) केवल reactions वाले पेजों पर लोड होती है।

```rust
#[action(permission = "answers.react")]
pub async fn react_to_answer(press: ReactionPress, db: Db, ctx: Ctx) -> Result<ReactionState> {
    db.toggle_reaction(&ctx, &press.target, &press.reaction, press.pressed).await?;
    db.reaction_state(&ctx, &press.target).await
}

Reactions::of(react_to_answer, format!("answer-{}", answer.id))
    .reaction("helpful", t!("answers.helpful"), "\u{1f44d}")
    .reaction("thanks", t!("answers.thanks"), "\u{1f64f}")
    .state(answer.reactions.clone())
```

@live reactions


## RelativeTime

एक क्षण जो पेज की भाषा में सापेक्ष समय के रूप में दिखाया जाता है, जैसे "3 मिनट पहले" या "2 दिन में"। सर्वर उपयोगकर्ता के समय क्षेत्र में सटीक तिथि और समय `<time datetime="...">` के भीतर लिखता है, इसलिए HTML हर मिनट वही रहता है और कैश किया गया पेज भी सही रहता है। रनटाइम पेज लोड होने पर, `nav.js` के कोई स्क्रीन बदलने पर और हर मिनट `Intl.RelativeTimeFormat` से टेक्स्ट दोबारा लिखता है; सटीक समय tooltip में रहता है। स्क्रिप्ट के बिना सटीक समय ही टेक्स्ट है।

```rust
RelativeTime::new(post.published)
```


## Share

लिंक साझा करने वाला बटन: जहाँ ब्राउज़र में Web Share API है (अधिकांश फ़ोन) वहाँ सिस्टम शेयर शीट, और अन्यथा क्लिपबोर्ड पर कॉपी, जिसकी सूचना toast देता है। यह दिखाया जा रहा पेज साझा करता है, या `Share::screen(screen, params)` से कोई स्क्रीन, जिसके params कंपाइलर जाँचता है। `.title` वह शीर्षक तय करता है जो शेयर शीट दिखाती है, और लेबल उपयोगकर्ता की भाषा में "साझा करें" होता है जब तक `.label` कोई और न दे। सर्वर बटन को छिपा हुआ रेंडर करता है और उसकी स्क्रिप्ट उसे दिखाती है, इसलिए बिना स्क्रिप्ट वाले पेज में ऐसा बटन नहीं होता जो कुछ न करे। स्क्रिप्ट (`share.js`, 1 KB से कम) केवल शेयर बटन वाले पेजों पर लोड होती है।

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


## AlphaIndex

अक्षरों की एक पट्टी, जिसका हर अक्षर किसी ऐसे screen का लिंक है जिसके params में वह अक्षर होता है, जैसे वे लोग जिनके नाम उससे शुरू होते हैं। अक्षर उपयोगकर्ता की भाषा से, उसी के अपने क्रम में आते हैं (`zeyn::ui::alphabet`): तुर्की में `Ç Ğ I İ Ö Ş Ü` अपनी जगह पर हैं, स्पेनिश में `Ñ` है, रूसी और अरबी की अपनी वर्णमालाएँ हैं, और हर दूसरी भाषा को `A` से `Z` मिलता है। जो screen दिखाया जा रहा है उसका अक्षर `aria-current="page"` से चिह्नित होता है (तुर्की का `i` `İ` को चिह्नित करता है), और `.other()` उन नामों के लिए अंत में एक `#` लिंक जोड़ता है जो किसी और चीज़ से शुरू होते हैं। लिंक पृष्ठ को फिर से लोड किए बिना खुलते हैं और फ़ोन पर कई पंक्तियों में बँट जाते हैं।

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

किसी action की पंक्तियों के सात दिन वाले कॉलम, जैसे किसी सप्ताह की कक्षाएँ, शिफ़्ट या कार्यक्रम। action वही प्रकार है जो DataGrid उपयोग करता है और उसकी जाँच भी उसी तरह होती है। `.day` एक पंक्ति को उसके दिन से जोड़ता है: एक `Date`, एक `Option<Date>` (बिना दिन वाली पंक्तियाँ छोड़ दी जाती हैं) या 1 (सोमवार) से 7 (रविवार) तक का सप्ताह-दिन संख्या; इसके अलावा कुछ भी कंपाइल नहीं होता। हर पंक्ति `ListItem` के रूप में दिखाई जाती है, इसलिए वह किसी screen से जुड़ सकती है। सप्ताह उपयोगकर्ता के पहले दिन से शुरू होता है (तुर्की में सोमवार, अमेरिकी अंग्रेज़ी में रविवार), आज चिह्नित होता है, बिना पंक्तियों वाले दिन यह बताते हैं, और संकरी स्क्रीन पर दिन ऊपर-नीचे आ जाते हैं। सर्वर action के क्रम में अधिकतम `limit` पंक्तियाँ लोड करता है (तय न करने पर 100, अधिकतम 500); action को सप्ताह की पंक्तियाँ दें, उदाहरण के लिए तारीख़ पर एक फ़िल्टर के साथ।

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
