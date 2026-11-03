# البطاقات والأشرطة والبحث

الصفحات العامة مثل المتجر تعرض الصفوف على شكل بطاقات بدل جدول. تحمّل `CardGrid` و`Rail` و`SearchBox` صفوفها عبر النوع نفسه من الإجراءات الذي تستخدمه DataGrid: يأخذ `DataQuery` ويعيد `Page`. يجري الترقيم والفرز ونص البحث على الخادم، وتحتاج القراءة إلى وصول الإجراء، كما في DataGrid.

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## شبكة البطاقات

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **البطاقات.** تصنع `Card::image(image, title)` بطاقة واحدة من صف، مع `text` و`badge` و`alt` و`link` (شاشة ومعاملاتها؛ البطاقة كلها رابط واحد وتُفتح دون إعادة تحميل الصفحة). الصور تُحمَّل كسولًا (lazy)، بالعرض والارتفاع من `image_size` حتى لا تقفز الصفحة. النص البديل للصورة فارغ، لأن العنوان المجاور لها يسمّي البطاقة؛ اضبط `alt` عندما تقول الصورة أكثر من ذلك. عناوين الصور غير المسار أو `https:` أو `data:image/` تُحذف.
- **الترقيم.** `Paging::infinite(n)` (الافتراضي، 24) يعرض زر "Show more" الذي يعمل أيضًا عندما يظهر عند التمرير؛ وينتقل التركيز إلى أول بطاقة جديدة عند الضغط على الزر. `Paging::pages(n)` يعرض زري السابق والتالي ورقم الصفحة. يحدد الخادم حجم الصفحة؛ ولا يقول العميل سوى من أين يتابع.
- **الفرز.** كل `sort_option` ترتيب يمكن للمستخدمين اختياره من قائمة منسدلة؛ والأول هو الترتيب الذي تبدأ به البطاقات. لا يقبل الخادم إلا هذه الترتيبات، ويُسقط المرشحات ونص البحث التي يرسلها العميل.
- **البحث.** `.search(text)` يعرض الصفوف المطابقة للنص الحر ([`DataQuery::q`](#free-text)) فقط، مثل نص شاشة نتائج.
- **الحقول المقنّعة.** تُبنى البطاقات بدالة مغلقة (closure) ترى الصف الخام. بالنسبة إلى نموذج فيه حقول `#[mask]`، لا يحصل على البطاقات إلا المستخدمون الذين يحق لهم كشفها؛ أما الباقون فيحصلون على `Forbidden`. استخدم نموذجًا بلا حقول مقنّعة للبطاقات العامة.

## الشريط (Rail)

الشريط شبكة بطاقات مرتبة في صف واحد يتمرر جانبيًا، مع التقاط عند التمرير (scroll snapping) وزري السابق والتالي اللذين يمرران بعرض عرض واحد (بسلاسة ما لم يطلب المستخدم تقليل الحركة). يحمّل الدفعة التالية، 12 في كل مرة، عندما يقترب المستخدم من نهايته. البطاقات روابط ضمن ترتيب Tab وتظهر في العرض عندما تنال التركيز.

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## صندوق البحث

حقل بحث مع اقتراحات، يتبع نمط combobox من W3C APG مع قائمة منبثقة (list box). عندما يكتب المستخدم ينتظر `instant` ميلي ثانية ثم يطلب الاقتراحات من الخادم: يعمل الإجراء مع النص بوصفه `DataQuery::q`، ويعود نص اقتراح كل صف (أول حقل نصي غير مقنّع فيه ما لم يحدد `suggestion` غير ذلك). يفتح Enter، أو اختيار اقتراح، شاشة النتائج مع النص، دون إعادة تحميل الصفحة.

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

لوحة المفاتيح: يتنقل السهمان لأسفل ولأعلى بين الاقتراحات (السهم لأسفل يفتحها)، ويبحث Enter بالاقتراح المميز أو بالنص المكتوب، ويغلق Escape الاقتراحات ثم يمسح الحقل. تخبر منطقة حية (live region) قارئات الشاشة بعدد الاقتراحات.

تأخذ شاشة النتائج `SearchParams` (`q`، فارغ عندما تُفتح الشاشة من قائمة). يقص الخادم النص عند `MAX_SEARCH_QUERY` (100) حرف، ويقرر عدد الاقتراحات المرسلة (8 ما لم يحدد `limit` غير ذلك، وحد أقصى 20)، ويتحقق من وصول الشاشة وحراس الميزة (guards) وصلاحية الإجراء، ويحسب كل استدعاء اقتراحات ضمن حد معدل الكتابة (`App::write_limit`).

## النص الحر

`DataQuery::q` نص حر: الصفوف التي تحتوي حقولها النصية عليه، مع تجاهل حالة الأحرف. يطابق الحرف التركي ذو النقطة وبلا نقطة (`İ`, `ı`) الحرف `i`، فيجد "istanbul" النص "İSTANBUL". الحقول ذات `#[mask]` لا يُبحث فيها أبدًا، والنص الفارغ يطابق كل الصفوف، والنص الأطول من 100 حرف يُتجاهل. تقرؤه `DataQuery::apply` وطبقات بيانات SQL (`LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...` مع معاملات مربوطة) بالطريقة نفسها.
