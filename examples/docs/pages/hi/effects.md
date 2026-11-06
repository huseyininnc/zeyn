# Effect और Window

Effect वे हैं जो ब्राउज़र क्लिक या सफल action के बाद करता है: Window खोलना, किसी Screen पर जाना, टोस्ट दिखाना, पुष्टि माँगना, या Window बंद करके उसके नीचे की Screen रिफ़्रेश करना। इन्हें Rust में घोषित किया जाता है और रनटाइम चलाता है, इसलिए कोई JavaScript नहीं लिखा जाता।

## Effect

| Effect | कैसे बनता है | परिणाम |
|---|---|---|
| Window खोलना | `Window::open(screen, params)` | Screen को मोडल Window में खोलता है |
| नेविगेट | `Effect::navigate(screen, params)` | लिंक की तरह Screen पर उसी जगह जाता है |
| टोस्ट | `Effect::Toast { message }` | छोटा संदेश दिखाता है |
| टोन वाला टोस्ट | `Toast::success(message).action(label, effect).into()` | टोन (info, success, warning, danger) में टोस्ट, और एक वैकल्पिक बटन जो दूसरा effect चलाता है |
| Confirm | `Effect::confirm(message, then)` | उपयोगकर्ता के पुष्टि करने के बाद `then` चलाता है |
| बंद करें और रिफ़्रेश | `Effect::CloseAndRefresh` | Window बंद करता है और जिस Screen ने उसे खोला उसे रीलोड करता है |
| Topic रिफ़्रेश करें | `Effect::refresh("orders")` या `ui.refresh(["orders", "stock"])` | पेज के उन node को उसी जगह रिफ़्रेश करता है जो किसी topic को सुनते हैं ([Topic](#topics) देखें) |

Params टाइप्ड हैं: गलत params वाली Screen का effect कंपाइल नहीं होता।

ऐसी Screen पर `Effect::navigate` जिसके रूट में पथ खंड हैं (देखें [पथ में Params](screens.md)) पथ रूप पर पहुँचता है, जैसे `/orders/42`; जो params पथ में नहीं हैं वे `params` क्वेरी में जाते हैं।

## action के बाद

फ़ॉर्म और action बटन `on_success` हैंडलर लेते हैं। यह action का परिणाम और effect इकट्ठा करने वाला `UiCtx` पाता है:

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx` में `toast`, `notify`, `close_and_refresh`, `navigate` और बाकी किसी भी चीज़ के लिए `effect` है, जैसे पुष्टि:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## Window

Window बिना रूट वाली Screen है, जो `Screen::window(title)` से बनती है। Window ढेर में लगती हैं: एक Window दूसरी खोल सकती है। Escape या बंद करने का बटन सबसे ऊपरी Window बंद करता है और फ़ोकस उस एलिमेंट पर लौटता है जिसने उसे खोला। `.width(px)` उसकी चौड़ाई तय करता है।

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## Effect कहाँ से आते हैं

- ग्रिड लिंक और पंक्ति खोलना: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`।
- टूलबार बटन: `.opens(screen, params)`।
- संदर्भ मेनू: `.item(label, effect)`।
- कमांड: `Command::open`, `Command::navigate`, `Command::effect`।
- फ़ॉर्म और action बटन के `on_success` हैंडलर।

Effect कभी पृष्ठ रीलोड नहीं करते। नेविगेशन केवल नई Screen लोड करता है; रिफ़्रेश केवल Window के नीचे की Screen रीलोड करता है।

## Topic

जब Screen का एक हिस्सा बदलता है, तो दूसरे हिस्से अक्सर वही डेटा दिखाते हैं: सहेजा गया ऑर्डर कुल बदल देता है, फ़िल्टर ग्रिड और चार्ट बदल देता है। Topic इन्हें जोड़ता है, और डेटा ब्राउज़र से होकर नहीं जाता। डेटा दिखाने वाले node को `.listens(topic)` से चिह्नित करें; topic का नाम लिए जाने पर वे उसी जगह रिफ़्रेश होते हैं।

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

Topic का नाम तीन तरीकों से लिया जाता है:

- किसी action के डेटा बदलने के बाद, `on_success` हैंडलर में `ui.refresh("sales")` (या `Effect::refresh`)।
- एक फ़िल्टर कंट्रोल जो `.drives("sales")` करता है: फ़िल्टर चिप, सर्च बॉक्स, या बिना सबमिट बटन वाला फ़ॉर्म (सेलेक्ट, तारीख की सीमा, टेक्स्ट)। [फ़िल्टर फ़ॉर्म](forms.md#filter-forms) देखें।
- जब किसी और ने डेटा बदला हो, तब realtime चैनल (feature `realtime`) से `hub.notify(&ctx, "sales")`। `docs/realtime.md` देखें।

`.listens` हर component पर काम करता है: इसे component के अपने builder method के बाद, सबसे आख़िर में कॉल करें। ग्रिड वाला listener अपने ग्रिड को ग्रिड endpoint से फिर से लोड करता है और पेज, क्रम और फ़िल्टर बनाए रखता है (फ़िल्टर से हुआ बदलाव पहले पेज से शुरू होता है)। बाक़ी हर listener सर्वर पर फिर से render होता है, सबके लिए एक ही `POST /_zeyn/nodes` से, इसलिए एक KPI और एक चार्ट का ख़र्च एक अनुरोध है। सिर्फ़ topic के नाम भेजे जाते हैं; सर्वर हर अनुरोध की तरह उपयोगकर्ता की पहुँच और params के साथ Screen खोलता है, इसलिए रिफ़्रेश कभी वह नहीं दिखाता जो उपयोगकर्ता नहीं देख सकता।

एक animation frame के भीतर नाम लिए गए topic एक बार संभाले जाते हैं, उसी Screen पर नया रिफ़्रेश अभी चल रहे रिफ़्रेश को रद्द कर देता है, और सिर्फ़ वही node रिफ़्रेश होते हैं जो पेज पर हैं। नई सामग्री में वही तत्व हो (id, test id या सूची की पंक्ति कुंजी से), तो फ़ोकस और खुले `<details>` अपनी जगह रहते हैं।

Topic के नाम 1 से 64 अक्षर, अंक, `-`, `_` या `.` होते हैं। `App::check_page` ऐसा topic बताता है जिसे किसी पेज पर drive किया गया हो पर उस पेज पर कोई उसे न सुनता हो, और `zeyn check` ऐसे topic के बारे में चेतावनी देता है जो रिफ़्रेश या drive होता है पर कभी सुना नहीं जाता, या सुना जाता है पर कभी रिफ़्रेश, drive या notify नहीं होता।

@live topics

## टोस्ट

`ui.toast(text)` info टोस्ट दिखाता है। `ui.notify(Toast::success(text))` टोन चुनता है: `Toast::info`, `success`, `warning` या `danger`। `.action(label, effect)` एक बटन जोड़ता है जो दूसरा effect चलाता है, जैसे सहेजा गया रिकॉर्ड खोलना:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

टोस्ट चार सेकंड रहते हैं (danger वाले आठ), और पॉइंटर टोस्ट पर रहने या उसके फ़ोकस में होने पर समय रुक जाता है, ताकि उसके बटन तक पहुँचा जा सके। Info, success और warning टोस्ट status रीजन द्वारा विनम्रता से पढ़े जाते हैं; danger टोस्ट alert हैं, तुरंत घोषित होते हैं। हर टोस्ट में बंद करने का बटन है, टोन उसकी रंग पट्टी और टेक्स्ट से भी दिया जाता है, और एक समय में अधिकतम तीन टोस्ट दिखते हैं (`MAX_TOASTS`): नया सबसे पुराने को हटा देता है।

@live toasts
