# الشاشات والمعاملات (Screens and params)

الشاشة (screen) صفحة لها مسار (route)، أو نافذة تُفتح فوق شاشة أخرى. يعلن `#[screen]` وصوله كما يفعل الـ action.

## صفحة

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` صفحة؛ و`Screen::window(title)` نافذة.
- عدة استدعاءات لـ `.body(...)` تضع عدة مكونات واحداً تحت الآخر.
- تعيد الشاشة `Screen` أو `Result<Screen>`.

## المعاملات (Params)

الوسيط الأول للشاشة هو معاملاتها: بنية `#[params]`، أو `_: ()` عندما لا تكون لها معاملات. وتُحقن الوسائط الأخرى، كما في الـ actions.

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

المعاملات مكتوبة الأنواع في كل مكان تُستخدم فيه: `Window::open(product_editor, 7)` لا يُصرَّف، لأن المحرر يأخذ `ProductParams`.

عندما تكون كل حقول بنية `#[params]` من النوع `Option`، تُفتح الشاشة أيضًا دون معاملات، من رابط في القائمة أو من عنوان عادي، وتكون كل الحقول `None`. أما الحقل غير الاختياري فيبقى بحاجة إلى قيمته: ومن دونها تجيب الشاشة بـ 404.

الشاشة بدون `route` ليس لها عنوان URL. تُفتح كنافذة من رابط أو زر في شريط الأدوات أو قائمة أو أمر.

## المعاملات في المسار

يمكن للمسار أن يسمّي حقول المعاملات كمقاطع مسار. يربط كل `{name}` الحقل الذي يحمل الاسم نفسه ويُقرأ بنوع ذلك الحقل:

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

- يفتح `/orders/42` الطلب 42. والحقول غير الموجودة في المسار تنتقل كما كان في استعلام JSON: `/orders/42?params={"tab":"lines"}`.
- تقبل المقاطع الأعداد الصحيحة ومفاتيح الكيانات (`OrderId`) و `String` و `Slug`. لا تُحلَّل القيمة إلا بالصيغة التي يكتبها الإطار (`7`، لا `007` ولا `+7`)؛ وأي شيء آخر يُجاب بـ 404.
- المقطع الذي لا يسمّي أي حقل في المعاملات، أو مسار فيه مقاطع على شاشة بلا معاملات، أو نوع حقل لا يمكن أن يكون مقطعاً، لا يُصرَّف، وتقول رسالة الخطأ ما يجب تغييره.
- تعمل عدة مقاطع بالطريقة نفسها: `#[screen(route = "/catalog/{slug}/{n}", public)]` مع `slug: Slug` و `n: u32`.
- الروابط التي يكتبها الإطار تستخدم صيغة المسار: `Effect::navigate` ومسارات التنقل (breadcrumbs) والأوامر و `return_to` بعد تسجيل الدخول. أما الشاشات بلا مقاطع فتبقي `?params={json}`.
- لا يمكن أن تكون الشاشة ذات المقاطع عنصر قائمة أو الشاشة الرئيسية، لأن لا شيء يستطيع ملأها.
- يُفحص الوصول في كل طلب، كما في أي شاشة.

### Slugs

`Slug::from_title("Çay Bardağı")` هي `cay-bardagi`: ASCII بأحرف صغيرة، والكلمات موصولة بـ `-`، والحروف التركية مُحوَّلة (`ı` و `İ` إلى `i`، و `ş` إلى `s`، و `ğ` إلى `g`، و `ç` إلى `c`، و `ö` إلى `o`، و `ü` إلى `u`)، وعلامات الحروف اللاتينية الأخرى تُحذف، وأي شيء آخر يُترك خارجاً. الـ slug بحد أقصى 80 بايتاً. وفي المسار لا تُحلَّل إلا الصيغة القانونية، فيُجاب `/catalog/Cay-Bardagi/1` بـ 404.

### غير موجود ومنقول

أعد `Err(Error::NotFound)` عندما لا يكون السجل موجوداً؛ فيحصل المستخدم على صفحة 404. وعندما يتغير العنوان، مثلاً بعد إعادة تسمية منتج، أجب بالعنوان الجديد. يحصل المتصفح على إعادة توجيه دائمة (301)، ويتبعها التنقل داخل التطبيق دون تحميل صفحة:

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

تحوّل `Screen::redirect(screen, params)` طلبًا واحدًا إلى شاشة أخرى، مثل اختيار عشوائي أو أحدث عنصر، بينما يبقى العنوان نفسه صالحًا. يحصل المتصفح على `303 See Other` مع المسار القانوني للهدف و`Cache-Control: no-store`؛ ويتبعه التنقل داخل التطبيق دون تحميل الصفحة، ويعرض شريط العنوان الهدف. الهدف دائمًا شاشة مكتوبة لها route، فلا يمكن أن يقود التحويل إلى عنوان خارج التطبيق، والهدف الذي لا يخدمه التطبيق يجيب بـ 404.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## الوصول (Access)

| الخيار | المعنى |
|---|---|
| `permission = "..."` | تحتاج الهوية إلى هذه الصلاحية |
| `public` | أي شخص. الزوار بلا جلسة يحصلون على الصفحة دون ملفات تعريف ارتباط (انظر [الصفحات العامة](public-pages.md)) |
| `cache = 300` | للشاشات العامة فقط: يجوز للمتصفحات وذاكرات التخزين المؤقت المشتركة الاحتفاظ بصفحة زائر بلا جلسة لهذا العدد من الثواني (86400 على الأكثر) |
| `sensitive` | يُسجَّل فتحها كقراءة لبيانات شخصية |
| `reason` | يجب أن يذكر المستخدم سبباً قبل أن تُفتح (انظر [التدقيق والإخفاء والمهام](enterprise.md)) |

