# الإجراءات (Actions)

الإجراء (action) هو كود على الخادم يمكن لشاشة أو شبكة أو نموذج أو أمر أو واجهة JSON API أو وكيل ذكاء اصطناعي أن يستدعيه. يعلن `#[action]` من يحق له الاستدعاء، ويتحقق الإطار من ذلك قبل تشغيل الكود.

## إعلان إجراء

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- الوسيط الأول هو المدخل. يأتي من العميل، ولذلك يقرؤه الإطار على أنه JSON غير موثوق.
- كل وسيط آخر يُحقن: `Ctx` أو `Service` مسجلة.
- نوع الإرجاع هو `Result<T>`.

## الوصول

كل إجراء يعلن واحدًا فقط مما يلي:

| الخيار | المعنى |
|---|---|
| `permission = "feature.resource.verb"` | تحتاج الهوية إلى هذه الصلاحية |
| `public` | أي شخص، بمن فيهم من لم يسجلوا الدخول |

الإجراء بلا وصول لا يُترجم (لا يُصرَّف). أسماء الصلاحيات بأحرف صغيرة ومفصولة بنقاط، مثل `sales.order.write`.

خيارات أخرى:

| الخيار | المعنى |
|---|---|
| `audit` | يكتب سجل تدقيق عن كل استدعاء |
| `reauth` | يجب أن يكون المستخدم قد سجل الدخول حديثًا (`App::reauth_within`) |

## الأخطاء

| الخطأ | الحالة | الاستخدام |
|---|---|---|
| `Error::NotFound` | 404 | السجل غير موجود أو لا يحق للمستخدم أن يعرف بوجوده |
| `Error::Forbidden` | 403 | الوصول مرفوض |
| `Error::Denied(t!("..."))` | 403 | مرفوض مع سبب يستطيع المستخدم قراءته |
| `Error::Conflict` | 409 | تغير إصدار الصف منذ تحميل النموذج |
| `Error::Validation(errors)` | 422 | أخطاء الحقول، تظهر بجوار الحقول |
| `Error::Internal("detail")` | 500 | تذهب التفاصيل إلى سجل الخادم فقط |

خطأ في حقل من الكود:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## القوائم

تأخذ القوائم `DataQuery` وتعيد `Page<M>`. إجراء القائمة لا يعيد كل الصفوف أبدًا؛ فالشبكة تطلب صفحات، ويضع الخادم حدًا أعلى لحجم الصفحة مهما أرسل العميل.

## السياق

`Ctx` هو الطلب: `ctx.identity()` و`ctx.locale()` و`ctx.time_zone()` و`ctx.now()` و`ctx.today()` و`ctx.tenant()` و`ctx.license()` و`ctx.config()` و`ctx.theme()` و`ctx.t(&text)`. استخدم `ctx.now()` و`ctx.today()` بدل ساعة النظام، حتى تتمكن الاختبارات من تثبيت الوقت بـ `App::fixed_now`.

## أين تُستخدم الإجراءات

- `DataGrid::of(action)` يحمّل الصفوف؛ و`.edit_with(action)` و`.row_action(...)` و`.bulk_action(...)` تغيّرها.
- `Form::submit(label, action)` يحفظ نموذجًا.
- `ActionButton::new(label, action, input)` يشغّل إجراءً من زر.
- `Command::action(...)` يشغّل إجراءً من لوحة الأوامر أو من اختصار.
- `Feature::api(action)` ينشره بصيغة JSON (راجع [API وOpenAPI](api.md)).
- `Feature::search(action)` يضيف بحثًا على الخادم إلى لوحة الأوامر.

كل مسار يجري الفحوص نفسها: CSRF لاستدعاءات المتصفح، ثم الحراس (guards) الخاصة بالميزة، ثم وصول الإجراء، ثم المعترضات (interceptors).
