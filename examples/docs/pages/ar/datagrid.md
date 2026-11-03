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
| `.badge(\|v\| tone)` | يعرض القيمة شارةً ملونة |
| `.progress(max)` | شريط من 0 إلى `max` |
| `.check_icon()` | علامة صح لـ `true` |
| `.link(\|row\| effect)` | يجعل الخلية رابطًا يشغّل تأثيرًا (effect) |
| `.pin_start()`, `.hidden()` | مثبّت في البداية، أو مخفي إلى أن يُظهره المستخدم |
| `.editable()` | قابل للتحرير في مكانه، ويُحفظ بـ `.edit_with(action)` |

## الصفوف

- `.on_open(|row| effect)`: نقر مزدوج أو Enter على صف.
- `.row_action(label, action, |row| input)`: زر على كل صف.
- `.bulk_action(label, action, M::id)`: يعمل على الصفوف المحددة.
- `.row_menu(|row| ContextMenu)`: قائمة النقر الأيمن للصف.
- `.row_style(|row| Option<RowStyle>)`: صفوف الخطر أو التحذير أو النجاح.
- `.exportable()`: تصدير CSV للصفوف المصفّاة، يبثه الخادم (stream).

يغطي [مرجع DataGrid](grid.md) العروض المحفوظة والرئيسي والتفصيلي (master detail) والتجميع وبيانات الشجرة والتحرير المضمن والتنقل بين الخلايا والنسخ واللصق وتحويل الأعمدة إلى افتراضية (virtualization) وإمكانية الوصول.
