# Base de datos

`zeyn::sql` guarda las entities en una base de datos SQL. Las lista a partir del `DataQuery` que envía un grid
(filtros, ordenación, páginas, totales, grupos) y las guarda con comprobaciones de versión de fila. PostgreSQL está
detrás de la feature de Cargo `postgres` y Oracle detrás de `oracle`, de modo que una aplicación sin base de datos
no compila nada de esto.

```toml
zeyn = { version = "0.0.8", features = ["postgres"] }   # or ["oracle"], or both
```

## Configuración

Registra un `Db` como servicio y recíbelo en las actions:

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

`Db::from_config` lee `database.url` como secreto y los límites del pool desde `database.*`:

| Clave | Valor por defecto | Significado |
|---|---|---|
| `database.url` | obligatoria | `postgres://user:password@host:5432/db` o `oracle://user:password@host:1521/service` |
| `database.max_connections` | 10 | Tamaño del pool; las peticiones esperan una conexión libre |
| `database.min_connections` | 0 | Conexiones que se mantienen abiertas en reposo |
| `database.acquire_timeout_ms` | 5000 | Espera máxima por una conexión |
| `database.idle_timeout_s` | 600 | Las conexiones inactivas por encima del mínimo se cierran pasado este tiempo |
| `database.statement_timeout_ms` | 30000 | Tiempo máximo que puede ejecutarse una sentencia; 0 mantiene el valor predeterminado del servidor |
| `database.count_cap` | 1000000 | Mayor total contado para una página |

`Db::connect(url, PoolLimits { .. })` hace lo mismo sin `Config`. El esquema de la URL elige el
driver. Los caracteres como `@` o `:` en una contraseña de Oracle se escriben codificados con porcentaje (`%40`).

Las conexiones de PostgreSQL se abren de forma diferida, así que la aplicación arranca aunque la base de datos esté caída y la primera
consulta informa del problema. Oracle pregunta al servidor su versión al conectarse, salvo que la
URL indique qué SQL escribir (`?compat=11` o `?compat=12`); con `compat` definido, arranca también sin
la base de datos.

