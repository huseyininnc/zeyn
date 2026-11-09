# Veritabanı

`zeyn::sql` entity'leri bir SQL veritabanında tutar. Onları bir grid'in gönderdiği `DataQuery`'den
(filtreler, sıralama, sayfalar, toplamlar, gruplar) listeler ve satır sürümü denetimleriyle kaydeder.
PostgreSQL `postgres` Cargo feature'ının, Oracle ise `oracle` feature'ının arkasındadır; bu yüzden
veritabanı olmayan bir uygulama bunların hiçbirini derlemez.

```toml
zeyn = { version = "0.0.9", features = ["postgres"] }   # or ["oracle"], or both
```

## Kurulum

Bir `Db`'yi servis olarak kaydedin ve action'larda alın:

```rust
let db = Db::from_config(&config).await?;
App::new("Stock").config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}

#[action(permission = "stock.product.write")]
pub async fn save_product(p: Valid<Product>, db: Db) -> Result<Product> {
    if p.id.0 == 0 { db.insert(&*p).await } else { db.update(&*p).await }
}
```

`Db::from_config`, `database.url` değerini sır (secret) olarak ve havuz sınırlarını `database.*`
altından okur:

| Anahtar | Varsayılan | Anlamı |
|---|---|---|
| `database.url` | zorunlu | `postgres://user:password@host:5432/db` veya `oracle://user:password@host:1521/service` |
| `database.max_connections` | 10 | Havuz boyutu; istekler boş bir bağlantı için bekler |
| `database.min_connections` | 0 | Boştayken açık tutulan bağlantılar |
| `database.acquire_timeout_ms` | 5000 | Bir bağlantı için en uzun bekleme |
| `database.idle_timeout_s` | 600 | Minimumun üzerindeki boş bağlantılar bu süreden sonra kapanır |
| `database.statement_timeout_ms` | 30000 | Bir ifadenin en uzun çalışma süresi; 0 sunucu varsayılanını korur |
| `database.count_cap` | 1000000 | Bir sayfa için sayılan en büyük toplam |

`Db::connect(url, PoolLimits { .. })` aynı işi `Config` olmadan yapar. URL şeması sürücüyü seçer. Bir
Oracle parolasındaki `@` veya `:` gibi karakterler yüzde kodlamalı yazılır (`%40`).

PostgreSQL bağlantıları tembel (lazy) açılır, bu yüzden veritabanı kapalıyken uygulama başlar ve ilk
sorgu sorunu bildirir. Oracle, URL hangi SQL'in yazılacağını söylemedikçe (`?compat=11` veya
`?compat=12`) bağlanırken sunucudan sürümünü ister; `compat` ayarlıyken veritabanı olmadan da başlar.

