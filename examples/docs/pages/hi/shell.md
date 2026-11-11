# शेल और कमांड

शेल हर Screen के चारों ओर का फ़्रेम है: मेनू, भाषा, थीम और समय क्षेत्र चयनकर्ता, टोस्ट और कमांड पैलेट। जो ऐप डेस्कटॉप टूल की तरह काम करते हैं वे मेनू बार, टूलबॉक्स, प्रॉपर्टी इंस्पेक्टर, स्टेटस बार और डॉकिंग पैनल जोड़ते हैं।

## मेनू

मेनू feature के `.menu(...)` कॉल से आता है। जो आइटम उपयोगकर्ता नहीं खोल सकता वे HTML से हटा दिए जाते हैं, और जिस feature के गार्ड उपयोगकर्ता को अस्वीकार करें वह कुछ नहीं दिखाती। मौजूदा Screen का लिंक `aria-current="page"` रखता है और ब्रांड की हल्की छटा पर दिखता है।

`.item(...)` के बाद `.icon(Icon::...)` आइटम के लेबल से पहले एक आइकन रखता है; जब किसी समूह के आइटम में आइकन हों, तो समूह का लेबल अपने आइकन के बिना दिखता है। साइड मेनू ऐप के लोगो और नाम से शुरू होता है: `App::logo("/static/logo.svg")` (और डार्क थीम के लिए `App::logo_dark(...)`) ऐप द्वारा परोसा गया चिह्न सेट करता है, नहीं तो शेल `--z-brand` में एक गोल कोनों वाला वर्ग बनाता है जिसमें नाम का पहला अक्षर होता है। शेल तटस्थ रहता है; ब्रांड रंग मौजूदा आइटम, प्राइमरी बटन, फ़ोकस रिंग और लोगो को चिह्नित करता है।

आइकन 82 चित्रों के एक सेट (`Icon::ALL`) से आते हैं, 20 px ग्रिड पर 1.5 px गोल रेखा से बने, और मौजूदा टेक्स्ट रंग में 16 या 20 px पर दिखते हैं। हर एक `/_zeyn/icons.css` में एक CSS मास्क है, लगभग 25 KB (संपीड़ित 3 KB) की कैश की गई स्टाइलशीट जो `crates/zeyn/assets/icons/*.svg` से बनती है, इसलिए एक आइकन की कीमत HTML में एक क्लास है और स्क्रिप्ट में कुछ नहीं।

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

### नेस्टेड और समेटे जा सकने वाले समूह

`.sub(label, |m| ...)` आख़िरी समूह में एक नेस्टेड समूह जोड़ता है; उसके अंदर `item`, `icon`, `open` और `sub` वैसे ही काम करते हैं जैसे किसी समूह में। नेस्टेड समूह अपने समूह की आइटम के बाद आते हैं और शीर्ष स्तर के समूह से अधिकतम तीन स्तर नीचे तक जाते हैं (`MENU_DEPTH`); इससे गहरा मेनू ऐप को शुरू होते समय समूह का नाम बताने वाले संदेश के साथ रोक देता है।

```rust
.menu(|m| {
    m.group(t!("sales.menu"), Icon::Cart)
        .item(t!("sales.orders"), orders)
        .sub(t!("sales.reports"), |m| m.item(t!("sales.by_city"), by_city).item(t!("sales.archive"), archive))
        .accordion()
})
```

