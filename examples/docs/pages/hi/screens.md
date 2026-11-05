# Screen और params

Screen रूट वाला पृष्ठ है, या दूसरी Screen के ऊपर खुली Window। `#[screen]` अपना एक्सेस उसी तरह घोषित करता है जैसे action।

## एक पृष्ठ

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` पृष्ठ है; `Screen::window(title)` Window है।
- कई `.body(...)` कॉल कई कंपोनेंट को एक के नीचे एक रखती हैं।
- Screen `Screen` या `Result<Screen>` लौटाती है।

## Params

Screen का पहला आर्ग्युमेंट उसके params हैं: `#[params]` struct, या जब कोई न हो तो `_: ()`। अन्य आर्ग्युमेंट action की तरह इंजेक्ट होते हैं।

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

Params हर उस जगह टाइप्ड हैं जहाँ वे इस्तेमाल होते हैं: `Window::open(product_editor, 7)` कंपाइल नहीं होता, क्योंकि एडिटर `ProductParams` लेता है।

बिना `route` वाली Screen का कोई URL नहीं होता। वह लिंक, टूलबार बटन, मेनू या कमांड से Window के रूप में खुलती है।

## पथ में Params

रूट params के फ़ील्ड को पथ खंड के रूप में नाम दे सकता है। हर `{name}` उसी नाम के फ़ील्ड से बँधता है और उस फ़ील्ड के प्रकार के रूप में पढ़ा जाता है:

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` ऑर्डर 42 खोलता है। जो फ़ील्ड पथ में नहीं हैं वे पहले की तरह JSON क्वेरी में जाते हैं: `/orders/42?params={"tab":"lines"}`।
- खंड पूर्णांक, entity कुंजियाँ (`OrderId`), `String` और `Slug` लेते हैं। मान केवल उसी रूप में पार्स होता है जिसमें फ्रेमवर्क उसे लिखता है (`7`, `007` या `+7` नहीं); बाकी कुछ भी 404 का उत्तर देता है।
- ऐसा खंड जो params के किसी फ़ील्ड को नाम नहीं देता, बिना params वाली Screen पर खंडों वाला रूट, या ऐसा फ़ील्ड प्रकार जो खंड नहीं हो सकता, कंपाइल नहीं होता, और त्रुटि बताती है कि क्या बदलना है।
- कई खंड भी इसी तरह काम करते हैं: `slug: Slug` और `n: u32` के साथ `#[screen(route = "/catalog/{slug}/{n}", public)]`।
- फ्रेमवर्क जो लिंक लिखता है वे पथ रूप इस्तेमाल करते हैं: `Effect::navigate`, ब्रेडक्रंब, कमांड और साइन इन के बाद `return_to`। बिना खंड वाली Screen `?params={json}` रखती हैं।
- खंडों वाली Screen मेनू आइटम या होम Screen नहीं हो सकती, क्योंकि उन्हें कुछ भर नहीं सकता।
- एक्सेस हर अनुरोध पर जाँचा जाता है, किसी भी Screen की तरह।

### Slug

`Slug::from_title("Çay Bardağı")` `cay-bardagi` है: छोटे ASCII अक्षर, शब्द `-` से जुड़े, तुर्की अक्षर मैप किए गए (`ı` और `İ` से `i`, `ş` से `s`, `ğ` से `g`, `ç` से `c`, `ö` से `o`, `ü` से `u`), अन्य लैटिन अक्षरों के उच्चारण चिह्न हटाए गए, और बाकी कुछ भी छोड़ा गया। slug अधिकतम 80 बाइट का होता है। पथ में केवल कैनोनिकल रूप पार्स होता है, इसलिए `/catalog/Cay-Bardagi/1` 404 का उत्तर देता है।

### नहीं मिला और स्थानांतरित

