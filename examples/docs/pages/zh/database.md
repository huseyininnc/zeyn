# 数据与数据库

Zeyn 不会把数据库藏在对象映射器后面。数据存放在应用注册的服务（store）中；action 通过参数接收它们。store 可以把行保存在内存中、调用数据库，或调用另一个服务。screen、grid 和 API 只看得到 action。

## Store

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

用 `App::service(store)` 注册一次。当某个 screen 或 API 需要的 store 没有注册时，应用会拒绝启动，并说明是哪一个。

## 列表：DataQuery 与 Page

列表 action 接收 `DataQuery` 并返回 `Page<M>`：

| 字段 | 含义 |
|---|---|
| `page` | `Offset { offset, limit }` 用于编号分页，`After { cursor, limit }` 用于无限滚动 |
| `sort` | 字段及方向，按顺序排列 |
| `filters` | 字段、运算符和值，类型由列决定 |
| `with_total` | 客户端是否需要总数 |
| `aggregates` | 对所有已过滤的行计算的列合计 |
| `time_zone` | `DateTime` 列上日期过滤所用的时区，由服务器设置 |
| `group` | 按某个字段对行分组，由 grid 设置 |

`DataQuery::apply(vec)` 在内存中完成全部处理。对于 SQL 数据库，请使用 `zeyn::sql`：`Db::find` 会把同样的查询转换成适用于 PostgreSQL 或 Oracle 的 `WHERE`、`ORDER BY`、`LIMIT` 和 `GROUP BY`。只有实体自己的字段可以被过滤或排序，每个值都作为参数绑定，页面大小由服务器限定上限，因此客户端输入永远不会进入 SQL 文本。只有在 `Db::find` 无法表达的查询中才手写 SQL：`Db::query::<M>(sql, args)` 把一条 `SELECT` 的行读入 model，只接受字符串字面量形式的 SQL，每个值都作为参数传入，并且最多读取 1000 行。

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

[数据库参考](/reference/database)涵盖连接设置、Oracle、迁移、事务、流，以及每个查询生成的 SQL。

`Page` 携带行、被请求时的总数、下一批的游标以及合计：

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## 行版本

`#[version] pub version: i64` 字段会开启乐观锁。表单会发送它加载时的版本；store 对其进行比较，当有人在此期间保存过时返回 `Error::Conflict`。表单随后会告知用户，而不是覆盖同事的修改。

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## store 中的验证

需要数据才能判断的规则，例如 `unique`，在 store 中检查，并作为字段错误返回：

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## 租户

使用 `App::tenancy(resolver)` 后，每个请求都带有一个租户（`ctx.tenant()`），来自主机名（`SubdomainTenant`）或请求头（`HeaderTenant`）；当请求没有指明租户时，则使用用户自己的租户。已登录的用户必须属于该请求的租户（`Identity::member_of`），否则该请求将以未登录状态运行。store 用它来选择 schema 或过滤行；`TenantRequired` 会拒绝没有租户的请求。

## 已保存的用户状态

通过 `App::datagrid_views(store)`，可以在服务器上按用户保存 grid 视图（列、排序、过滤）、停靠布局和所选主题。`MemoryViewStore` 在内存中保存它们，用于开发；生产环境中，应用用自己的数据库实现 `DataGridViewStore`。

## 后台任务

导入、导出和其他耗时的工作作为带进度的后台作业（`Jobs`）运行，因此请求永远不必等待它们。参见[审计、掩码与作业](enterprise.md)。

除非应用为作业提供 store，否则作业保存在内存中。使用 `SqlJobStore` 时，排队的运行以及每个计划的下一次运行都保存在数据库中：它们在重启后依然存在，多台服务器共享一个队列，租约（lease）确保两个 worker 永远不会运行同一次运行。

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

- `jobs.enqueue(name, payload)` 把一次运行加入队列；`enqueue_after` 会延迟它。payload 是最多 4000 字节的文本，例如一个 id 或小型 JSON。
- worker 以一分钟的租约认领到期的运行，并在运行期间续租。如果服务器停止，租约结束后该运行会被重新认领；这次尝试会计数。
- 失败的尝试会等待退避时间，退避时间逐次翻倍，最长十分钟，直到重试次数用完。之后该运行以 `failed` 状态连同错误码留在表中。计划（schedule）永远不会彻底失败：它会转到下一个间隔。
- PostgreSQL 使用 `FOR UPDATE SKIP LOCKED` 认领，Oracle（11g 及更高版本）通过选择 id 的方式以同样的效果认领。表名是 `zeyn_jobs`；`create_job_tables(dialect, prefix)` 提供用于迁移的语句。
- 内存 store（`MemoryJobStore`）最多保存 10,000 次运行。