- **समेटे जा सकने वाले।** हर समूह का शीर्षक `aria-expanded` और `aria-controls` वाला एक बटन है (W3C APG डिस्क्लोज़र नेविगेशन पैटर्न)। सर्वर मौजूदा स्क्रीन वाले समूहों को खुला और बाक़ी को बंद बनाता है, और `nav.js` हर नेविगेशन के बाद नई स्क्रीन की शाखा खोलता है। स्क्रिप्ट के बिना सभी समूह खुले रहते हैं, इसलिए कुछ भी पहुँच से बाहर नहीं होता।
- **हमेशा खुला।** किसी समूह के बाद `.open()` उसे खुला रखता है: उसका शीर्षक बिना बटन का सादा टेक्स्ट होता है।
- **अकॉर्डियन।** app shell की किसी भी feature के मेनू पर `.accordion()` पूरे साइड मेनू को अकॉर्डियन बना देता है: एक समूह खोलने पर उसके खुले सहोदर समूह बंद हो जाते हैं।
- **कीबोर्ड।** ऊपर और नीचे दिखने वाले शीर्षकों और लिंक के बीच चलते हैं, Home और End पहले और आख़िरी पर जाते हैं। दायाँ बंद समूह खोलता है या खुले समूह के अंदर जाता है; बायाँ खुला समूह बंद करता है या बाहरी समूह के शीर्षक पर जाता है। Enter और Space समूह खोलते और बंद करते हैं। दाएँ से बाएँ लिखी जाने वाली भाषाओं में बायाँ और दायाँ आपस में बदल जाते हैं।
- **अनुमतियाँ और विलय।** एक ही लेबल वाले समूह हर स्तर पर features के बीच मिल जाते हैं, और जिस समूह की कोई आइटम उपयोगकर्ता नहीं खोल सकता वह नेस्टेड समूहों समेत हटा दिया जाता है। `App::header(|h| h.breadcrumb())` का ब्रेडक्रंब समूह का पूरा पथ दिखाता है। site shell नेस्टेड समूहों के लिंक उनके शीर्ष स्तर के समूह में दिखाता है।

### हर अनुरोध पर बनने वाले मेनू

`App::menu_source(source)` ऐसी मेनू प्रविष्टियाँ जोड़ता है जो हर अनुरोध के लिए बनती हैं, जैसे उपयोगकर्ता की भूमिकाओं से छाँटी गई किसी मेनू तालिका से। स्रोत एक async फ़ंक्शन है जो `Ctx` (पहचान, टेनेंट, भाषा) लेता है और `MenuNode` लौटाता है: सबसे ऊपर `MenuNode::group(label, children)`, और उसके भीतर `MenuNode::screen(label, "screen_name")` लिंक (`.params(json)` पथ के हिस्से और क्वेरी उसी तरह भरता है जैसे `screen_url`), `MenuNode::path(label, "/reports/sales")` लिंक और नेस्टेड समूह। `.icon(...)`, `.open()` और `.permission("menu.reports")` किसी भी नोड पर काम करते हैं। लेबल सादा पाठ होता है, जो किसी भी दूसरे पाठ की तरह एस्केप होता है, या `Text::key` के साथ कैटलॉग कुंजी।

ये समूह features के स्थिर मेनू के बाद आते हैं, और किसी स्थिर समूह के लेबल वाला समूह उसी में मिल जाता है। ब्रेडक्रम्ब, कमांड पैलेट और `nav.js` इनके लिंक को किसी भी दूसरे मेनू लिंक की तरह मानते हैं।

नीचे की तालिका PostgreSQL और Oracle दोनों पर चलती है। एक सपाट `SELECT` उसे पढ़ता है, और पेड़ Rust में `parent_id` से बनता है:

```sql
CREATE TABLE menu_items (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES menu_items (id),
    label       VARCHAR(100) NOT NULL,
    screen      VARCHAR(100),
    path        VARCHAR(500),
    icon        VARCHAR(40),
    permission  VARCHAR(100),
    sort_order  INTEGER DEFAULT 0 NOT NULL
);
```