जब रिकॉर्ड वहाँ न हो तो `Err(Error::NotFound)` लौटाएँ; उपयोगकर्ता को 404 पृष्ठ मिलता है। जब पता बदल गया हो, उदाहरण के लिए उत्पाद का नाम बदलने के बाद, नए पते से उत्तर दें। ब्राउज़र को स्थायी रीडायरेक्ट (301) मिलता है, और ऐप के भीतर नेविगेशन उसे बिना पृष्ठ लोड के अपनाता है:

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)` एक अनुरोध को दूसरी Screen पर भेज देता है, जैसे यादृच्छिक चयन या सबसे नई प्रविष्टि, जबकि पता खुद वैध रहता है। ब्राउज़र को लक्ष्य के कैनोनिकल पथ और `Cache-Control: no-store` के साथ `303 See Other` मिलता है; ऐप नेविगेशन उसे पृष्ठ लोड किए बिना अपनाता है, और एड्रेस बार लक्ष्य दिखाता है। लक्ष्य हमेशा route वाली टाइप की हुई Screen होती है, इसलिए रीडायरेक्ट कभी ऐप के बाहर के पते तक नहीं ले जा सकता, और जिस लक्ष्य को ऐप सर्व नहीं करता वह 404 देता है।

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## एक्सेस

| विकल्प | अर्थ |
|---|---|
| `permission = "..."` | आइडेंटिटी को यह अनुमति चाहिए |
| `public` | कोई भी। बिना सत्र वाले आगंतुकों को पृष्ठ बिना कुकी के मिलता है (देखें [सार्वजनिक पृष्ठ](public-pages.md)) |
| `cache = 300` | केवल सार्वजनिक Screen: ब्राउज़र और साझा कैश बिना सत्र वाले आगंतुक का पृष्ठ इतने सेकंड (अधिकतम 86400) रख सकते हैं |
| `sensitive` | इसे खोलना व्यक्तिगत डेटा के पठन के रूप में दर्ज होता है |
| `reason` | खुलने से पहले उपयोगकर्ता को कारण देना होगा (देखें [ऑडिट, मास्किंग और जॉब](enterprise.md)) |

पृष्ठ `Screen` पर `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)` और `.robots(..)` से सर्च इंजन और लिंक पूर्वावलोकन को अपने बारे में बताता भी है; देखें [सार्वजनिक पृष्ठ](public-pages.md#search-engines-and-link-previews)।

जो Screen उपयोगकर्ता नहीं खोल सकता उसे खोलने वाले मेनू आइटम, टूलबार बटन और कमांड HTML से हटा दिए जाते हैं।

जो Screen साइन इन न किए आगंतुकों के लिए है वह `public` घोषित करती है। उसकी action को अपना एक्सेस चाहिए: डेटा सूचीबद्ध करने वाली सार्वजनिक Screen `public` action कॉल करती है, वरना आगंतुकों के लिए सूची खाली रहती है।

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

`permission` और `public` दोनों छोड़ना कंपाइल त्रुटि है, इसलिए Screen कभी गलती से खुली नहीं रहती।

## टूलबार और संदर्भ मेनू

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

`Alt+N` जैसे शॉर्टकट Screen पर काम करते हैं और पैलेट में दिखाए जाते हैं। जो शॉर्टकट ब्राउज़र इस्तेमाल करता है (`Ctrl+P`, `Ctrl+S`, `F5`) वे तब तक बाँधे नहीं जाते जब तक बटन `.override_browser()` न कहे, और जिन्हें वह कभी आगे नहीं भेजता (`Ctrl+N`, `Ctrl+T`, `Ctrl+W`) वे कभी नहीं बाँधे जाते; खाली संयोजन `Alt+Shift+<letter>` और `Ctrl+Alt+<letter>` हैं।

## हेडर

`.subtitle(text)` शीर्षक के नीचे एक छोटी पंक्ति रखता है, और `.primary(|t| ...)` स्क्रीन की मुख्य क्रियाओं को प्राथमिक बटनों के रूप में शीर्षक पंक्ति के अंत में रखता है। दोनों मिलकर वह पेज हेडर बनाते हैं जो ज़्यादातर एडमिन स्क्रीन में होता है।

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary` वही बटन लेता है जो `.toolbar` लेता है, वही शॉर्टकट और पैलेट प्रविष्टियाँ भी; बाकी टूल के लिए टूलबार हेडर के नीचे ही रहता है। उपशीर्षक स्क्रीन रीडर के लिए स्क्रीन का विवरण है (`aria-describedby`)। पंक्ति बहुत संकरी हो तो क्रियाएँ शीर्षक के नीचे चली जाती हैं, और फ़ोन पर शुरुआत की ओर संरेखित होती हैं। उपशीर्षक या मुख्य क्रियाओं के बिना स्क्रीन पहले की तरह एक सादे शीर्षक के साथ बनती है।

## Feature और मेनू

Screen एक feature की होती है। feature का रूट प्रीफ़िक्स रूट से पहले आता है, उसके गार्ड Screen खुलने से पहले चलते हैं, और उसका मेनू Screen सूचीबद्ध करता है:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` `/` से एक Screen खुलवाता है।

## त्रुटि पृष्ठ

`App::not_found(screen)` और `App::server_error(screen)` ऐप को 404 और सर्वर त्रुटियों के लिए अपने पृष्ठ देते हैं:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- 404 Screen अज्ञात पथों और हर उस Screen का उत्तर देती है जो `Err(Error::NotFound)` लौटाती है, उन पथ मानों समेत जो पार्स नहीं होते।
- सर्वर त्रुटि Screen `Error::Internal`, टाइमआउट (504) और हैंडलर में panic का उत्तर देती है। त्रुटि का विवरण केवल सर्वर लॉग में जाता है; Screen को कोई params नहीं मिलते और वह अनुरोध के बारे में कुछ नहीं जानती।
- दोनों शेल के भीतर सही स्टेटस कोड के साथ दिखाई जाती हैं। ऐप के भीतर नेविगेशन केवल Screen पाता है और उसे बिना पृष्ठ लोड के बदल देता है। फ्रेमवर्क एंडपॉइंट और API JSON में उत्तर देते रहते हैं।
- त्रुटि Screen कोई params नहीं लेतीं, उन्हें रूट नहीं चाहिए और वे किसी feature का हिस्सा नहीं हैं। उन्हें `public` बनाएँ: जो उपयोगकर्ता एक को नहीं खोल सकता, या जो त्रुटि Screen खुद विफल हो जाए, उसे छोटा बिल्ट इन संदेश मिलता है।

## Screen ब्राउज़र तक कैसे पहुँचती है

पहला अनुरोध पूरा पृष्ठ पाता है: मेनू, Screen और एसेट। फिर ऐप के भीतर के लिंक केवल Screen लोड करते हैं और उसे उसी जगह बदल देते हैं, इसलिए मेनू, खुली स्थिति और स्क्रॉल स्थिति बनी रहती है। बैक बटन, नए टैब और बुकमार्क सामान्य लिंक की तरह काम करते हैं।
