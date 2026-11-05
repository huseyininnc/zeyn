# Datos y bases de datos

Zeyn no esconde la base de datos tras un mapeador de objetos. Los datos viven en servicios (stores) que la aplicación registra; las acciones los reciben como argumentos. Un store puede guardar filas en memoria, llamar a una base de datos o llamar a otro servicio. Las pantallas, las tablas y la API solo ven acciones.

## Un store

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

Regístralo una vez con `App::service(store)`. La aplicación se niega a arrancar cuando una pantalla o una API necesita un store que no está registrado, y dice cuál.

## Listas: DataQuery y Page

Una acción de lista recibe un `DataQuery` y devuelve `Page<M>`:

| Campo | Significado |
|---|---|
| `page` | `Offset { offset, limit }` para páginas numeradas, o `After { cursor, limit }` para desplazamiento infinito |
| `sort` | Campos y direcciones, en orden |
| `filters` | Campo, operador y valor, tipados por la columna |
| `with_total` | Si el cliente necesita el recuento total |
| `aggregates` | Totales de columna que calcular sobre todas las filas filtradas |
| `time_zone` | Zona para los filtros de fecha en columnas `DateTime`, fijada por el servidor |
| `group` | Agrupa las filas por un campo, fijado por la tabla |

`DataQuery::apply(vec)` lo hace todo en memoria. Para una base de datos SQL usa `zeyn::sql`: `Db::find` convierte la misma consulta en `WHERE`, `ORDER BY`, `LIMIT` y `GROUP BY` para PostgreSQL u Oracle. Solo se pueden filtrar u ordenar los campos propios de la entidad, cada valor se enlaza como parámetro y el servidor limita el tamaño de página, de modo que la entrada del cliente nunca llega al texto SQL. Escribe SQL a mano solo para las consultas que `Db::find` no puede expresar: `Db::query::<M>(sql, args)` lee las filas de un `SELECT` en un modelo, acepta el SQL solo como literal de cadena y cada valor solo como parámetro, y lee como máximo 1000 filas.

```rust
let hits: Vec<Hit> = db
    .query("SELECT id, title FROM items WHERE title % $1 ORDER BY similarity(title, $1) DESC LIMIT 20", vec![q.into()])
    .await?;
```

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

La [referencia de la base de datos](/reference/database) cubre los ajustes de conexión, Oracle, migraciones, transacciones, streams y el SQL en que se convierte cada consulta.

`Page` lleva las filas, el total cuando se pide, el cursor del siguiente lote y los totales:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## Versiones de fila

Un campo `#[version] pub version: i64` activa el bloqueo optimista. Los formularios envían la versión que cargaron; el store la compara y devuelve `Error::Conflict` cuando alguien guardó entretanto. Entonces el formulario avisa al usuario en lugar de sobrescribir el cambio de su compañero.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## Validación en el store

Las reglas que necesitan los datos, como `unique`, se comprueban en el store y se devuelven como errores de campo:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Tenants

Con `App::tenancy(resolver)` cada petición lleva un tenant (`ctx.tenant()`), tomado del nombre de host (`SubdomainTenant`) o de una cabecera (`HeaderTenant`), o el tenant propio del usuario cuando la petición no nombra ninguno. Un usuario con sesión iniciada debe pertenecer al tenant de la petición (`Identity::member_of`); de lo contrario la petición se ejecuta sin sesión. Un store lo usa para elegir un esquema o filtrar filas; `TenantRequired` rechaza las peticiones que no tienen uno.

## Estado de usuario guardado

Las vistas de tabla (columnas, orden, filtros), los diseños de dock y el tema elegido se pueden guardar por usuario en el servidor con `App::datagrid_views(store)`. `MemoryViewStore` los guarda en memoria para desarrollo; en producción una aplicación implementa `DataGridViewStore` con su base de datos.

## Trabajo en segundo plano

Las importaciones, las exportaciones y otros trabajos largos se ejecutan como jobs en segundo plano con progreso (`Jobs`), de modo que una petición nunca los espera. Consulta [Auditoría, enmascarado y jobs](enterprise.md).

Los jobs viven en memoria salvo que la aplicación les dé un store. Con `SqlJobStore`, las ejecuciones en cola y la siguiente ejecución de cada programación se guardan en la base de datos: sobreviven a un reinicio, varios servidores comparten una cola y un lease garantiza que dos workers nunca ejecuten la misma ejecución.

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

- `jobs.enqueue(name, payload)` pone una ejecución en cola; `enqueue_after` la retrasa. El payload es texto de como máximo 4000 bytes, como un id o un JSON pequeño.
- Un worker reclama una ejecución pendiente con un lease de un minuto y lo renueva mientras la ejecución trabaja. Si el servidor se detiene, la ejecución se reclama de nuevo cuando termina el lease; ese intento cuenta.
- Un intento fallido espera el backoff, que se duplica hasta diez minutos, hasta que se agotan los reintentos. Entonces la ejecución permanece en la tabla como `failed` con su código de error. Una programación nunca falla para siempre: pasa a su siguiente intervalo.
- PostgreSQL reclama con `FOR UPDATE SKIP LOCKED`, Oracle (11g y posteriores) de la misma manera mediante un select de ids. La tabla es `zeyn_jobs`; `create_job_tables(dialect, prefix)` da las sentencias para una migración.
- El store en memoria (`MemoryJobStore`) guarda como máximo 10.000 ejecuciones.
