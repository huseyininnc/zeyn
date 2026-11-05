# الغلاف والأوامر

الغلاف (shell) هو الإطار المحيط بكل شاشة: القائمة ومحددات اللغة والسمة والمنطقة الزمنية والإشعارات المنبثقة (toasts) ولوحة الأوامر. التطبيقات التي تعمل كأدوات سطح المكتب تضيف شريط قوائم وصندوق أدوات ومفتش خصائص وشريط حالة ولوحات قابلة للإرساء.

## القائمة

تأتي القائمة من استدعاءات `.menu(...)` في الميزات. العناصر التي لا يحق للمستخدم فتحها تُستبعد من HTML، والميزة التي ترفض حراسها المستخدم لا تعرض شيئاً. يحمل رابط الشاشة الحالية `aria-current="page"`.

## غلاف الموقع

تُفتح شاشات العمل في غلاف التطبيق بقائمته الجانبية. أما الصفحات العامة، مثل متجر أو كتالوج منتجات، فيمكنها أن تُفتح في غلاف الموقع بدلاً من ذلك: اسم التطبيق كرابط علامة تجارية، وقائمة أفقية، وزر قائمة على الشاشات الضيقة، وإجراءات اختيارية وتذييل. تختاره الميزة؛ وتحتفظ شاشات الميزات الأخرى بغلاف التطبيق، فيعيشان معاً في تطبيق واحد.

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

- **القائمة.** تسرد القائمة العلوية قوائم كل ميزة في غلاف الموقع. المجموعة ذات العنصر الواحد رابط عادي؛ والمجموعة الأكبر زر يفتح روابطها (نمط W3C APG للتنقل بالإفصاح). يبدّل Enter أو Space أو النقر حالة المجموعة، ويفتحها السهم لأسفل وينتقل إلى أول رابط فيها، وتنقل مفاتيح السهم لأعلى ولأسفل وHome وEnd بين روابطها، ويغلقها Escape ويعيد إلى الزر، ويغلقها Tab خارج المجموعة أو النقر خارجها. ينقل السهمان لليسار واليمين بين العناصر العلوية.
- **الشاشات الضيقة.** تحت 800 بكسل تنطوي القائمة خلف زر قائمة مع `aria-expanded`؛ ويغلقها Escape. وبدون السكربت تبقى القائمة مفتوحة، فلا يصبح شيء بعيد المنال.
- **خانات الرأس.** للشريط العلوي أربع خانات تقبل أي مكوّن، مثل شريط تطبيق بأسماء منطقية: `start(node)` قبل العلامة التجارية، و`center(node)` بين القائمة والنهاية، و`end(node)` في نهاية الشريط، و`bottom(node)` صفًّا بكامل العرض تحته، مثل chips. تحمل كل خانة عدة عقد بالترتيب، ويتبادل start وend الجانبين في اللغات التي تُكتب من اليمين إلى اليسار. تُعرض المكوّنات في الخانات بلغة المستخدم وصلاحياته وأقنعته؛ وتلك التي تحمّل بيانات عبر إجرائها الخاص، مثل `SearchBox`، تعمل هناك، مع فحصها مقابل حراس الميزة وصلاحية الإجراء. أما الشبكات والنماذج فمكانها الشاشة. على الهواتف تبقى خانتا start وend في الشريط، وتذهب خانة center إلى حيث يقول التطبيق: `center_on_phone(CenterOnPhone::Row)` (الافتراضي) يضعها في صف تحت الشريط، و`CenterOnPhone::Icon` خلف زر عدسة مكبّرة يفتحها (إفصاح W3C APG) وينقل التركيز إليها؛ ويغلقها Escape ويعيد إلى الزر. ويغيّر `center_button(icon, label)` ذلك الزر. يضبط CSS تخطيط الهاتف قبل أن يعمل السكربت، فلا يتحرك الرأس أثناء تحميل الصفحة.
- **الإجراءات.** يعرض `sign_in()` رابط تسجيل دخول للزوار ورابط تسجيل خروج للمستخدمين الذين سجلوا الدخول عندما يكون للتطبيق صفحة تسجيل دخول؛ ويعرض `language()` و`theme()` المحددات نفسها الموجودة في غلاف التطبيق. وتأتي بعد خانة end وتنطوي داخل القائمة على الهواتف.
- **الشريط السفلي.** يضيف `bottom_nav([...])` حتى خمسة روابط مثبتة في أسفل الشاشة على الهواتف (بعرض 800 px وأقل) للأماكن التي يقصدها الناس أكثر؛ وأكثر من خمسة لا يُترجم. كل عنصر هو `BottomNavItem` له تسمية وشاشة لها مسار وأيقونة اختيارية. يحمل رابط الصفحة المعروضة `aria-current="page"` (يُبقيه `nav.js` محدّثًا)، وتُحذف الروابط إلى الشاشات التي لا يجوز للمستخدم فتحها، ويبتعد الشريط عن مؤشر الشاشة الرئيسية بـ `env(safe-area-inset-bottom)`، وتُترك مساحة أسفل الصفحة حتى لا يخفي الشريط شيئًا. وتبقى القائمة العليا لبقية الأماكن. ولا يحتاج الشريط إلى سكربت.
- **التذييل.** أعمدة من روابط إلى شاشات (`link`، وتُستبعد للمستخدمين الذين لا يحق لهم فتحها) أو إلى عناوين (`url`: مسار أو `https:` أو `http:` أو `mailto:`؛ يرفض `App::check` المخططات الأخرى والروابط إلى شاشات غير مسجلة)، وسطر نص.

