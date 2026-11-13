# DataGrid

تعرض `DataGrid` صفوفًا من إجراء يأخذ `DataQuery` ويعيد `Page<E>`. يجري الترقيم والفرز والمرشحات والمجاميع على الخادم، ولذلك فإن شبكة على مليون صف تكلّف المتصفح ما تكلّفه شبكة على مئة صف.

## شبكة

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

افرز بالنقر على ترويسة، واكتب في خانة مرشح، واسحب ترويسة لنقل عمود أو حافتها لتغيير حجمه. تشمل المجاميع كل الصفوف المصفّاة، لا الصفحة وحدها.

## الإجراء

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

تقوم `DataQuery::apply` بالترشيح والفرز والتجميع والترقيم على `Vec` في الذاكرة، وهذا يناسب القوائم الصغيرة والاختبارات. أما المخزن المدعوم بقاعدة بيانات فيقرأ `DataQuery` نفسها (الصفحة والفرز والمرشحات والمجاميع) ويجيب بـ SQL؛ راجع [البيانات وقواعد البيانات](database.md).

## الترقيم

| الترقيم | الاستخدام |
|---|---|
| `Paging::pages(n)` | صفحات مرقمة، 50 صفًا ما لم يُضبط غير ذلك |
| `Paging::infinite(n)` | يحمّل صفوفًا أكثر أثناء التمرير |
| `Paging::virtual_scroll()` | القوائم الكبيرة جدًا؛ الصفوف الظاهرة فقط في الصفحة |
| `Paging::none()` | القوائم القصيرة التي تتسع لها صفحة واحدة |

يضع الخادم حدًا أعلى لحجم الصفحة مهما طلب العميل.

## الأعمدة

| الاستدعاء | النتيجة |
|---|---|
| `.width(px)`, `.flex(n)` | عرض ثابت أو حصة من العرض الحر |
| `.filter()` | خانة مرشح تناسب النوع: نص، أو نطاق أرقام، أو نطاق تواريخ، أو قائمة اختيارات |
| `.sum()`, `.avg()`, `.min()`, `.max()` | مجاميع في التذييل. `sum` على عمود نصي لا تُترجم |
| `.badge(\|v\| tone)` | يعرض القيمة تسميةً بلون النبرة على تدرجها الخفيف |
| `.status(\|v\| tone)` | يعرض القيمة حالةً: نقطة بلون النبرة قبل نص عادي، أهدأ من الشارة |
| `.progress(max)` | شريط من 0 إلى `max` |
| `.check_icon()` | علامة صح لـ `true` |
| `.link(\|row\| effect)` | يجعل الخلية رابطًا يشغّل تأثيرًا (effect) |
| `.pin_start()`, `.hidden()` | مثبّت في البداية، أو مخفي إلى أن يُظهره المستخدم |
| `.editable()` | قابل للتحرير في مكانه، ويُحفظ بـ `.edit_with(action)` |
| `.plain()` | أعداد صحيحة دون فواصل الآلاف، للرموز والسنوات |
| `.wrap()` | تلتف القيم الطويلة على أسطر أكثر. من دونه تبقى الخلايا في سطر واحد وتنتهي بعلامة حذف، فيكون لكل الصفوف الارتفاع نفسه |

ارتفاع الصفوف 40 px (32 px في الكثافة المضغوطة) ولا يفصل بينها إلا خطوط؛ ويظلل `DataGrid::zebra()` صفاً من كل صفين في الشبكات العريضة جداً. تُقرأ الروابط في الخلايا كنص بوزن 500 ويظهر تحتها خط عند التمرير.

تُحاذى الأرقام والمبالغ إلى اليمين بأرقام متساوية العرض. وتُجمَّع الأعداد الصحيحة حسب إعدادات المستخدم المحلية (`1,234,567` بالإنجليزية و`1.234.567` بالتركية) في خلايا الجدول وفي القيم المعروضة مثل `DescriptionList`؛ ولا تُجمَّع معرّفات `#[key]` أبدًا، ويوقف `.plain()` التجميع في عمود واحد.