`inventory` örneği ürünlerini bellekte veya `ZEYN_DATABASE_URL` değişkeninin adlandırdığı
veritabanında tutar (`--features postgres` veya `--features oracle` ile derleyin):

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_DATABASE_URL=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo run -p zeyn-example-inventory --features postgres
```

## Oracle Instant Client

Oracle sürücüsü, ODPI-C üzerindeki `oracle` crate'idir. Oracle yazılımı olmadan derlenir, ancak çalışma
zamanında Oracle Client kitaplığını (`oci.dll`, `libclntsh.so`) yükler. Oracle Instant Client'ın
kullanımı ücretsizdir:

1. Platformunuz için Instant Client Basic Light'ı Oracle'ın Instant Client sayfasından indirin. Windows
   x64 için `instantclient-basiclite-windows.x64-19.*.zip` zip dosyasını alın. Sürüm 19, Oracle
   Database 11.2'den 23ai'ye kadar bağlanır; daha yeni client'lar artık 11g'ye bağlanmaz.
2. Onu kendi klasörüne açın, örneğin `C:\oracle\instantclient_19`.
3. O klasörü `PATH` içinde ilk sıraya koyun veya `ZEYN_ORACLE_CLIENT_DIR` değişkenini ona ayarlayın.
   Değişken `PATH`'e üstün gelir; bu, başka bir Oracle home kuruluyken işe yarar. Linux'ta RPM'i kurun
   veya `LD_LIBRARY_PATH` ayarlayın; Basic Light ayrıca `libaio` ister.

Kitaplık olmadan ilk bağlantı `DPI-1047: Cannot locate a 64-bit Oracle
Client library` hatasıyla başarısız olur.

## Lehçeler (dialect)

SQL oluşturucu ortaktır. Bir `Dialect` yalnızca farklı olanı sağlar ve `Postgres` ile
`Oracle { compat }` onu uygular:

| | PostgreSQL | Oracle 12c ve sonrası (`V12`) | Oracle 11g (`V11`) |
|---|---|---|---|
| Parametreler | `$1` | `:1` | `:1` |
| Tanımlayıcılar | `"code"`, en fazla 63 karakter | `"CODE"`, en fazla 30 karakter | `"CODE"`, en fazla 30 karakter |
| Sayfalar | `LIMIT $n OFFSET $m` | `OFFSET :m ROWS FETCH NEXT :n ROWS ONLY` | sıralı sorgu üzerinde `ROWNUM` penceresi |
| Yeni anahtarlar | identity, `RETURNING` | identity, `RETURNING id INTO :n` | `<table>_seq.NEXTVAL` |
| Yazmadan sonra geri okuma | satırı `RETURNING` | anahtarla okuma | anahtarla okuma |
| Evet/hayır | `BOOLEAN` | `NUMBER(1)` | `NUMBER(1)` |
| Boş metin | bir değer | NULL | NULL |

`compat`, EF Core'un `UseOracleSQLCompatibility("11")` fikrini izler: URL seçmezse sunucu sürümü
seçer. Tanımlayıcılar her Oracle sürümünde 30 karakter içinde kalır, çünkü 12.1 de 11g ile aynı
sınıra sahiptir; daha uzun bir entity veya alan adı, onu adlandıran bir hatadır.

Oracle boş dizeyi NULL olarak saklar. Zeyn, gerekli bir `String` alanındaki NULL'ı `""` olarak okur,
metin sütunlarını null yapılabilir yazar ve metni null yapılabilir bir sütun gibi sıralar ve sayfalar;
böylece imleçler (cursor) kesin kalır. `Some("")` tutan bir `Option<String>` geri okunurken `None`
olur.

Oracle'ın sürücüsü bloklayıcıdır. Her çağrı, sınırlı havuzdan alınan bir bağlantıyla Tokio'nun
bloklayan thread'lerinde çalışır, bu yüzden async runtime'ı asla durdurmaz; aynı anda en fazla
`max_connections` çağrı çalışır ve diğerleri `acquire_timeout_ms` kadar bekler.

## Tablolar

Sütunlar alanların adını alır. Türler:

| Alan | PostgreSQL | Oracle |
|---|---|---|
| `#[key]` entity kimliği | `BIGINT GENERATED BY DEFAULT AS IDENTITY` | `NUMBER(19)` identity (12c+) veya sequence ile (11g) |
| `String` | `VARCHAR(max_len)` veya `TEXT` | `VARCHAR2(max_len CHAR)` veya `VARCHAR2(4000)`, null yapılabilir |
| `i32`, `i64` | `BIGINT` (`INTEGER` sütun da çalışır) | `NUMBER(19)` |
| `bool` | `BOOLEAN` | `NUMBER(1)` |
| `Money` | en küçük birimde `BIGINT` tutar, artı `<field>_currency VARCHAR(3)` | `NUMBER(19)`, artı `<field>_currency` |
| `Decimal` | alanın kurallarından `NUMERIC(precision, scale)`, kuralsız `NUMERIC(18, 2)` | `NUMBER(precision, scale)` |
| `Date` | `DATE` | `DATE` |
| `Time` | `TIME` | `HH:MM:SS` tutan `VARCHAR2(8)` |
| `DateTime` | `TIMESTAMPTZ(3)` | UTC tutan `TIMESTAMP(3)` |
| `LocalDateTime` | `TIMESTAMP` | `TIMESTAMP(0)` |
| `#[choice]` enum | varyant adını tutan `VARCHAR` | `VARCHAR2` |
| Özel alan türleri | JSON tutan `TEXT` | JSON tutan `CLOB` |
| `Option<T>` | aynı sütun, null yapılabilir | aynı sütun, null yapılabilir |