يتم الانتقال بين شاشة من غلاف الموقع وشاشة من غلاف التطبيق عبر `nav.js` دون تحميل صفحة: تذكر الاستجابة الجزئية الغلاف الذي تنتمي إليه (`x-zeyn-shell`)، وعندما يختلف عن الإطار الحالي يستبدل وقت التشغيل الإطار كله كما يفعل تغيير اللغة، ويحمّل ورقة أنماط الغلاف الآخر وسكربته. وتُتبع الروابط إلى `/_zeyn/route/{screen}` (روابط مسار التنقل breadcrumb) بالطريقة نفسها.

## الأوامر

يُعلَن الأمر مرة واحدة في Rust بتسميته واختصاره وما يفعله. وعند تسجيله على ميزة تسرده اللوحة ويعمل اختصاره في كل صفحة.

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

| الدالة المنشئة | تشغّل |
|---|---|
| `Command::open(id, label, screen, params)` | تفتح شاشة كنافذة |
| `Command::navigate(id, label, screen, params)` | تنتقل إلى شاشة |
| `Command::effect(id, label, effect)` | تأثيراً في المتصفح، مثل إشعار منبثق |
| `Command::action(id, label, action, input)` | إجراءً على الخادم؛ ولا يصل المُدخل إلى المتصفح أبداً |

يبلّغ `App::check` عن الاختصارات التي لا يمكن قراءتها، أو التي يشترك فيها أمران، أو التي تحتفظ بها اللوحة لنفسها.

## لوحة الأوامر

يفتح **Alt+Shift+P** اللوحة افتراضياً (يغيّره `App::palette_shortcut`، وكذلك إلى مفتاح متصفح مثل `Ctrl+Shift+P` عن قصد): وهي ARIA combobox تبحث في أوامر التطبيق وشريط الأدوات والقائمة السياقية للشاشة الحالية وشاشات القائمة. تأتي الأوامر المستخدمة حديثاً أولاً.

يمكن لميزة أن تضيف إليها بحثاً على الخادم. أثناء كتابة المستخدم تستدعي اللوحة الإجراء وتسرد نتائجه تحت الأوامر؛ واختيار إحداها يفتح الصفحة في مكانها. يبحث هذا الموقع في صفحاته بهذه الطريقة:

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

يشغّل الخادم كل بحث يحق للمستخدم تشغيله بحراس الميزة وصلاحية الإجراء والمعترضات، ويقص الاستعلام عند 100 حرف، ويسقط النتائج التي تخرج من التطبيق، ويعيد 20 نتيجة كحد أقصى.

## شريط القوائم وصندوق الأدوات وشريط الحالة

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

- يتبع `MenuBar` نمط ARIA menubar ويضم الأوامر نفسها الموجودة في اللوحة.
- `Toolbox` مجموعة رأسية من أزرار التبديل؛ وتُرسل الأداة المختارة كحدث `zeyn:tool`.
- يحرّر `PropertyInspector::new(title, form)` الكائن المحدد بنموذج مقسم إلى مجموعات `.section(...)`.
- لدى `StatusBar` منطقة حية (live region)، لذلك تقرأ قارئات الشاشة الرسائل المرسلة إليه.

## اللوحات القابلة للإرساء

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

يسحب المستخدمون علامات التبويب بين المناطق، ويغيّرون حجم المناطق بالمؤشر أو بلوحة المفاتيح، ويغلقون اللوحات؛ و`MenuList::panels` في قائمة يعيدها. يُحفظ التخطيط لكل مستخدم وشاشة في مخزن العروض الخاص بالتطبيق، أو في المتصفح عندما لا يكون لدى التطبيق مخزن.