El ejemplo `inventory` guarda sus productos en memoria, o en la base de datos que indique `ZEYN_DATABASE_URL`
(compílalo con `--features postgres` o `--features oracle`):

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_DATABASE_URL=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo run -p zeyn-example-inventory --features postgres
```

## Oracle Instant Client

El driver de Oracle es el crate `oracle` sobre ODPI-C. Compila sin software de Oracle, pero en
tiempo de ejecución carga la biblioteca de cliente de Oracle (`oci.dll`, `libclntsh.so`). Oracle Instant Client es
de uso gratuito:

1. Descarga Instant Client Basic Light para tu plataforma desde la página de Instant Client de Oracle. En
   Windows x64 toma el zip `instantclient-basiclite-windows.x64-19.*.zip`. La versión 19 se conecta a
   Oracle Database 11.2 hasta 23ai; los clientes más nuevos ya no se conectan a 11g.
2. Descomprímelo en una carpeta propia, por ejemplo `C:\oracle\instantclient_19`.
3. Pon esa carpeta primero en `PATH`, o define `ZEYN_ORACLE_CLIENT_DIR` apuntando a ella. La variable tiene prioridad sobre
   `PATH`, lo que ayuda cuando hay otro Oracle home instalado. En Linux, instala el RPM o define
   `LD_LIBRARY_PATH`; Basic Light también necesita `libaio`.

Sin la biblioteca, la primera conexión falla con `DPI-1047: Cannot locate a 64-bit Oracle
Client library`.

## Dialectos

El constructor de SQL es compartido. Un `Dialect` aporta solo lo que difiere, y `Postgres` y
`Oracle { compat }` lo implementan:

| | PostgreSQL | Oracle 12c y posteriores (`V12`) | Oracle 11g (`V11`) |
|---|---|---|---|
| Parámetros | `$1` | `:1` | `:1` |
| Identificadores | `"code"`, hasta 63 caracteres | `"CODE"`, hasta 30 caracteres | `"CODE"`, hasta 30 caracteres |
| Páginas | `LIMIT $n OFFSET $m` | `OFFSET :m ROWS FETCH NEXT :n ROWS ONLY` | ventana `ROWNUM` sobre la consulta ordenada |
| Claves nuevas | identity, `RETURNING` | identity, `RETURNING id INTO :n` | `<table>_seq.NEXTVAL` |
| Lectura posterior a una escritura | `RETURNING` de la fila | lectura por clave | lectura por clave |
| Sí/no | `BOOLEAN` | `NUMBER(1)` | `NUMBER(1)` |
| Texto vacío | un valor | NULL | NULL |

`compat` sigue la idea de `UseOracleSQLCompatibility("11")` de EF Core: la versión del servidor
lo elige cuando la URL no lo hace. Los identificadores se mantienen dentro de 30 caracteres en todas las versiones de Oracle
porque 12.1 tiene el mismo límite que 11g; un nombre de entity o de campo más largo es un error que lo nombra.

Oracle almacena la cadena vacía como NULL. Zeyn lee un NULL en un campo `String` obligatorio como `""`,
escribe las columnas de texto como anulables, y ordena y pagina el texto como una columna anulable, de modo que los cursores se mantienen
exactos. Un `Option<String>` que contiene `Some("")` se lee de vuelta como `None`.

El driver de Oracle es bloqueante. Cada llamada se ejecuta en los hilos de bloqueo de Tokio con una conexión tomada
del pool acotado, de modo que nunca detiene el runtime asíncrono; como máximo `max_connections` llamadas se ejecutan
a la vez y las demás esperan hasta `acquire_timeout_ms`.

## Tablas

Las columnas llevan el nombre de los campos. Tipos:

| Campo | PostgreSQL | Oracle |
|---|---|---|
| id de entity con `#[key]` | `BIGINT GENERATED BY DEFAULT AS IDENTITY` | `NUMBER(19)` identity (12c+) o con una secuencia (11g) |
| `String` | `VARCHAR(max_len)` o `TEXT` | `VARCHAR2(max_len CHAR)` o `VARCHAR2(4000)`, anulable |
| `i32`, `i64` | `BIGINT` (una columna `INTEGER` también funciona) | `NUMBER(19)` |
| `bool` | `BOOLEAN` | `NUMBER(1)` |
| `Money` | importe `BIGINT` en la unidad más pequeña, más `<field>_currency VARCHAR(3)` | `NUMBER(19)`, más `<field>_currency` |
| `Decimal` | `NUMERIC(precision, scale)` según las reglas del campo, `NUMERIC(18, 2)` sin ellas | `NUMBER(precision, scale)` |
| `Date` | `DATE` | `DATE` |
| `Time` | `TIME` | `VARCHAR2(8)` con `HH:MM:SS` |
| `DateTime` | `TIMESTAMPTZ(3)` | `TIMESTAMP(3)` en UTC |
| `LocalDateTime` | `TIMESTAMP` | `TIMESTAMP(0)` |
| enum `#[choice]` | `VARCHAR` con el nombre de la variante | `VARCHAR2` |
| Tipos de campo personalizados | `TEXT` con JSON | `CLOB` con JSON |
| `Option<T>` | la misma columna, anulable | la misma columna, anulable |

Las monedas se almacenan como `TRY`, `USD` y `EUR`. Las fechas y las horas viajan como texto ISO y el SQL
las convierte con formatos fijos (`TO_DATE(:1, 'YYYY-MM-DD')`, `to_char(...)`), de modo que los ajustes
de sesión como `NLS_DATE_FORMAT` o la zona horaria de PostgreSQL no cambian los resultados. Los decimales
se enlazan como texto exacto sin separador decimal (`12345E-2`, leído por `CAST(... AS NUMERIC)`
y `TO_NUMBER(...)`) y se leen de vuelta con `CAST(... AS TEXT)` o
`TO_CHAR(..., 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''')`, de modo que `NLS_NUMERIC_CHARACTERS` tampoco
los cambia.

`create_table::<E>(dialect)` (o `db.create_table::<E>()`) escribe el `CREATE TABLE` para
andamiaje y pruebas, con una restricción llamada `<table>_<field>_key` para cada
campo `#[rule(unique)]` y, en 11g, la secuencia de la clave. Los esquemas de producción pertenecen a las migraciones.
En PostgreSQL:

```rust
db.migrate(&sqlx::migrate!("./migrations")).await?;
db.migrate_dir("./migrations").await?;
```

La primera incrusta las migraciones en tiempo de compilación, la segunda lee la carpeta en tiempo de ejecución.
`sqlx::migrate!` necesita `sqlx` con sus features `macros` y `migrate` en el propio
`Cargo.toml` de la aplicación; `migrate_dir` no necesita nada más. Los esquemas de Oracle se migran con la
herramienta propia del equipo; `create_table` da el script inicial.

## Lectura

`db.find::<E>(&query)` ejecuta como máximo tres sentencias en una conexión: la página, el recuento
y los totales. Los filtros los lee el mismo código que `DataQuery::apply`, de modo que una lista se comporta
igual con datos en memoria y en SQL:

| Filtro | SQL |
|---|---|
| `contains`, `starts_with` | `LOWER(col) LIKE $1 ESCAPE '\'` con `%` y `_` del texto escapados |
| `equals` | `LOWER(col) = $1` |
| `from`, `to` en números y dinero | `col >= $1`, `col <= $2` |
| `from`, `to` en fechas y horas | `col >= CAST($1 AS date)`; un límite de fecha sobre un instante cubre todo el día en la zona horaria del usuario |
| `any_of` | `col IN ($1, $2, ...)` |
| `is` | `col = $1` |
| `blank` | `col IS NULL`, `col IS NOT NULL` |

`contains` sobre columnas de dinero y de sí/no se ignora en SQL. El texto se ordena según la collation de la base de datos
(Oracle: `NLS_SORT`).

Toda ordenación termina con la clave, de modo que el orden es total. Los NULL van primero al subir y al final al
bajar, como en memoria; la cláusula `NULLS` se escribe solo para columnas anulables, de modo que los índices simples
sirven para las demás ordenaciones.

`get(key)` lee una fila y falla con `Error::NotFound` cuando no hay ninguna. Cualquier tabla o vista
puede leerse a través de una entity, por ejemplo `user_sequences` de Oracle.

## Consultas escritas a mano

Las búsquedas por similitud, los joins y otras consultas que `find` no puede expresar pasan por `db.query::<M>(sql, args)`. `M` es cualquier `#[model]` o entity; sus campos se leen de las columnas del mismo nombre, con las mismas conversiones que `find`. El SQL debe ser un literal de cadena, así que los valores solo pueden llegar como parámetros (`$1`, `$2` en PostgreSQL, `:1`, `:2` en Oracle); un `String` o un `format!` no compila.

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

`search_key` contiene `zeyn::text::fold(&title)`, que la app escribe al guardar la fila, de modo que la clave y el texto de búsqueda plegado se comparan en la misma forma (`İ` y `ı` como `i`, sin acentos, en minúsculas). El ejemplo necesita `CREATE EXTENSION pg_trgm` y un índice de trigramas: `CREATE INDEX items_search_key ON items USING gin (search_key gin_trgm_ops)`. Las columnas para las que el modelo no tiene campo, como `score`, las lee la base de datos y se descartan.

- `query_one` devuelve la única fila o `None`; más de una fila es un error.
- Se leen como máximo `QUERY_ROW_CAP` (1000) filas. La sentencia se envuelve como `SELECT <fields> FROM (<sql>) zeyn_q LIMIT 1001`, de modo que una fila por encima del límite es un error que lo dice, en lugar de una lista que crece en silencio; `query_max(sql, args, n)` fija otro límite.
- Un campo sin columna, o una columna de tipo incorrecto, es un error.
- Se aplica el tiempo de espera de sentencias del pool. `Tx` tiene las mismas tres llamadas.

## Cursores

Las páginas numeradas usan offsets. El scroll infinito y el virtual usan cursores keyset, que no se ralentizan
al profundizar en una lista y no omiten ni repiten filas cuando se insertan otras:

```sql
WHERE "code" <= $1 AND (("code" < $2) OR ("code" = $3 AND "id" > $4))
ORDER BY "code" DESC, "id" ASC LIMIT $5
```

Una página obtiene una fila más de las pedidas para saber si siguen más. `Page::next` entonces lleva un
cursor: JSON en base64url con los campos de ordenación, sus direcciones y el valor de la última fila para cada
clave de ordenación (la clave al final). El cursor es opaco para los clientes pero no secreto, y el servidor nunca
confía en él. Se decodifica con un límite de tamaño, se rechaza cuando su orden difiere del de la consulta, y
cada valor se comprueba contra el tipo de su columna antes de enlazarlo. Un cursor incorrecto es un error de
validación, no una consulta. Solo dice dónde continuar; los filtros y los permisos se aplican de nuevo.

Las páginas agrupadas usan offsets para sus cursores.

## Totales y recuentos

`with_total` cuenta como máximo `database.count_cap` filas (un `COUNT(*)` sobre la consulta acotada), de modo que un
total sobre una tabla enorme sigue siendo barato; cuando el recuento alcanza el tope, `Page::total` es el tope.

Los totales de columna son una sola sentencia de agregación sobre todas las filas que cumplen los filtros. Siguen
las reglas en memoria: una suma vacía es 0, un promedio vacío es vacío, los promedios de dinero se redondean con la mitad alejándose
de cero, y el dinero en más de una moneda no tiene total.

`DataQuery::group` se convierte en `GROUP BY` con el recuento y los totales pedidos por grupo.

## Escritura