Para birimleri `TRY`, `USD` ve `EUR` olarak saklanır. Tarihler ve saatler ISO metni olarak taşınır ve
SQL onları sabit biçimlerle dönüştürür (`TO_DATE(:1, 'YYYY-MM-DD')`, `to_char(...)`); böylece
`NLS_DATE_FORMAT` veya PostgreSQL saat dilimi gibi oturum ayarları sonuçları değiştirmez. Decimal
değerler ondalık ayırıcı olmadan tam metin olarak bağlanır (`12345E-2`, `CAST(... AS NUMERIC)` ve
`TO_NUMBER(...)` ile okunur) ve `CAST(... AS TEXT)` veya
`TO_CHAR(..., 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''')` ile geri okunur; böylece
`NLS_NUMERIC_CHARACTERS` de onları değiştirmez.

`create_table::<E>(dialect)` (veya `db.create_table::<E>()`) iskele (scaffolding) ve testler için
`CREATE TABLE` yazar; her `#[rule(unique)]` alanı için `<table>_<field>_key` adlı bir kısıtla ve 11g'de
anahtar sequence'ıyla. Üretim şemaları migration'lara aittir. PostgreSQL'de:

```rust
db.migrate(&sqlx::migrate!("./migrations")).await?;
db.migrate_dir("./migrations").await?;
```

Birincisi migration'ları derleme zamanında gömer, ikincisi klasörü çalışma zamanında okur.
`sqlx::migrate!` uygulamanın kendi `Cargo.toml` dosyasında `macros` ve `migrate` feature'larıyla
`sqlx` ister; `migrate_dir` başka bir şey istemez. Oracle şemaları ekibin kendi aracıyla migrate edilir;
`create_table` başlangıç betiğini verir.

## Okuma

`db.find::<E>(&query)` tek bir bağlantıda en fazla üç ifade çalıştırır: sayfa, sayım ve toplamlar.
Filtreler `DataQuery::apply` ile aynı kod tarafından okunur, bu yüzden bir liste bellekteki veride ve
SQL'de aynı davranır:

| Filtre | SQL |
|---|---|
| `contains`, `starts_with` | metindeki `%` ve `_` escape edilerek `LOWER(col) LIKE $1 ESCAPE '\'` |
| `equals` | `LOWER(col) = $1` |
| sayılarda ve parada `from`, `to` | `col >= $1`, `col <= $2` |
| tarihlerde ve saatlerde `from`, `to` | `col >= CAST($1 AS date)`; bir an (moment) üzerindeki tarih sınırı kullanıcının saat diliminde tam günü kapsar |
| `any_of` | `col IN ($1, $2, ...)` |
| `is` | `col = $1` |
| `blank` | `col IS NULL`, `col IS NOT NULL` |

Para ve evet/hayır sütunlarında `contains` SQL'de yok sayılır. Metin veritabanı collation'ına göre
sıralanır (Oracle: `NLS_SORT`).

Her sıralama anahtarla biter, böylece sıra tamdır. NULL'lar bellekteki gibi artan sırada başta, azalan
sırada sonda gelir; `NULLS` yan tümcesi yalnızca null yapılabilir sütunlar için yazılır, böylece düz
indeksler diğer sıralamalara hizmet eder.

`get(key)` bir satırı okur ve hiç yoksa `Error::NotFound` ile başarısız olur. Herhangi bir tablo veya
view bir entity üzerinden okunabilir, örneğin Oracle'ın `user_sequences` görünümü.

## Elle yazılmış sorgular

Benzerliğe göre aramalar, join'ler ve `find`'ın ifade edemediği diğer sorgular `db.query::<M>(sql, args)` üzerinden geçer. `M` herhangi bir `#[model]` ya da entity'dir; alanları, `find` ile aynı dönüşümlerle, aynı adlı sütunlardan okunur. SQL bir string literal olmalıdır, bu yüzden değerler yalnızca parametre olarak gelebilir (PostgreSQL'de `$1`, `$2`, Oracle'da `:1`, `:2`); bir `String` ya da `format!` derlenmez.

```rust
#[model]
pub struct Hit {
    pub id: ItemId,
    pub slug: String,
    pub title: String,
}

#[action(public)]
pub async fn search_items(q: String, db: Db) -> Result<Vec<Hit>> {
    db.query(
        "SELECT i.id, i.slug, i.title, similarity(i.search_key, $1) AS score \
         FROM items i WHERE i.search_key % $1 \
         ORDER BY score DESC, i.id LIMIT 20",
        vec![zeyn::text::fold(&q).into()],
    )
    .await
}
```

