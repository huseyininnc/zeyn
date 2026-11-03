# البيانات وقواعد البيانات

لا يخفي Zeyn قاعدة البيانات خلف مُخطِّط كائنات (object mapper). تعيش البيانات في خدمات (مخازن stores) يسجلها التطبيق؛ وتستقبلها الإجراءات وسائط. يمكن للمخزن أن يحتفظ بالصفوف في الذاكرة، أو يستدعي قاعدة بيانات، أو يستدعي خدمة أخرى. أما الشاشات والشبكات وواجهة API فلا ترى سوى الإجراءات.

## مخزن

```rust
#[derive(Clone)]
pub struct ProductStore {
    db: Pool,
}

impl Service for ProductStore {}

impl ProductStore {
    pub async fn get(&self, id: ProductId) -> Result<Product> {
        self.db.product(id).await.map_err(|e| Error::Internal(e.to_string()))?.ok_or(Error::NotFound)
    }
}
```

سجّله مرة واحدة بـ `App::service(store)`. يرفض التطبيق أن يبدأ عندما تحتاج شاشة أو واجهة API إلى مخزن غير مسجل، ويقول أيّ مخزن هو.

## القوائم: DataQuery وPage

يأخذ إجراء القائمة `DataQuery` ويعيد `Page<M>`:

| الحقل | المعنى |
|---|---|
| `page` | `Offset { offset, limit }` للصفحات المرقمة، أو `After { cursor, limit }` للتمرير اللانهائي |
| `sort` | الحقول والاتجاهات، بالترتيب |
| `filters` | الحقل والمعامل والقيمة، بنوع يحدده العمود |
| `with_total` | هل يحتاج العميل إلى العدد الكلي |
| `aggregates` | مجاميع الأعمدة التي تُحسب على كل الصفوف المصفّاة |
| `time_zone` | المنطقة الزمنية لمرشحات التاريخ على أعمدة `DateTime`، يضبطها الخادم |
| `group` | تجميع الصفوف حسب حقل، تضبطه الشبكة |

تنفذ `DataQuery::apply(vec)` كل ذلك في الذاكرة. ولقاعدة بيانات SQL استخدم `zeyn::sql`: تحوّل `Db::find` الاستعلام نفسه إلى `WHERE` و`ORDER BY` و`LIMIT` و`GROUP BY` لقاعدة PostgreSQL أو Oracle. لا يمكن ترشيح أو فرز إلا حقول الكيان نفسه، وكل قيمة تُربط معاملًا (parameter)، ويضع الخادم حدًا أعلى لحجم الصفحة، فلا يصل مدخل العميل إلى نص SQL أبدًا. اكتب SQL يدويًا فقط للاستعلامات التي لا تستطيع `Db::find` التعبير عنها، وعندها لا تلصق فيه اسم حقل أو قيمة من العميل أبدًا.

```toml
zeyn = { version = "0.0.7", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

يغطي [مرجع قاعدة البيانات](/reference/database) إعدادات الاتصال وOracle وعمليات الترحيل (migrations) والمعاملات (transactions) والتدفقات (streams) وSQL الذي يصبح عليه كل استعلام.

تحمل `Page` الصفوف، والعدد الكلي عند طلبه، ومؤشر الدفعة التالية، والمجاميع:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## إصدارات الصفوف

يفعّل الحقل `#[version] pub version: i64` القفل المتفائل (optimistic locking). ترسل النماذج الإصدار الذي حمّلته؛ ويقارنه المخزن ويعيد `Error::Conflict` عندما يحفظ شخص آخر في الأثناء. ثم يخبر النموذج المستخدم بدل أن يكتب فوق تغيير زميله.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## التحقق في المخزن

القواعد التي تحتاج إلى البيانات، مثل `unique`، تُفحص في المخزن وتُعاد أخطاء حقول:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## المستأجرون (Tenants)

مع `App::tenancy(resolver)` يحمل كل طلب مستأجرًا (`ctx.tenant()`)، يأتي من اسم المضيف (`SubdomainTenant`) أو من ترويسة (`HeaderTenant`)، أو هو مستأجر المستخدم نفسه عندما لا يسمّي الطلب أحدًا. يجب أن ينتمي المستخدم المسجّل دخوله إلى مستأجر الطلب (`Identity::member_of`)، وإلا يعمل الطلب كأنه بلا تسجيل دخول. يستخدمه المخزن لاختيار مخطط (schema) أو ترشيح الصفوف؛ وترفض `TenantRequired` الطلبات التي لا مستأجر لها.

## حالة المستخدم المحفوظة

يمكن حفظ عروض الشبكات (الأعمدة والفرز والمرشحات) وتخطيطات الإرساء (dock) والسمة المختارة لكل مستخدم على الخادم بـ `App::datagrid_views(store)`. يحتفظ `MemoryViewStore` بها في الذاكرة للتطوير؛ ويطبّق التطبيق `DataGridViewStore` بقاعدة بياناته للإنتاج.

## العمل في الخلفية

تعمل عمليات الاستيراد والتصدير وغيرها من الأعمال الطويلة مهام خلفية مع تقدّم (`Jobs`)، فلا ينتظرها طلب أبدًا. راجع [التدقيق والإخفاء والمهام](enterprise.md).

تعيش المهام في الذاكرة ما لم يعطها التطبيق مخزنًا. مع `SqlJobStore` تُحفظ في قاعدة البيانات التشغيلات المصطفة والتشغيل التالي لكل جدول زمني: فهي تنجو من إعادة التشغيل، وتتشارك عدة خوادم طابورًا واحدًا، ويضمن عقد إيجار (lease) ألا يشغّل عاملان التشغيل نفسه.

```rust
let jobs = SqlJobStore::new(db.clone());
jobs.create_tables().await?;

App::new("Billing")
    .job_store(jobs)
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }

#[action(permission = "billing.invoice.send")]
pub async fn send(id: InvoiceId, jobs: Jobs) -> Result<()> {
    jobs.enqueue("send_invoice", id.0.to_string()).await?;
    Ok(())
}
```

- يضع `jobs.enqueue(name, payload)` تشغيلًا في الطابور؛ ويؤجله `enqueue_after`. الحمولة (payload) نص لا يزيد على 4000 بايت، مثل معرّف أو JSON صغير.
- يستولي العامل على تشغيل مستحق بعقد إيجار مدته دقيقة ويجدده ما دام التشغيل يعمل. إذا توقف الخادم يُستولى على التشغيل مجددًا بعد انتهاء العقد؛ وتُحسب تلك المحاولة.
- تنتظر المحاولة الفاشلة فترة التراجع (backoff) التي تتضاعف حتى عشر دقائق، إلى أن تُستنفد إعادات المحاولة. عندها يبقى التشغيل في الجدول بحالة `failed` مع رمز خطئه. الجدول الزمني لا يفشل نهائيًا أبدًا: فهو ينتقل إلى فترته التالية.
- يستولي PostgreSQL بـ `FOR UPDATE SKIP LOCKED`، وOracle (الإصدار 11g وما بعده) بالطريقة نفسها عبر استعلام عن المعرّفات. الجدول هو `zeyn_jobs`؛ وتعطي `create_job_tables(dialect, prefix)` العبارات اللازمة لعملية ترحيل.
- يحتفظ المخزن الذي في الذاكرة (`MemoryJobStore`) بما لا يزيد على 10,000 تشغيل.
