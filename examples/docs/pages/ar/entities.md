# الكيانات والنماذج

تُوصَف البيانات مرة واحدة، على شكل struct في Rust مع سمات (attributes). يقود الوصف نفسه النماذج (forms) وأعمدة الشبكات والتحقق على الخادم وفي المتصفح وواجهة JSON API ومخطط OpenAPI الخاص بها.

## الكيان (Entity)

يعلّم `#[entity(table = "...")]` صف جدول. يحتاج إلى حقل `#[key]` واحد وينشئ معرّفًا مكتوب النوع، مثل `ProductId`، ومرجع حقل لكل حقل، مثل `Product::code`.

```rust
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}
```

مراجع الحقول مكتوبة الأنواع: `Product::price` هو `Field<Product, Money>`. مجموع شبكة على عمود نصي أو حقل نموذج من نموذج آخر لا يُترجم.

## النموذج (Model)

`#[model]` بيانات ليست صف جدول: مرشحات وخطوات معالج (wizard) ومدخلات نماذج.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## الاختيار (Choice)

يحوّل `#[choice]` تعدادًا (enum) من متغيرات وحدوية قائمة منسدلة أو مجموعة أزرار راديو أو عنصر تحكم مجزّأ. تأتي التسميات من ملفات اللغات.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## سمات الحقول

| السمة | المعنى |
|---|---|
| `#[key]` | المفتاح الأساسي للكيان |
| `#[version]` | إصدار صف من نوع `i64`. الحفظ بإصدار قديم يفشل بـ `Error::Conflict` |
| `#[readonly]` | يُعرض ولا تكتبه النماذج أبدًا |
| `#[label("...")]` | تسمية احتياطية عندما لا يكون في ملفات اللغات واحدة |
| `#[rule(...)]` | قواعد التحقق، أدناه |

## القواعد

| القاعدة | على | المعنى |
|---|---|---|
| `required` | أي نوع | يجب أن تكون له قيمة |
| `min_len = n`, `max_len = n` | نص | الطول بالأحرف |
| `email` | نص | عنوان بريد إلكتروني |
| `embed` | نص | عنوان للتضمين بـ `Embed`: `https` على مضيف ضمن `frame-src` الخاص بالتطبيق (`App::csp`). تقبل النماذج أيضًا كود `<iframe>` الملصوق وتخزّن عنوانه القانوني فقط، دون معاملات التتبع. راجع [العرض](display.md#embed) |
| `min = n`, `max = n` | الأرقام والمال والأعداد العشرية والتواريخ | حدود. تُكتب التواريخ هكذا `2026_01_01` |
| `scale = n`, `precision = n` | `Decimal` | أكثر عدد من المنازل العشرية (2 بدون القاعدة) وأكثر عدد من الأرقام في المجموع (18 بدون القاعدة). وهما تحددان أيضًا نوع العمود، `NUMERIC(precision, scale)` |
| `mask = "..."` | نص | تملأ القيمة الخام نمطًا: `9` رقم، و`a` حرف، و`A` حرف كبير، و`*` أيهما؛ وتُعرض الأحرف الأخرى أثناء الكتابة لكنها لا تُخزَّن. راجع [عناصر التحكم](controls.md#masked-input) |
| `unique` | أي نوع | يفحصه المخزن؛ ويعرض النموذج الخطأ القادم من الإجراء |

القاعدة التي لا تناسب نوع الحقل خطأ تصريف برسالة تقول ماذا تفعل. القواعد التي تشمل عدة حقول توضع في دالة:

```rust
#[model(validate = check_stay)]
pub struct Stay {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn check_stay(s: &Stay, errors: &mut ValidationErrors) {
    if let (Some(a), Some(b)) = (s.from, s.to) {
        if b < a {
            errors.add(Stay::to, t!("stay.before_start"));
        }
    }
}
```

## Valid

الإجراء الذي يحفظ يأخذ `Valid<M>`. لا يمكن صنع قيمة `Valid` إلا بتشغيل كل قاعدة، ولذلك لا يستطيع الإجراء أن ينسى التحقق:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## أنواع الحقول

`String` و`i32` و`i64` و`bool` و`Money` و`Decimal` و`Date` و`Time` و`DateTime` و`LocalDateTime` وتعدادات `#[choice]` ومعرّفات الكيانات و`Option<T>` لأي منها. يصبح `Vec<M>` من نموذج محرّر سطور في النماذج (راجع [محرر السطور](lines.md)).

يُخزَّن المال بوحدات صغرى صحيحة مع عملته. لا تستخدم `f64` للمال أبدًا. و`Decimal` عدد دقيق لكل ما فيه كسر: الكميات وأسعار الوحدات بأكثر من منزلتين عشريتين والمعدلات. يتسع لـ 38 رقمًا، ولا يمر بعدد عشري عائم (float) أبدًا، وينتقل في JSON نصًا (`"12.50"`). تُغطى التواريخ والأوقات في [الوقت واللغات](time-and-languages.md).