```rust
#[model]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub label: String,
    pub screen: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub permission: Option<String>,
    pub sort_order: i64,
}

async fn menu(ctx: Ctx) -> Result<Vec<MenuNode>> {
    let db = ctx.service::<Db>().ok_or_else(|| Error::Internal("no database".into()))?;
    let rows: Vec<MenuRow> = db
        .query("SELECT id, parent_id, label, screen, path, icon, permission, sort_order FROM menu_items ORDER BY sort_order, id", vec![])
        .await?;
    Ok(tree(&rows, None))
}

fn tree(rows: &[MenuRow], parent: Option<i64>) -> Vec<MenuNode> {
    rows.iter()
        .filter(|r| r.parent_id == parent)
        .map(|r| {
            let mut node = match (&r.screen, &r.path) {
                (Some(screen), _) => MenuNode::screen(r.label.clone(), screen.clone()),
                (None, Some(path)) => MenuNode::path(r.label.clone(), path.clone()),
                (None, None) => MenuNode::group(r.label.clone(), tree(rows, Some(r.id))),
            };
            if let Some(icon) = Icon::ALL.iter().find(|i| Some(i.name()) == r.icon.as_deref()) {
                node = node.icon(*icon);
            }
            match &r.permission {
                Some(p) => node.permission(p.clone()),
                None => node,
            }
        })
        .collect()
}

App::new("Admin").menu_source(menu)
```

- **कभी भरोसा नहीं।** सर्वर हर अनुरोध पर हर लिंक जाँचता है: अज्ञात screens के लिंक लॉग में चेतावनी के साथ हटा दिए जाते हैं, उन screens के लिंक जिन्हें उपयोगकर्ता नहीं खोल सकता (पहुँच, feature guards, नोड की अपनी `permission`) स्रोत के लौटाने पर भी हटा दिए जाते हैं, और वे screen लिंक भी जिनके पथ के हिस्से params से नहीं भरते। पथ एक ही `/` से शुरू होना चाहिए: कोई स्कीम नहीं, `//` नहीं, बैकस्लैश नहीं और कोई नियंत्रण वर्ण नहीं। समूह अधिकतम `MENU_DEPTH` स्तर तक नेस्ट होते हैं, स्रोत के उत्तर में अधिकतम `MAX_MENU_NODES` (500) नोड रहते हैं, और लेबल `MAX_MENU_LABEL` (100) वर्णों पर काट दिए जाते हैं।
- **कैश किया हुआ।** उत्तर हर टेनेंट, भाषा और अनुमतियों के समूह के लिए `MENU_TTL` (60 सेकंड) तक कैश रहते हैं, इसलिए एक जैसी भूमिकाओं वाले उपयोगकर्ता एक ही उत्तर साझा करते हैं और उनके लिए स्रोत मिनट में एक बार चलता है। कैश अधिकतम `MAX_MENU_CACHE` (256) उत्तर रखता है और सबसे पुराना हटा देता है। जिस स्रोत का उत्तर केवल अनुमतियों पर नहीं बल्कि उपयोगकर्ता पर भी निर्भर हो, उसे `App::menu_per_user()` चाहिए। कोई action तालिका बदलने के बाद इंजेक्ट की गई `Menus` सेवा पर `invalidate()` बुलाता है।
- **पेज कभी नहीं गिरता।** जब स्रोत त्रुटि लौटाता है, पैनिक करता है या `MENU_TIMEOUT` (2 सेकंड) से ज़्यादा समय लेता है, तो पेज स्थिर मेनू दिखाता है और सर्वर विफलता लॉग करता है। विफलताएँ कैश नहीं होतीं।
- **कहाँ दिखता है।** मेनू app shell का हिस्सा है: screens के पूरे पेज लोड और त्रुटि पेज इसे रखते हैं, जबकि साइट शेल और साइन इन पेज केवल स्थिर मेनू दिखाते हैं।

## साइट शेल