- `insert(&row)` deja una clave nueva (0) a cargo de la base de datos, establece la versión de la fila en 1 y devuelve la
  fila almacenada.
- `update(&row)` ejecuta `UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2`.
  Cero filas significa que otra persona guardó primero (o eliminó la fila): `Error::Conflict`.
- `delete::<E>(key)` falla con `Error::NotFound` cuando no se eliminó nada.
- Los campos `#[readonly]` nunca se escriben desde un modelo, de modo que un formulario falsificado no puede cambiarlos. El código
  del servidor los cambia con SQL propio: `db.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", args)`.
  `execute` recibe un literal de cadena, de modo que los valores solo pueden llegar como parámetros.

Una violación de unicidad (PostgreSQL `23505`, Oracle `ORA-00001`) se convierte en un error de validación sobre el
campo: el campo `#[rule(unique)]` cuyo nombre está en el nombre de la restricción, o el único campo unique.
El formulario lo muestra bajo la entrada como cualquier otra regla. Nombra las restricciones `<table>_<column>_key`.

## Transacciones

```rust
let mut tx = db.begin().await?;
let order = tx.insert(&order).await?;
tx.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", vec![qty.into(), id.0.into()]).await?;
tx.commit().await?;
```

`Tx` tiene las mismas llamadas que `Db` sobre una conexión. Si se descarta sin `commit`, se revierte.
Las llamadas sobre el propio `Db` confirman una por una.

## Streams

`db.stream::<E>(&query)` devuelve todas las filas coincidentes en orden sin paginar, para exportaciones. Una tarea
lee filas hacia un canal de `STREAM_BUFFER` (64) filas y espera mientras el lector va por detrás, de modo que la
memoria se mantiene plana para cualquier tamaño de resultado. Descartar el stream detiene la consulta y libera la
conexión. `RowStream` implementa `futures_core::Stream`.

## Seguridad: inyección SQL

- Los identificadores (nombres de tablas y columnas) provienen solo de la entity: `E::TABLE` y su
  lista `FieldMeta`. Los nombres que un cliente envía en filtros y ordenaciones se buscan en esa lista y
  se descartan cuando no están. Cada identificador se comprueba (letras, dígitos, `_`, como máximo la
  longitud del dialecto) y se entrecomilla.
- Los valores son siempre parámetros enlazados, incluidos los patrones `LIKE`, los valores de cursor, los límites y
  los offsets. Ningún valor se escribe nunca en el texto SQL.
- El SQL escrito a mano pasa por `execute(&'static str, args)` y `query(&'static str, args)`:
  la sentencia debe ser un literal.
- Los límites se acotan en el servidor (`MAX_PAGE_SIZE`, `MAX_FILTERS`, `MAX_SORT`, longitud del texto del filtro,
  tamaño de `any_of`), pida lo que pida el cliente.
- Los errores del driver se registran como `Error::Internal` y nunca se muestran a los usuarios. La URL de conexión se
  lee como un `Secret` y nunca se registra en el log.

## Rendimiento

- Las sentencias se preparan y se guardan en caché por conexión (la caché de sentencias de sqlx, la caché de sentencias
  de Oracle de 64).
- Las filas se leen en unos pocos tipos de celda simples y se mueven al modelo con un deserializador serde:
  sin mapa intermedio y sin copias del texto.
- Los pools están acotados y cada `find` usa una conexión para sus dos o tres sentencias.
- Oracle obtiene 128 filas por viaje de ida y vuelta. Una página (como máximo 501 filas) se lee en un hilo de bloqueo
  y se entrega de una vez; los streams van fila por fila.
- Los streams retienen como máximo `STREAM_BUFFER` filas.

## Pruebas

El constructor de SQL tiene pruebas unitarias para ambos dialectos que no necesitan una base de datos. Las pruebas de integración
comparan `Db::find` con `DataQuery::apply` sobre las mismas filas, recorren cada cursor keyset y
comprueban guardados, conflictos, errores de unicidad, transacciones y streams. Se ejecutan cuando su variable
contiene una o más URL (separadas por espacios) y, en caso contrario, indican que se omitieron:

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test postgres

docker run -d --name zeyn-ora11 -p 15211:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:11-slim
docker run -d --name zeyn-ora21 -p 15221:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:21-slim
ZEYN_TEST_ORACLE="oracle://zeyn:zeyn@127.0.0.1:15211/XE oracle://zeyn:zeyn@127.0.0.1:15221/XEPDB1" cargo test -p zeyn --features oracle --test oracle
```

Los contenedores de Oracle tardan unos minutos en arrancar; espera a ver `DATABASE IS READY TO USE!` en
`docker logs`.