`search_key`, uygulamanın satırı kaydederken yazdığı `zeyn::text::fold(&title)` değerini tutar; böylece anahtar ile katlanmış arama metni aynı biçimde karşılaştırılır (`İ` ve `ı` `i` olarak, aksansız, küçük harf). Örnek `CREATE EXTENSION pg_trgm` ve bir trigram indeksi gerektirir: `CREATE INDEX items_search_key ON items USING gin (search_key gin_trgm_ops)`. Modelde alanı olmayan sütunlar, örneğin `score`, veritabanı tarafından okunur ve atılır.

- `query_one` tek satırı ya da `None` döndürür; birden fazla satır hatadır.
- En fazla `QUERY_ROW_CAP` (1000) satır okunur. İfade `SELECT <fields> FROM (<sql>) zeyn_q LIMIT 1001` olarak sarılır; böylece sınırı aşan bir satır, sessizce büyüyen bir liste yerine bunu söyleyen bir hatadır; `query_max(sql, args, n)` başka bir sınır belirler.
- Sütunu olmayan bir alan ya da türü yanlış bir sütun hatadır.
- Havuzun ifade zaman aşımı geçerlidir. `Tx` aynı üç çağrıya sahiptir.

## İmleçler (cursor)

Numaralı sayfalar offset kullanır. Sonsuz ve sanal kaydırma keyset imleçleri kullanır; bunlar bir
listenin derinlerinde yavaşlamaz ve başkaları satır eklediğinde satır atlamaz veya tekrarlamaz:

```sql
WHERE "code" <= $1 AND (("code" < $2) OR ("code" = $3 AND "id" > $4))
ORDER BY "code" DESC, "id" ASC LIMIT $5
```

Bir sayfa, devamının olup olmadığını bilmek için istenenden bir satır fazla getirir. `Page::next` o
zaman bir imleç taşır: sıralama alanlarını, yönlerini ve son satırın her sıralama anahtarındaki değerini
(anahtar en sonda) içeren base64url JSON. İmleç istemciler için opaktır ama gizli değildir ve sunucu ona
asla güvenmez. Bir boyut sınırıyla çözülür, sırası sorgunun sırasından farklıysa reddedilir ve her değer
bağlanmadan önce sütun türüne göre denetlenir. Kötü bir imleç bir doğrulama hatasıdır, sorgu değildir.
Yalnızca nereden devam edileceğini söyler; filtreler ve izinler yeniden uygulanır.

Gruplanmış sayfalar imleçleri için offset kullanır.

## Toplamlar ve sayımlar

`with_total` en fazla `database.count_cap` satır sayar (sınırlanmış sorgu üzerinde bir `COUNT(*)`),
böylece devasa bir tablodaki toplam ucuz kalır; sayım sınıra ulaştığında `Page::total` sınırdır.

Sütun toplamları, filtrelerle eşleşen her satır üzerinde tek bir toplama ifadesidir. Bellekteki
kuralları izlerler: boş bir toplam 0'dır, boş bir ortalama boştur, para ortalamaları yarısı sıfırdan
uzağa yuvarlanır ve birden çok para biriminde para için toplam yoktur.

`DataQuery::group`, grup başına sayım ve istenen toplamlarla `GROUP BY` haline gelir.

## Yazma

- `insert(&row)` yeni bir anahtarı (0) veritabanına bırakır, satır sürümünü 1 yapar ve saklanan satırı
  döndürür.
- `update(&row)` `UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2` çalıştırır.
  Sıfır satır, başka birinin önce kaydettiği (veya satırı sildiği) anlamına gelir: `Error::Conflict`.
- `delete::<E>(key)` hiçbir şey silinmediğinde `Error::NotFound` ile başarısız olur.
- `#[readonly]` alanlar bir modelden asla yazılmaz, bu yüzden sahte bir form onları değiştiremez.
  Sunucu kodu onları kendi SQL'iyle değiştirir: `db.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", args)`.
  `execute` bir dize sabiti (string literal) alır, bu yüzden değerler yalnızca parametre olarak gelebilir.