कार्य Screen अपने साइड मेनू वाले ऐप शेल में खुलती हैं। सार्वजनिक पृष्ठ, जैसे शॉप या उत्पाद कैटलॉग, इसके बजाय साइट शेल में खुल सकते हैं: ब्रांड लिंक के रूप में ऐप का नाम, क्षैतिज मेनू, संकरी स्क्रीन पर मेनू बटन, वैकल्पिक action और फ़ुटर। feature इसे चुनती है; अन्य feature की Screen ऐप शेल रखती हैं, इसलिए दोनों एक ऐप में रहते हैं।

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **मेनू।** शीर्ष मेनू साइट शेल में हर feature के मेनू सूचीबद्ध करता है। एक आइटम वाला समूह सादा लिंक है; बड़ा समूह एक बटन है जो अपने लिंक खोलता है (W3C APG disclosure navigation पैटर्न)। Enter, Space या क्लिक समूह को टॉगल करते हैं, Down उसे खोलकर पहले लिंक पर जाता है, Up, Down, Home और End उसके लिंक के बीच चलते हैं, Escape उसे बंद करके बटन पर लौटता है, और समूह से Tab बाहर या बाहर क्लिक उसे बंद करता है। Left और Right शीर्ष आइटमों के बीच चलते हैं।
- **संकरी स्क्रीन।** 1024 px से नीचे मेनू `aria-expanded` वाले मेनू बटन के पीछे मुड़ जाता है; Escape उसे बंद करता है। स्क्रिप्ट के बिना मेनू खुला रहता है, इसलिए कुछ भी पहुँच से बाहर नहीं होता।
- **हेडर स्लॉट।** शीर्ष बार में चार स्लॉट हैं जो कोई भी कंपोनेंट लेते हैं, तार्किक नामों वाले ऐप बार की तरह: `start(node)` ब्रांड से पहले, `center(node)` मेनू और अंत के बीच, `end(node)` बार के अंत में और `bottom(node)` उसके नीचे पूरी चौड़ाई की पंक्ति के रूप में, जैसे chips। हर स्लॉट कई नोड क्रम से रखता है, और दाएँ से बाएँ भाषाओं में start और end की दिशाएँ बदल जाती हैं। स्लॉट में कंपोनेंट उपयोगकर्ता की भाषा, अनुमतियों और मास्क के साथ रेंडर होते हैं; जो अपने action से डेटा लोड करते हैं, जैसे `SearchBox`, वहाँ काम करते हैं, feature के गार्ड और action की अनुमति के विरुद्ध जाँचे जाकर। ग्रिड और फ़ॉर्म स्क्रीन में रहने चाहिए। फ़ोन पर start और end स्लॉट बार में रहते हैं, और center स्लॉट वहाँ जाता है जहाँ ऐप कहता है: `center_on_phone(CenterOnPhone::Row)` (डिफ़ॉल्ट) उसे बार के नीचे एक पंक्ति में रखता है, `CenterOnPhone::Icon` एक मैग्नीफ़ायर बटन के पीछे जो उसे खोलता है (W3C APG disclosure) और फ़ोकस उसमें ले जाता है; Escape उसे बंद करके बटन पर लौटता है। `center_button(icon, label)` उस बटन को बदलता है। फ़ोन का लेआउट स्क्रिप्ट चलने से पहले CSS से तय होता है, इसलिए पेज लोड होते समय हेडर हिलता नहीं।
- **Action।** `sign_in()` आगंतुकों के लिए साइन इन लिंक और साइन इन उपयोगकर्ताओं के लिए साइन आउट लिंक दिखाता है जब ऐप में साइन इन पृष्ठ हो; `language()` और `theme()` ऐप शेल जैसे ही चयनकर्ता दिखाते हैं। वे end स्लॉट के बाद आते हैं और फ़ोन पर मेनू में मुड़ जाते हैं।
- **निचला बार।** `bottom_nav([...])` फ़ोन और टैबलेट पर (1024 px से संकरी) स्क्रीन के नीचे जड़े अधिकतम पाँच लिंक जोड़ता है, उन जगहों के लिए जहाँ लोग सबसे ज़्यादा जाते हैं; पाँच से अधिक कंपाइल नहीं होते। हर आइटम एक `BottomNavItem` है जिसमें एक लेबल, रूट वाली एक स्क्रीन और वैकल्पिक आइकन होता है। दिखाए जा रहे पेज के लिंक पर `aria-current="page"` होता है (`nav.js` उसे अद्यतन रखता है), जिन स्क्रीन को उपयोगकर्ता नहीं खोल सकता उनके लिंक छोड़ दिए जाते हैं, बार `env(safe-area-inset-bottom)` से होम इंडिकेटर से दूर रहता है, और पेज के नीचे जगह छोड़ी जाती है ताकि बार कुछ न छिपाए। बाकी के लिए ऊपरी मेनू बना रहता है। बार को स्क्रिप्ट की ज़रूरत नहीं।
- **फ़ुटर।** Screen के लिंक के कॉलम (`link`, उन उपयोगकर्ताओं के लिए हटाए जाते हैं जो उन्हें नहीं खोल सकते) या पते (`url`: एक पथ, `https:`, `http:` या `mailto:`; `App::check` अन्य स्कीम और अपंजीकृत Screen के लिंक अस्वीकार करता है), और टेक्स्ट की एक पंक्ति।