تخبر الصفحة أيضاً محركات البحث ومعاينات الروابط عن نفسها بـ `.description(..)` و `.canonical(..)` و `.og_image(..)` و `.json_ld(..)` و `.robots(..)` على `Screen`؛ انظر [الصفحات العامة](public-pages.md#search-engines-and-link-previews).

عناصر القوائم وأزرار شريط الأدوات والأوامر التي تفتح شاشة لا يحق للمستخدم فتحها تُحذف من HTML.

الشاشة المخصصة للزوار غير المسجلين تعلن `public`. ولـ actions الخاصة بها وصولها الخاص: الشاشة العامة التي تعرض بيانات تستدعي action من نوع `public`، وإلا بقيت القائمة فارغة للزوار.

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

حذف `permission` و `public` معاً خطأ تصريف، فلا تُترك شاشة مفتوحة بالصدفة أبداً.

## شريط الأدوات وقائمة السياق

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

تعمل الاختصارات مثل `Alt+N` على الشاشة، وتظهر في اللوحة (palette) وفي تلميح الزر (مع `.tip(text)` إن كان له واحد)، لا في تسميته. أما الاختصارات التي يستخدمها المتصفح (`Ctrl+P` و `Ctrl+S` و `F5`) فلا تُربط ما لم يستدعِ الزر `.override_browser()`، وتلك التي لا يمررها أبداً (`Ctrl+N` و `Ctrl+T` و `Ctrl+W`) لا تُربط أبداً؛ والتركيبات الحرة هي `Alt+Shift+<letter>` و `Ctrl+Alt+<letter>`.

تأتي أيقونات الأزرار ومجموعات القائمة والأدوات من `Icon`، وهي مجموعة من 82 رسماً يسردها `Icon::ALL` (`Plus` و`Edit` و`Trash` و`Search` و`Filter` و`Download` و`ChartBar` و`Users` و`Settings` و`Bell` و`SignOut` وغيرها؛ ويعرضها كتالوج المكونات كلها). وهي أقنعة في `/_zeyn/icons.css` مرسومة على شبكة 20 px بخط مستدير بعرض 1.5 px، وتأخذ لون النص، ومخفية عن قارئات الشاشة، لذلك تحمل التسمية المعنى دائمًا.

ارتفاع الأزرار 36 px (و32 px في الكثافة المضغوطة) بإطار هادئ؛ وتحمل الأزرار الأساسية لون العلامة. يزيل `.ghost()` بعد `.button(..)` الإطار للإجراءات الثانوية في الأشرطة المزدحمة، ويرسم `.danger()` بلون الخطر إجراءً يحذف أو يلغي.

## الترويسة

يضع `.subtitle(text)` سطرًا قصيرًا تحت العنوان، ويضع `.primary(|t| ...)` الإجراءات الرئيسية للشاشة في نهاية سطر العنوان كأزرار أساسية. ويكوّنان معًا ترويسة الصفحة الموجودة في معظم شاشات الإدارة.

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

يأخذ `.primary` الأزرار نفسها التي يأخذها `.toolbar`، بالاختصارات وعناصر لوحة الأوامر نفسها؛ ويبقى شريط الأدوات تحت الترويسة لبقية الأدوات. واستدعاءان أو أكثر لـ `.export(..)` في شريط واحد يُجمعان في زر قائمة واحد "Export" فيه عنصر لكل صيغة. العنوان الفرعي هو وصف الشاشة لقارئات الشاشة (`aria-describedby`). إذا ضاق السطر انتقلت الإجراءات تحت العنوان، وعلى الهاتف تصطف عند البداية. ومن دون عنوان فرعي أو إجراءات رئيسية تُعرض الشاشة كما كانت، بعنوان عادي.

## الميزات والقائمة

تنتمي الشاشة إلى ميزة (feature). تأتي بادئة مسار الميزة قبل المسار، وتعمل حراسها قبل أن تُفتح الشاشة، وتسرد قائمتها الشاشات:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

يجعل `App::home(screen)` المسار `/` يفتح شاشة.

## صفحات الخطأ

يعطي `App::not_found(screen)` و `App::server_error(screen)` التطبيق صفحاته الخاصة لـ 404 ولأخطاء الخادم:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- تجيب شاشة 404 عن المسارات المجهولة وعن كل شاشة تعيد `Err(Error::NotFound)`، بما في ذلك قيم المسار التي لا تُحلَّل.
- تجيب شاشة خطأ الخادم عن `Error::Internal` ومهل الانتظار (504) والـ panics في المعالجات. تذهب تفاصيل الخطأ إلى سجل الخادم فقط؛ ولا تحصل الشاشة على معاملات ولا تعرف شيئاً عن الطلب.
- تُعرض كلتاهما داخل الغلاف (shell) مع رمز الحالة الصحيح. التنقل داخل التطبيق يحصل على الشاشة فقط ويبدّلها دون تحميل صفحة. وتواصل نقاط نهاية الإطار وواجهة API الإجابة بـ JSON.
- لا تأخذ شاشات الخطأ معاملات ولا تحتاج مساراً وليست جزءاً من ميزة. اجعلها `public`: المستخدم الذي لا يحق له فتح واحدة، أو شاشة خطأ تفشل هي نفسها، يحصل على الرسالة المدمجة القصيرة.

## كيف تصل الشاشة إلى المتصفح

يحصل الطلب الأول على الصفحة كاملة: القائمة والشاشة والأصول. ثم تحمّل الروابط داخل التطبيق الشاشة فقط وتبدّلها في مكانها، فتبقى القائمة والحالة المفتوحة وموضع التمرير. ويعمل زر الرجوع والتبويبات الجديدة والإشارات المرجعية كما مع الروابط العادية.