## الصفوف

- `.on_open(|row| effect)`: نقر مزدوج أو Enter على صف.
- `.row_action(label, action, |row| input)`: زر على كل صف.
- `.bulk_action(label, action, M::id)`: يعمل على الصفوف المحددة.
- `.confirm(message)` و`.tone(Tone::Danger)` مباشرةً بعد `.row_action` أو `.bulk_action`: يسأل قبل تشغيل ذلك الإجراء في مربع الحوار نفسه الذي يستخدمه `ActionButton::confirm` (يبدأ التركيز على الإلغاء)، ويرسم زره باللون الأحمر.
- `.row_menu(|row| ContextMenu)`: قائمة النقر الأيمن للصف.
- `.row_style(|row| Option<RowStyle>)`: صفوف الخطر أو التحذير أو النجاح.
- `.add_with(create)` و`.delete_with(action, M::id)` و`.batch_edit()`: زر "إضافة صف"، وزر ومفتاح
  للحذف، وتعديلات تُحفظ معًا. راجع [مرجع DataGrid](grid.md).
- `.groupable()` و`.summary(label, |t| ...)` على عمود: يجمّع المستخدم حسبه من قائمته أو من لوحة التجميع، وتظهر تحته قيمة محسوبة من المجاميع.
- `.filter_builder()`: زر "تصفية" يفتح نافذة لبناء شروط التصفية؛ وهي القائمة نفسها التي يستخدمها صف التصفية والعروض المحفوظة ووكلاء الذكاء الاصطناعي.
- `.exportable()`: تصدير CSV للصفوف المصفّاة، يبثه الخادم (stream).

## أعمدة تُعرف وقت التشغيل

`DataGrid::dynamic(action)` مخصص للنتائج التي تأتي أعمدتها مع الصفوف، مثل معاينة SQL. يعيد الـ action قيمة `DynamicPage` فيها الأعمدة (الاسم والتسمية و`DynamicKind` مثل `Integer` أو `Decimal` أو `Date`) وصفحة واحدة من الصفوف بصيغة JSON:

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

يعمل الترقيم والتمرير الافتراضي واستخدام لوحة المفاتيح وتنسيق الأرقام والتواريخ بلغة المستخدم كما في grid ذي الأنواع. تُهرَّب الأسماء والتسميات والخلايا، ويحدّ الخادم عدد الأعمدة والصفوف وطول النص. لا يرتّب هذا الـ grid ولا يصفّي، وصفوفه للقراءة فقط. راجع [مرجع DataGrid](grid.md).

## البحث والأشرطة

`.search()` يضع مربع بحث فوق الجدول. يتلقى الإجراء النص على شكل `DataQuery::q`، ويطابقه
`DataQuery::apply` مع الحقول النصية دون المقنّعة منها.
`.band(label, |b| b.column(..).column(..))` يجمع الأعمدة تحت خلية عنوان واحدة. راجع
[مرجع DataGrid](grid.md).

## التحديث في المكان

يجعل `.listens(topic)`، المستدعى في النهاية، الشبكة تعيد التحميل في مكانها عند ذكر الموضوع (انظر [المواضيع](effects.md#topics)): بعد `ui.refresh(topic)` تعيد تحميل صفحتها الحالية مع ترتيبها ومرشحاتها، وبعد تغيّر عنصر تصفية يحمل `.drives(topic)` تبدأ من الصفحة الأولى مع params الشاشة الجديدة. يُعاد تحميل الشبكة عبر `POST /_zeyn/grid` مع كل فحوص الصلاحيات، ويصل param الشاشة الذي غيّره المرشح إلى الشبكة عبر دالة الشاشة، مثلًا على أنه scope:

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

يغطي [مرجع DataGrid](grid.md) العروض المحفوظة والرئيسي والتفصيلي (master detail) والتجميع وبيانات الشجرة والتحرير المضمن والتنقل بين الخلايا والنسخ واللصق وتحويل الأعمدة إلى افتراضية (virtualization) وإمكانية الوصول.
