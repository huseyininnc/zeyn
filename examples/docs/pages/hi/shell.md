# शेल और कमांड

शेल हर Screen के चारों ओर का फ़्रेम है: मेनू, भाषा, थीम और समय क्षेत्र चयनकर्ता, टोस्ट और कमांड पैलेट। जो ऐप डेस्कटॉप टूल की तरह काम करते हैं वे मेनू बार, टूलबॉक्स, प्रॉपर्टी इंस्पेक्टर, स्टेटस बार और डॉकिंग पैनल जोड़ते हैं।

## मेनू

मेनू feature के `.menu(...)` कॉल से आता है। जो आइटम उपयोगकर्ता नहीं खोल सकता वे HTML से हटा दिए जाते हैं, और जिस feature के गार्ड उपयोगकर्ता को अस्वीकार करें वह कुछ नहीं दिखाती। मौजूदा Screen का लिंक `aria-current="page"` रखता है।

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
    .shell(Shell::site().header(|h| h.sign_in().language().theme()).footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    }))
```

- **मेनू।** शीर्ष मेनू साइट शेल में हर feature के मेनू सूचीबद्ध करता है। एक आइटम वाला समूह सादा लिंक है; बड़ा समूह एक बटन है जो अपने लिंक खोलता है (W3C APG disclosure navigation पैटर्न)। Enter, Space या क्लिक समूह को टॉगल करते हैं, Down उसे खोलकर पहले लिंक पर जाता है, Up, Down, Home और End उसके लिंक के बीच चलते हैं, Escape उसे बंद करके बटन पर लौटता है, और समूह से Tab बाहर या बाहर क्लिक उसे बंद करता है। Left और Right शीर्ष आइटमों के बीच चलते हैं।
- **संकरी स्क्रीन।** 800 px से नीचे मेनू `aria-expanded` वाले मेनू बटन के पीछे मुड़ जाता है; Escape उसे बंद करता है। स्क्रिप्ट के बिना मेनू खुला रहता है, इसलिए कुछ भी पहुँच से बाहर नहीं होता।
- **Action।** `sign_in()` आगंतुकों के लिए साइन इन लिंक और साइन इन उपयोगकर्ताओं के लिए साइन आउट लिंक दिखाता है जब ऐप में साइन इन पृष्ठ हो; `language()` और `theme()` ऐप शेल जैसे ही चयनकर्ता दिखाते हैं।
- **फ़ुटर।** Screen के लिंक के कॉलम (`link`, उन उपयोगकर्ताओं के लिए हटाए जाते हैं जो उन्हें नहीं खोल सकते) या पते (`url`: एक पथ, `https:`, `http:` या `mailto:`; `App::check` अन्य स्कीम और अपंजीकृत Screen के लिंक अस्वीकार करता है), और टेक्स्ट की एक पंक्ति।

साइट शेल Screen और ऐप शेल Screen के बीच जाना `nav.js` से बिना पृष्ठ लोड के होता है: आंशिक उत्तर बताता है कि वह किस शेल का है (`x-zeyn-shell`), और जब वह मौजूदा फ़्रेम से अलग हो, रनटाइम पूरा फ़्रेम उसी तरह बदल देता है जैसे भाषा बदलने पर, और दूसरे शेल की स्टाइलशीट और स्क्रिप्ट लोड करता है। `/_zeyn/route/{screen}` के लिंक (ब्रेडक्रंब के) उसी तरह अपनाए जाते हैं।

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

**Alt+Shift+P** डिफ़ॉल्ट रूप से पैलेट खोलता है (`App::palette_shortcut` इसे बदलता है, जान-बूझकर `Ctrl+Shift+P` जैसी ब्राउज़र कुंजी में भी): एक ARIA combobox जो ऐप के कमांड, मौजूदा Screen का टूलबार और संदर्भ मेनू और मेनू की Screen में खोजता है। हाल में इस्तेमाल हुए कमांड पहले आते हैं।

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
- `StatusBar` में लाइव रीजन है, इसलिए उसे भेजे गए संदेश स्क्रीन रीडर पढ़ते हैं।

## डॉकिंग पैनल

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

उपयोगकर्ता टैब को क्षेत्रों के बीच खींचते हैं, पॉइंटर या कीबोर्ड से क्षेत्रों का आकार बदलते हैं, और पैनल बंद करते हैं; मेनू में `MenuList::panels` उन्हें वापस लाता है। लेआउट प्रति उपयोगकर्ता और Screen ऐप के व्यू स्टोर में सहेजा जाता है, या ब्राउज़र में जब ऐप के पास कोई न हो।