साइट शेल Screen और ऐप शेल Screen के बीच जाना `nav.js` से बिना पृष्ठ लोड के होता है: आंशिक उत्तर बताता है कि वह किस शेल का है (`x-zeyn-shell`), और जब वह मौजूदा फ़्रेम से अलग हो, रनटाइम पूरा फ़्रेम उसी तरह बदल देता है जैसे भाषा बदलने पर, और दूसरे शेल की स्टाइलशीट और स्क्रिप्ट लोड करता है। `/_zeyn/route/{screen}` के लिंक (ब्रेडक्रंब के) उसी तरह अपनाए जाते हैं।

## साइन इन शेल

साइन इन पेज को अपना अलग फ़्रेम मिलता है: `Shell::auth()` में न मेनू है, न ऊपरी पट्टी, न कमांड पैलेट। ऐप का लोगो (`App::logo`) और नाम 400 px चौड़े एक कार्ड के ऊपर रहते हैं, और भाषा और थीम पिकर पेज के नीचे रहते हैं, ताकि साइन इन से पहले भी दोनों पहुँच में रहें। `.split()` 1024 px से फ़ॉर्म को एक आधे हिस्से में और `--z-brand` में एक ब्रांड पैनल को दूसरे आधे में रखता है; संकरी स्क्रीन बीच में रखा कार्ड दिखाती हैं। फ़ोन पर कार्ड ही पेज बन जाता है और उसके इनपुट और बटन 44 px ऊँचे होते हैं। इसकी स्टाइल (`auth.css`, 3.5 KB) केवल इन्हीं पेज पर लोड होती है।

`zeyn::accounts::ui` की हर साइन इन स्क्रीन (साइन इन, साइन अप, पासवर्ड भूलना और रीसेट करना, दो चरण वाला साइन इन, रिकवरी कोड, लॉकआउट, पुष्टि और साइन आउट) इसी में खुलती है, जबकि खाता और प्रशासन स्क्रीन ऐप शेल रखती हैं। कस्टम साइन इन स्क्रीन इसे अपनी feature से पाती है:

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

साइन इन फ़ॉर्म उपयोगकर्ता नाम को `Control::Username` (`autocomplete="username"`) से और पासवर्ड को `current-password` से भरता है, पासवर्ड टॉगल को फ़ील्ड के भीतर एक आँख के रूप में दिखाता है, और Caps Lock चालू रहने पर उसके नीचे "Caps Lock चालू है" कहता है (एक polite live region)।

## स्क्रीन में थीम पिकर

`ThemePicker::new()` थीम पिकर को किसी स्क्रीन के भीतर रखता है, जैसे सेटिंग पेज का Appearance भाग। यह `App::themes` की थीम देता है (ऐप डिवाइस का अनुसरण करे तो System के साथ) और चल रही थीम दिखाता है।

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