Bir benzersizlik ihlali (PostgreSQL `23505`, Oracle `ORA-00001`) alanda bir doğrulama hatasına
dönüşür: adı kısıt adında geçen `#[rule(unique)]` alan veya tek benzersiz alan. Form onu diğer her
kural gibi girişin altında gösterir. Kısıtları `<table>_<column>_key` olarak adlandırın.

## İşlemler (transaction)

```rust
let mut tx = db.begin().await?;
let order = tx.insert(&order).await?;
tx.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", vec![qty.into(), id.0.into()]).await?;
tx.commit().await?;
```

`Tx`, tek bir bağlantıda `Db` ile aynı çağrılara sahiptir. `commit` olmadan bırakılırsa geri alınır
(rollback). `Db` üzerindeki çağrılar tek tek commit edilir.

## Akışlar (stream)

`db.stream::<E>(&query)` eşleşen her satırı sayfalama olmadan sırayla döndürür, dışa aktarmalar için.
Bir görev satırları `STREAM_BUFFER` (64) satırlık bir kanala okur ve okuyucu geride kaldığında bekler;
böylece bellek her sonuç boyutunda sabit kalır. Akışı bırakmak sorguyu durdurur ve bağlantıyı serbest
bırakır. `RowStream`, `futures_core::Stream` uygular.

## Güvenlik: SQL injection

- Tanımlayıcılar (tablo ve sütun adları) yalnızca entity'den gelir: `E::TABLE` ve `FieldMeta`
  listesi. Bir istemcinin filtrelerde ve sıralamalarda gönderdiği adlar bu listede aranır ve orada
  yoksa düşürülür. Her tanımlayıcı denetlenir (harfler, rakamlar, `_`, en fazla lehçenin uzunluğu) ve
  tırnak içine alınır.
- Değerler her zaman bağlı parametrelerdir; `LIKE` kalıpları, imleç değerleri, limitler ve offset'ler
  dahil. Hiçbir değer asla SQL metnine yazılmaz.
- Elle yazılmış SQL `execute(&'static str, args)` ve `query(&'static str, args)` üzerinden geçer: ifade bir sabit olmalıdır.
- Limitler sunucuda sıkıştırılır (`MAX_PAGE_SIZE`, `MAX_FILTERS`, `MAX_SORT`, filtre metni uzunluğu,
  `any_of` boyutu), istemci ne isterse istesin.
- Sürücü hataları `Error::Internal` olarak günlüğe yazılır ve kullanıcılara asla gösterilmez. Bağlantı
  URL'si bir `Secret` olarak okunur ve asla günlüğe yazılmaz.

## Performans

- İfadeler bağlantı başına hazırlanır ve önbelleğe alınır (sqlx'in ifade önbelleği, Oracle'ın 64'lük
  ifade önbelleği).
- Satırlar birkaç düz hücre türüne okunur ve bir serde deserializer ile modele taşınır: ara bir map
  yoktur ve metnin kopyaları yoktur.
- Havuzlar sınırlıdır ve her `find`, iki veya üç ifadesi için tek bir bağlantı kullanır.
- Oracle her gidiş dönüşte 128 satır getirir. Bir sayfa (en fazla 501 satır) bloklayan bir thread'de
  okunur ve hemen teslim edilir; akışlar satır satır gider.
- Akışlar en fazla `STREAM_BUFFER` satır tutar.

## Testler

SQL oluşturucunun her iki lehçe için veritabanı gerektirmeyen birim testleri vardır. Entegrasyon
testleri aynı satırlar üzerinde `Db::find` ile `DataQuery::apply` sonuçlarını karşılaştırır, her keyset
imlecini gezer ve kayıtları, çakışmaları, benzersizlik hatalarını, işlemleri ve akışları denetler.
Değişkenleri bir veya daha fazla URL (boşlukla ayrılmış) tuttuğunda çalışırlar, aksi halde atlandıklarını
söylerler:

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test postgres

docker run -d --name zeyn-ora11 -p 15211:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:11-slim
docker run -d --name zeyn-ora21 -p 15221:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:21-slim
ZEYN_TEST_ORACLE="oracle://zeyn:zeyn@127.0.0.1:15211/XE oracle://zeyn:zeyn@127.0.0.1:15221/XEPDB1" cargo test -p zeyn --features oracle --test oracle
```

Oracle container'larının başlaması birkaç dakika sürer; `docker logs` içinde `DATABASE IS READY TO USE!`
satırını bekleyin.
