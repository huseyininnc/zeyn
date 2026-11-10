# Veri ve veritabanları

Zeyn veritabanını bir nesne eşleyicinin arkasına saklamaz. Veri, uygulamanın kaydettiği servislerde (store) yaşar; action'lar bunları argüman olarak alır. Bir store satırları bellekte tutabilir, bir veritabanını ya da başka bir servisi çağırabilir. Screen'ler, grid'ler ve API yalnızca action'ları görür.

## Bir store

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

Bir kez `App::service(store)` ile kaydedin. Bir screen ya da API kayıtlı olmayan bir store'a ihtiyaç duyarsa uygulama başlamayı reddeder ve hangisi olduğunu söyler.

## Listeler: DataQuery ve Page

Bir liste action'ı `DataQuery` alır ve `Page<M>` döndürür:

| Alan | Anlamı |
|---|---|
| `page` | Numaralı sayfalar için `Offset { offset, limit }`, sonsuz kaydırma için `After { cursor, limit }` |
| `sort` | Alanlar ve yönler, sırasıyla |
| `filters` | Alan, işleç ve değer; sütunun türüne göre tiplenir |
| `with_total` | İstemcinin toplam sayıya ihtiyacı olup olmadığı |
| `aggregates` | Süzülen her satır üzerinden hesaplanacak sütun toplamları |
| `time_zone` | `DateTime` sütunlarındaki tarih süzgeçleri için saat dilimi, sunucu tarafından atanır |
| `group` | Satırları bir alana göre gruplar, grid tarafından atanır |

`DataQuery::apply(vec)` bunların hepsini bellekte yapar. Bir SQL veritabanı için `zeyn::sql` kullanın: `Db::find`, aynı sorguyu PostgreSQL ya da Oracle için `WHERE`, `ORDER BY`, `LIMIT` ve `GROUP BY` ifadelerine çevirir. Yalnızca entity'nin kendi alanları süzülebilir ya da sıralanabilir, her değer parametre olarak bağlanır ve sayfa boyutu sunucuda sınırlanır; böylece istemci girdisi SQL metnine hiç ulaşmaz. SQL'i yalnızca `Db::find`'ın ifade edemediği sorgular için elle yazın: `Db::query::<M>(sql, args)` bir `SELECT`'in satırlarını bir modele okur, SQL'i yalnızca string literal olarak, her değeri parametre olarak alır ve en fazla 1000 satır okur.

```rust
let hits: Vec<Hit> = db
    .query("SELECT id, title FROM items WHERE title % $1 ORDER BY similarity(title, $1) DESC LIMIT 20", vec![q.into()])
    .await?;
```

```toml
zeyn = { version = "0.0.10", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

[Veritabanı başvurusu](/reference/database), bağlantı ayarlarını, Oracle'ı, migration'ları, transaction'ları, akışları ve her sorgunun dönüştüğü SQL'i kapsar.

`Page` satırları, istenirse toplamı, sonraki grubun imlecini (cursor) ve toplamları taşır:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## Satır sürümleri

`#[version] pub version: i64` alanı iyimser kilitlemeyi (optimistic locking) açar. Form'lar yükledikleri sürümü gönderir; store onu karşılaştırır ve arada başkası kaydettiyse `Error::Conflict` döndürür. Form da meslektaşın değişikliğinin üzerine yazmak yerine kullanıcıyı bilgilendirir.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## Store içinde doğrulama

`unique` gibi veriye ihtiyaç duyan kurallar store içinde denetlenir ve alan hataları olarak döndürülür:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Tenant'lar

`App::tenancy(resolver)` ile her istek bir tenant taşır (`ctx.tenant()`); bu tenant ana bilgisayar adından (`SubdomainTenant`), bir başlıktan (`HeaderTenant`) ya da istek hiçbirini belirtmiyorsa kullanıcının kendi tenant'ından gelir. Oturum açmış bir kullanıcı isteğin tenant'ına ait olmalıdır (`Identity::member_of`); aksi halde istek oturumsuz çalışır. Bir store bunu şema seçmek ya da satırları süzmek için kullanır; `TenantRequired` tenant'sız istekleri reddeder.

## Kayıtlı kullanıcı durumu

Grid görünümleri (sütunlar, sıralama, süzgeçler), dock yerleşimleri ve seçilen tema, `App::datagrid_views(store)` ile kullanıcı başına sunucuda tutulabilir. `MemoryViewStore` bunları geliştirme için bellekte tutar; bir uygulama üretim için `DataGridViewStore`'u kendi veritabanıyla uygular.

## Arka plan işleri

İçe ve dışa aktarmalar ile diğer uzun işler, ilerleme bilgisiyle arka plan işleri olarak çalışır (`Jobs`), böylece bir istek onları hiç beklemez. [Audit, masking and jobs](enterprise.md) sayfasına bakın.

Uygulama onlara bir store vermedikçe işler bellekte yaşar. `SqlJobStore` ile kuyruktaki çalıştırmalar ve her zamanlamanın sonraki çalıştırması veritabanında tutulur: yeniden başlatmadan etkilenmezler, birden çok sunucu tek bir kuyruğu paylaşır ve bir lease iki işçinin aynı çalıştırmayı asla yürütmemesini sağlar.

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

- `jobs.enqueue(name, payload)` bir çalıştırmayı kuyruğa alır; `enqueue_after` onu geciktirir. Payload, bir kimlik ya da küçük bir JSON gibi, en çok 4000 bayt uzunluğunda bir metindir.
- Bir işçi zamanı gelmiş bir çalıştırmayı bir dakikalık lease ile üstlenir ve çalıştırma sürdükçe lease'i yeniler. Sunucu durursa, lease bittikten sonra çalıştırma yeniden üstlenilir; bu deneme sayılır.
- Başarısız bir deneme, ikiye katlanan ve on dakikaya kadar çıkan geri çekilmeyi (backoff) bekler; yeniden deneme hakları bitene kadar bu sürer. Sonra çalıştırma, hata koduyla birlikte tabloda `failed` olarak kalır. Bir zamanlama asla kalıcı olarak başarısız olmaz: sonraki aralığına geçer.
- PostgreSQL `FOR UPDATE SKIP LOCKED` ile üstlenir, Oracle (11g ve sonrası) aynısını kimlik seçen bir sorgu üzerinden yapar. Tablo `zeyn_jobs`'tur; `create_job_tables(dialect, prefix)` bir migration için gereken ifadeleri verir.
- Bellek içi store (`MemoryJobStore`) en çok 10.000 çalıştırma tutar.