यह शेल के पिकर जैसा ही नियंत्रण है: चुनाव बिना पेज लोड किए तुरंत लागू होता है, `zeyn_theme` कुकी में (साइन इन उपयोगकर्ताओं के लिए व्यू स्टोर में भी) रखा जाता है, और पेज का दूसरा पिकर भी उसे दिखाता है। जब ऐप कोई विकल्प नहीं देता (`theme_picker(false)` या एक ही थीम), तो यह कुछ नहीं बनाता।

## ऐप हेडर

`App::header(|h| ...)` ऐप शेल की स्क्रीन के ऊपर एक पट्टी रखता है, जिसमें साइट हेडर वाले स्लॉट होते हैं: `start`, `center`, `end` और `bottom`। बीच में खोज बॉक्स और अंत में सूचनाओं का लिंक आता है। `sign_in()`, `language()` और `theme()` भी यहाँ काम करते हैं, हालाँकि साइड मेनू में पिकर पहले से हैं।

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

तीन तैयार हिस्से पट्टी को ऐप फ़्रेम बनाते हैं:

- `breadcrumb()` पट्टी को मेनू समूह और मौजूदा स्क्रीन के शीर्षक से शुरू करता है; `nav.js` हर नेविगेशन के बाद उसे फिर से लिखता है।
- `search()` एक खोज फ़ील्ड जोड़ता है जो कमांड पैलेट खोलता है, उसका शॉर्टकट दिखाते हुए; पैलेट बटन साइड मेनू से हट जाता है। फ़ोन पर यह एक आइकन बटन है।
- `account()` पट्टी को एक गोल बटन पर उपयोगकर्ता के आद्याक्षरों से ख़त्म करता है। यह एक डायलॉग खोलता है (W3C APG non modal dialog; Escape और बंद करने का बटन उसे बंद करते हैं) जिसमें साइन इन किया व्यक्ति, थीम और भाषा पिकर और साइन आउट लिंक होते हैं, या आगंतुकों के लिए साइन इन लिंक। पिकर साइड मेनू से हट जाते हैं।

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

स्क्रीन स्क्रॉल होने पर पट्टी ऊपर टिकी रहती है, और स्क्रीन बिना पेज लोड किए उसके नीचे बदलती हैं। फ़ोन पर यह मेनू बटन के नीचे रहती है और बीच का स्लॉट अपनी अलग पंक्ति लेता है। पट्टी के घटक स्क्रीन के घटकों की तरह उपयोगकर्ता के लिए बनते हैं; खोज बॉक्स अपने action से सुझाव लाता है, जिसकी अनुमति सर्वर जाँचता है। `App::header` के बिना ऐप शेल में ऊपरी पट्टी नहीं होती।

## कमांड

कमांड Rust में एक बार अपने लेबल, शॉर्टकट और काम के साथ घोषित होता है। feature पर पंजीकृत होने पर पैलेट उसे सूचीबद्ध करता है और उसका शॉर्टकट हर पृष्ठ पर काम करता है।

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| कंस्ट्रक्टर | क्या चलाता है |
|---|---|
| `Command::open(id, label, screen, params)` | Screen को Window के रूप में खोलता है |
| `Command::navigate(id, label, screen, params)` | किसी Screen पर जाता है |
| `Command::effect(id, label, effect)` | ब्राउज़र में एक effect, जैसे टोस्ट |
| `Command::action(id, label, action, input)` | सर्वर पर एक action; इनपुट कभी ब्राउज़र तक नहीं जाता |

`App::check` उन शॉर्टकट को बताता है जो पढ़े नहीं जा सकते, जिन्हें दो कमांड साझा करते हैं, या जिन्हें पैलेट अपने लिए रखता है।

## कमांड पैलेट

