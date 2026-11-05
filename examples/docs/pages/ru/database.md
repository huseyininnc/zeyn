# Данные и базы данных

Zeyn не прячет базу данных за объектным маппером. Данные живут в сервисах (хранилищах, stores), которые регистрирует приложение; действия получают их как аргументы. Хранилище может держать строки в памяти, обращаться к базе данных или вызывать другой сервис. Экраны, таблицы и API видят только действия.

## Хранилище

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

Зарегистрируйте его один раз через `App::service(store)`. Приложение отказывается запускаться, если экрану или API нужно хранилище, которое не зарегистрировано, и сообщает, какое именно.

## Списки: DataQuery и Page

Действие-список принимает `DataQuery` и возвращает `Page<M>`:

| Поле | Значение |
|---|---|
| `page` | `Offset { offset, limit }` для нумерованных страниц или `After { cursor, limit }` для бесконечной прокрутки |
| `sort` | Поля и направления по порядку |
| `filters` | Поле, оператор и значение, типизированные по столбцу |
| `with_total` | Нужно ли клиенту общее количество |
| `aggregates` | Итоги столбцов, вычисляемые по всем отфильтрованным строкам |
| `time_zone` | Часовой пояс для фильтров дат в столбцах `DateTime`, задаётся сервером |
| `group` | Группировка строк по полю, задаётся таблицей |

`DataQuery::apply(vec)` выполняет всё это в памяти. Для SQL-базы данных используйте `zeyn::sql`: `Db::find` превращает тот же запрос в `WHERE`, `ORDER BY`, `LIMIT` и `GROUP BY` для PostgreSQL или Oracle. Фильтровать и сортировать можно только по собственным полям сущности, каждое значение передаётся как параметр, а размер страницы ограничивает сервер, поэтому ввод клиента никогда не попадает в текст SQL. Пишите SQL вручную только для запросов, которые `Db::find` выразить не может: `Db::query::<M>(sql, args)` читает строки `SELECT` в модель, принимает SQL только как строковый литерал, каждое значение только как параметр и читает не более 1000 строк.

```rust
let hits: Vec<Hit> = db
    .query("SELECT id, title FROM items WHERE title % $1 ORDER BY similarity(title, $1) DESC LIMIT 20", vec![q.into()])
    .await?;
```

```toml
zeyn = { version = "0.0.8", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

[Справочник по базе данных](/reference/database) описывает параметры подключения, Oracle, миграции, транзакции, потоки и SQL, в который превращается каждый запрос.

`Page` несёт строки, общее количество по запросу, курсор следующей порции и итоги:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## Версии строк

Поле `#[version] pub version: i64` включает оптимистичную блокировку. Формы отправляют загруженную ими версию; хранилище сравнивает её и возвращает `Error::Conflict`, если кто-то сохранил запись в промежутке. Тогда форма сообщает об этом пользователю, а не перезаписывает изменение коллеги.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## Валидация в хранилище

Правила, которым нужны данные, например `unique`, проверяются в хранилище и возвращаются как ошибки полей:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Арендаторы (Tenants)

С `App::tenancy(resolver)` каждый запрос несёт арендатора (`ctx.tenant()`): из имени хоста (`SubdomainTenant`), из заголовка (`HeaderTenant`) или собственного арендатора пользователя, если запрос его не называет. Вошедший пользователь должен принадлежать арендатору запроса (`Identity::member_of`), иначе запрос выполняется без входа. Хранилище использует его, чтобы выбрать схему или отфильтровать строки; `TenantRequired` отклоняет запросы без арендатора.

## Сохранённое состояние пользователя

Представления таблиц (столбцы, сортировка, фильтры), раскладки док-панелей и выбранную тему можно хранить для каждого пользователя на сервере через `App::datagrid_views(store)`. `MemoryViewStore` хранит их в памяти для разработки; для продакшена приложение реализует `DataGridViewStore` поверх своей базы данных.

## Фоновая работа

Импорт, экспорт и другая длительная работа выполняются как фоновые задания с отображением прогресса (`Jobs`), поэтому запрос никогда их не ждёт. См. [Аудит, маскирование и задания](enterprise.md).

Задания живут в памяти, пока приложение не даст им хранилище. С `SqlJobStore` запуски в очереди и следующий запуск каждого расписания хранятся в базе данных: они переживают перезапуск, несколько серверов делят одну очередь, а аренда (lease) гарантирует, что два исполнителя никогда не запустят один и тот же запуск.

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

- `jobs.enqueue(name, payload)` ставит запуск в очередь; `enqueue_after` откладывает его. Полезная нагрузка (payload) это текст не длиннее 4000 байт, например идентификатор или небольшой JSON.
- Исполнитель захватывает подошедший по времени запуск с арендой в одну минуту и продлевает её, пока запуск работает. Если сервер останавливается, запуск захватывается снова после окончания аренды; эта попытка засчитывается.
- Неудачная попытка ждёт задержку (backoff), которая удваивается до десяти минут, пока не исчерпаны повторы. Затем запуск остаётся в таблице со статусом `failed` и кодом ошибки. Расписание никогда не отказывает окончательно: оно переходит к следующему интервалу.
- PostgreSQL захватывает через `FOR UPDATE SKIP LOCKED`, Oracle (11g и новее) так же, через выборку идентификаторов. Таблица называется `zeyn_jobs`; `create_job_tables(dialect, prefix)` даёт операторы для миграции.
- Хранилище в памяти (`MemoryJobStore`) вмещает не более 10 000 запусков.
