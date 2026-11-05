# عناصر التحكم والمنتقيات (Controls and pickers)

تختار حقول النموذج عنصر التحكم من نوعها. يغيّره `.control(M::f, Control::...)`، ويحوّل `.lookup(M::f, action)` الحقل إلى بحث على الخادم. كل عنصر تحكم هو عنصر نموذج أصلي في أساسه، فتُرسل القيم بالطريقة نفسها مع بيئة التشغيل في المتصفح (runtime) أو بدونها، ويتبع كل عنصر نمط ARIA الخاص به لمستخدمي لوحة المفاتيح وقارئات الشاشة.

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## عناصر التحكم الافتراضية

| نوع الحقل | عنصر التحكم |
|---|---|
| `String` | حقل نص؛ ومنطقة نص تنمو مع عداد أحرف عندما يزيد `max_len` على 255 |
| `i32`, `i64` | حقل رقم مع القاعدتين `min` و `max` |
| `Money` | مبلغ بتنسيق أرقام المستخدم، مع العملة |
| `Decimal` | رقم دقيق بتنسيق المستخدم، مع `scale` الحقل (انظر [إدخال Decimal](#decimal-and-percent)) |
| `bool` | خانة اختيار (Checkbox) |
| enum من نوع `#[choice]` | Select: زر يفتح قائمة قابلة للبحث |
| `Vec` من enum من نوع `#[choice]` | اختيار متعدد: رقائق (chips) وقائمة تُختار منها عدة خيارات |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | منتقيات التاريخ والوقت |
| `Vec<M>` من نموذج | محرر أسطر (انظر [محرر الأسطر](lines.md)) |

## عناصر تحكم أخرى

| عنصر التحكم | يُستخدم لـ |
|---|---|
| `Control::Radio` | اختيار بخيارات قليلة، كلها ظاهرة |
| `Control::Segmented` | اختيار بين اثنين إلى خمسة خيارات قصيرة |
| `Control::Listbox` | قائمة اختيار طويلة مع البحث بالكتابة |
| `Control::Switch` | قيمة `bool` تسري كإعداد |
| `Control::Percent` | قيمة `Decimal` أو عدد صحيح تُعرض مع علامة النسبة المئوية |
| `Control::Password`, `Control::NewPassword` | حقل نص من نوع كلمة مرور، مع زر إظهار وإخفاء |
| `Control::PasswordMeter` | كلمة مرور جديدة مع مقياس قوة |
| `Control::Rating` | عدد صحيح من 1 إلى N يظهر على شكل نجوم (انظر [Rating](#rating)) |

## Rating

يعرض `Control::Rating` حقلًا لعدد صحيح على شكل نجوم من 1 إلى N، حيث N هو قاعدة `max` للحقل (5 إن لم توجد قاعدة، وبحد أقصى 10). وهو مجموعة من أزرار الراديو الأصلية داخل `fieldset` تكون legend فيه هي تسمية الحقل، لذلك يتبع نمط مجموعة الراديو في W3C APG دون أي سكربت: يصل Tab إلى المجموعة مرة واحدة، وتحرك مفاتيح الأسهم وتحدد، وتُسمّى كل نجمة بلغة المستخدم مثل "3 من 5". عند التمرير تضيء النجوم حتى المؤشر. يتحقق الخادم من القيمة بقواعد الحقل كأي رقم، لذا حدد للحقل `min` و`max`؛ وعند وجود `required` على `Option<i64>` يُرفض ترك كل النجوم فارغة.

```rust
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

Form::of(Feedback::default())
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit(t!("feedback.send"), send_feedback)
```

@live rating


## Lookup

يبحث الـ lookup في السجلات على الخادم أثناء كتابة المستخدم. يأخذ الـ action كائن `LookupQuery` ويعيد `Page<LookupItem>`؛ ويخزن الحقل المعرّف (id).

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

يفعل `.lookup_many(M::f, action)` الشيء نفسه لحقل `Vec<i64>` ويعرض المختارات كوسوم قابلة للإزالة.

يعرض `.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` عدة أعمدة في القائمة. يذهب البحث دائماً إلى الـ action على الخادم، وتُفحص صلاحيته عند بحث كل ضغطة مفتاح؛ ويعطي كل عنصر قيمه بـ `LookupItem::cells` بترتيب الترويسات. ويبحث `LookupQuery::apply` في الخلايا أيضاً. تسمع قارئات الشاشة كل خيار هكذا "Code: C-2, Name: Mert Kaya, City: Ankara". وعلى الشاشات الضيقة تتكدس الأعمدة داخل كل خيار.

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## الاختيار المتعدد

يُحرَّر `Vec` من enum من نوع `#[choice]` بالاختيار المتعدد: تظهر الخيارات المختارة كرقائق مع زر إزالة، ويفتح combobox قائمة تُفعَّل فيها الخيارات وتُعطَّل (قائمة APG listbox مع `aria-multiselectable` داخل combobox). وفي الأساس عنصر `<select multiple>` أصلي، فيعمل الحقل دون بيئة التشغيل. ترسل بيئة التشغيل القيم مضمومة بفواصل (`Email,Sms`)؛ ويقرؤها الخادم واحدة واحدة، ويرفض القيم المجهولة ويسقط المكررات. وفي JSON (واجهات API وأدوات الذكاء الاصطناعي وقاعدة البيانات) يكون الحقل مصفوفة من قيم الخيارات.

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| المفتاح | ماذا يفعل |
|---|---|
| Arrow Down, Arrow Up, Enter, Space | يفتح القائمة على combobox المغلق |
| Home, End | يفتح القائمة على أول خيار أو آخر خيار |
| Arrow Down, Arrow Up | ينتقل عبر القائمة المفتوحة |
| Shift مع Arrow Down أو Arrow Up | ينتقل ويفعّل الخيار التالي أو يعطّله |
| Space, Enter | يفعّل الخيار النشط أو يعطّله؛ وتبقى القائمة مفتوحة |
| الحروف | تقفز إلى الخيار التالي الذي يبدأ بها |
| Backspace, Delete | على combobox المغلق: يزيل آخر خيار مختار |
| Escape, Alt مع Arrow Up | يغلق القائمة؛ ويبقى التركيز على combobox |
| Tab | يغلق القائمة وينتقل إلى ما بعدها |

يُترك Ctrl+A (تحديد الكل في نمط APG) للمتصفح، كما تقتضي سياسة الاختصارات؛ وأزرار إزالة الرقائق ضمن ترتيب Tab. تحتاج القائمة والرقائق وأعمدة الـ lookup إلى نحو 1.7 KB من السكربت بعد ضغط gzip ولا تحتاج إلى طلب؛ والحقل 3 عقد زائد عقدة لكل خيار مختار.

## Decimal والنسبة المئوية

تحمل حقول `Decimal` أرقاماً دقيقة: لا فاصلة عائمة في أي مكان بين الإدخال والخادم وقاعدة البيانات والتصدير. ينسّق الإدخال الرقم بلغة المستخدم أثناء الكتابة (`1.234,5678` بالتركية، و `1,234.5678` بالإنجليزية) ويسمح بعدد الخانات العشرية `scale` للحقل؛ ويقرؤه الخادم بالكيفية نفسها ويفحص `scale` و `precision` و `min` و `max` مرة أخرى.

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

يحتفظ `Control::Percent` بالقيمة بالنقاط المئوية: تُعرض `12.5` على أنها `12.5 %` بالإنجليزية و `%12,5` بالتركية وتُخزَّن `12.5`. وتسمع قارئات الشاشة كلمة "percent" عبر وصف الإدخال.

## الإدخال المقنّع (Masked input)

يعرض حقل النص ذو القاعدة `mask` نمطه أثناء كتابة المستخدم ولا يرسل إلا الأحرف الخام. النمط هو القاعدة: يقرأ الخادم الثوابت المكتوبة بعيداً (`(532) 123 45 67` تصبح `5321234567`)، ويحوّل خانات `A` إلى أحرف كبيرة، ويرفض قيمة لا تملأ كل الخانات. تُرسل استدعاءات API وأدوات الذكاء الاصطناعي القيمة الخام وتحصل على الفحص نفسه.

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| محرف النمط | معناه |
|---|---|
| `9` | رقم |
| `a` | حرف |
| `A` | حرف، يُكتب كبيراً |
| `*` | حرف أو رقم |
| شرطة مائلة عكسية | المحرف التالي ثابت (literal)، مثل `\T` |
| أي شيء آخر | ثابت: يُعرض ولا يُخزَّن أبداً |

بدون بيئة التشغيل يكون الحقل حقل نص عادياً بالقيمة الخام؛ ويستطيع المستخدمون كتابتها مع الثوابت أو بدونها.

## كلمة المرور (Password)

`Control::Password` (تسجيل الدخول) و `Control::NewPassword` (التسجيل والتغيير) و `Control::PasswordMeter` (كلمة مرور جديدة مع مقياس قوة) حقول كلمة مرور لا تُملأ من الخادم أبداً ولا تُحفظ في المسودات أبداً. يتبع زر الإظهار والإخفاء نمط APG لزر التبديل (toggle button): يبقى اسمه "Show password" ويخبر `aria-pressed` ما إذا كان النص ظاهراً. ويعيد النموذج كلمة المرور إلى الإخفاء عند الإرسال.

المقياس تلميح يُحسب في المتصفح من الطول وأنواع المحارف والأنماط الواضحة، ويُعرض كأربعة أشرطة ونص ("Strength: good") في منطقة حية مهذبة (polite live region). وما يقبله الخادم تحدده قواعد الحقل (`min_len` ودالة `validate` في النموذج)، لا المقياس أبداً.

@live inputs

### لوحة المفاتيح

| المفتاح | أين | ماذا يفعل |
|---|---|---|
| Arrow Up, Arrow Down | حقول decimal والنسبة المئوية | يزيد أو ينقص بوحدة صحيحة واحدة ضمن `min` و `max` |
| Page Up, Page Down | حقول decimal والنسبة المئوية | يزيد أو ينقص بعشر وحدات |
| Backspace | الإدخال المقنّع | يحذف المحرف قبل المؤشر؛ وفوق ثابت (literal) يحذف المحرف الذي قبل الثابت |
| Tab | حقل كلمة المرور | ينتقل إلى زر الإظهار والإخفاء |
| Enter, Space | زر الإظهار والإخفاء | يُظهر كلمة المرور أو يُخفيها؛ ويبقى التركيز على الزر |

### الميزانية والفحوص

| | |
|---|---|
| السكربت المضاف | نحو 2 KB بعد ضغط gzip (7 KB خام) في `inputs.js` للأقنعة والتبديل والمقياس؛ وتستخدم حقول decimal والنسبة المئوية عنصر الأرقام |
| عقد DOM | 3 للإدخال المقنّع، و4 لكلمة المرور مع زر التبديل، و11 مع المقياس |
| الطلبات | لا شيء أثناء الكتابة |
| فحوص الخادم | يعمل `scale` و `precision` و `min` و `max` و `mask` عند كل حفظ، من الصفحة وواجهة API وأدوات الذكاء الاصطناعي على حد سواء |

## منتقيات التاريخ والوقت

تقبل المنتقيات إدخالاً مكتوباً بتنسيق المستخدم وتفتح تقويماً أو قائمة أوقات. يضبط `DateOptions` الحدود: `.no_weekends()` و `.week_numbers()` و `.step_minutes(15)`. ويدمج `.range(from, to)` حقلي تاريخ في منتقي نطاق واحد مع إعدادات مسبقة مثل *This month*. المثال الحي في [الوقت واللغات](time-and-languages.md#pickers).