**Alt+Shift+P** डिफ़ॉल्ट रूप से पैलेट खोलता है (`App::palette_shortcut` इसे बदलता है, जान-बूझकर `Ctrl+Shift+P` जैसी ब्राउज़र कुंजी में भी): एक ARIA combobox जो ऐप के कमांड, मौजूदा Screen का टूलबार और संदर्भ मेनू और मेनू की Screen में खोजता है। हाल में इस्तेमाल हुए कमांड पहले आते हैं। जब फ़ोकस किसी फ़ील्ड में न हो तो `/` भी इसे खोलता है, और `App::header(|h| h.search())` का खोज फ़ील्ड भी। परिणाम अपनी जगह के नीचे समूहित होते हैं ("जाएँ · बिक्री", स्क्रीन का शीर्षक, "कमांड"), हर एक अपने आइकन, मेल खाए अक्षर बोल्ड में और अंत में अपने शॉर्टकट के साथ। मेनू आइटम पेज पर ही मिलाए जाते हैं, इसलिए पहली कुंजी दबाते ही वे मिल जाते हैं।

feature इसमें सर्वर खोज जोड़ सकती है। उपयोगकर्ता के टाइप करते समय पैलेट action कॉल करता है और उसके परिणाम कमांड के नीचे सूचीबद्ध करता है; एक चुनने पर पृष्ठ उसी जगह खुलता है। यह साइट अपने पृष्ठ इसी तरह खोजती है:

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

सर्वर हर वह खोज चलाता है जो उपयोगकर्ता चला सकता है, feature के गार्ड, action की अनुमति और इंटरसेप्टर के साथ, क्वेरी को 100 अक्षरों पर काटता है, ऐप से बाहर जाने वाले परिणाम हटाता है और अधिकतम 20 लौटाता है।

## मेनू बार, टूलबॉक्स और स्टेटस बार

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar` ARIA menubar पैटर्न का पालन करता है और पैलेट वाले ही कमांड रखता है।
- `Toolbox` टॉगल बटन का ऊर्ध्वाधर समूह है; चुना गया टूल `zeyn:tool` इवेंट के रूप में भेजा जाता है।
- `PropertyInspector::new(title, form)` चुनी गई वस्तु को `.section(...)` समूहों में बँटे फ़ॉर्म से संपादित करता है।
- `PropertyInspector::island(name)` किसी आइलैंड में चुनी गई चीज़ को बिना फ़ॉर्म के संपादित करता है ([Island inspector](#island-inspector) देखें)।
- `StatusBar` में लाइव रीजन है, इसलिए उसे भेजे गए संदेश स्क्रीन रीडर पढ़ते हैं।

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

आइलैंड `detail: { title, schema, values }` के साथ `zeyn:inspect` भेजता है, जहाँ `schema` एक सीरियलाइज़ किया गया `FormSchema` है। फ़ील्ड के प्रकार `text`, `multiline`, `integer`, `decimal`, `length`, `color`, `boolean`, `choice` और `expression` हैं; हर एक `required`, `min`, `max`, `max_len`, `default` और `help` लेता है।

इंस्पेक्टर फ़ील्ड को मॉडल फ़ॉर्म के कंट्रोल से बनाता है और नाम, लेबल और मान HTML के रूप में नहीं, टेक्स्ट के रूप में भरता है। जब फ़ील्ड छोड़ी जाती है, Enter दबाया जाता है या कोई चयनकर्ता बंद होता है, तो वह मान जाँचता है और आइलैंड की जड़ को `detail: { name, value }` के साथ `zeyn:property` भेजता है। अमान्य मान अपनी त्रुटि दिखाता है और नहीं भेजा जाता। 200 से अधिक फ़ील्ड वाला स्कीमा अनदेखा किया जाता है। `FieldSpec::check` सर्वर पर वही जाँच करता है।

## डॉकिंग पैनल

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

उपयोगकर्ता टैब को क्षेत्रों के बीच खींचते हैं, पॉइंटर या कीबोर्ड से क्षेत्रों का आकार बदलते हैं, और पैनल बंद करते हैं; मेनू में `MenuList::panels` उन्हें वापस लाता है। लेआउट प्रति उपयोगकर्ता और Screen ऐप के व्यू स्टोर में सहेजा जाता है, या ब्राउज़र में जब ऐप के पास कोई न हो।
