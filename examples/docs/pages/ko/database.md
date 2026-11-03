# 데이터와 데이터베이스

Zeyn은 데이터베이스를 객체 매퍼 뒤에 숨기지 않습니다. 데이터는 앱이 등록하는 서비스(스토어)에 있으며, action은 이를 인수로 받습니다. 스토어는 행을 메모리에 보관하거나, 데이터베이스를 호출하거나, 다른 서비스를 호출할 수 있습니다. Screen, 그리드, API는 action만 봅니다.

## 스토어

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

`App::service(store)`로 한 번 등록합니다. Screen이나 API가 등록되지 않은 스토어를 필요로 하면 앱은 시작을 거부하고 어느 스토어인지 알려 줍니다.

## 목록: DataQuery와 Page

목록 action은 `DataQuery`를 받아 `Page<M>`을 반환합니다.

| 필드 | 의미 |
|---|---|
| `page` | 번호가 있는 페이지는 `Offset { offset, limit }`, 무한 스크롤은 `After { cursor, limit }` |
| `sort` | 순서대로 나열한 필드와 방향 |
| `filters` | 필드, 연산자, 값이며 열의 타입으로 정해집니다 |
| `with_total` | 클라이언트가 전체 개수를 필요로 하는지 여부 |
| `aggregates` | 필터링된 모든 행에 대해 계산할 열 합계 |
| `time_zone` | `DateTime` 열의 날짜 필터에 쓰는 시간대이며 서버가 설정합니다 |
| `group` | 필드로 행을 그룹화하며 그리드가 설정합니다 |

`DataQuery::apply(vec)`는 이 모든 것을 메모리에서 처리합니다. SQL 데이터베이스에는 `zeyn::sql`을 사용합니다. `Db::find`는 같은 쿼리를 PostgreSQL 또는 Oracle용 `WHERE`, `ORDER BY`, `LIMIT`, `GROUP BY`로 바꿉니다. 엔티티 자신의 필드만 필터링하거나 정렬할 수 있고, 모든 값은 매개변수로 바인드되며, 페이지 크기는 서버가 제한하므로 클라이언트 입력이 SQL 텍스트에 도달하지 않습니다. `Db::find`로 표현할 수 없는 쿼리에만 SQL을 직접 작성하고, 그 경우에도 클라이언트의 필드 이름이나 값을 그대로 붙여 넣지 마십시오.

```toml
zeyn = { version = "0.0.6", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

[데이터베이스 레퍼런스](/reference/database)는 연결 설정, Oracle, 마이그레이션, 트랜잭션, 스트림, 각 쿼리가 변환되는 SQL을 다룹니다.

`Page`는 행, 요청한 경우의 전체 개수, 다음 묶음의 커서, 합계를 담습니다.

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## 행 버전

`#[version] pub version: i64` 필드는 낙관적 잠금을 켭니다. 폼은 불러온 버전을 보내고, 스토어는 이를 비교하여 그사이에 다른 사람이 저장했다면 `Error::Conflict`를 반환합니다. 그러면 폼은 동료의 변경을 덮어쓰는 대신 사용자에게 알립니다.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## 스토어에서의 검증

`unique`처럼 데이터가 필요한 규칙은 스토어에서 검사하고 필드 오류로 반환합니다.

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## 테넌트

`App::tenancy(resolver)`를 사용하면 모든 요청이 호스트 이름(`SubdomainTenant`)이나 헤더(`HeaderTenant`)에서, 요청이 아무것도 지정하지 않으면 사용자 자신의 테넌트에서 가져온 테넌트(`ctx.tenant()`)를 가집니다. 로그인한 사용자는 요청의 테넌트에 속해야 하며(`Identity::member_of`), 그렇지 않으면 요청은 로그아웃 상태로 실행됩니다. 스토어는 이를 사용해 스키마를 고르거나 행을 필터링하며, `TenantRequired`는 테넌트가 없는 요청을 거부합니다.

## 저장된 사용자 상태

그리드 뷰(열, 정렬, 필터), 도크 레이아웃, 선택한 테마는 `App::datagrid_views(store)`로 서버에서 사용자별로 보관할 수 있습니다. `MemoryViewStore`는 개발용으로 메모리에 보관하고, 운영에서는 앱이 자신의 데이터베이스로 `DataGridViewStore`를 구현합니다.

## 백그라운드 작업

가져오기, 내보내기, 그 밖의 긴 작업은 진행 상황이 있는 백그라운드 작업(`Jobs`)으로 실행되므로 요청이 기다리지 않습니다. [감사, 마스킹, 작업](enterprise.md)을 참고하십시오.

앱이 스토어를 지정하지 않으면 작업은 메모리에 있습니다. `SqlJobStore`를 사용하면 대기 중인 실행과 모든 일정의 다음 실행이 데이터베이스에 보관됩니다. 재시작 후에도 유지되고, 여러 서버가 하나의 큐를 공유하며, 리스(lease)가 두 워커가 같은 실행을 하지 않도록 보장합니다.

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

- `jobs.enqueue(name, payload)`는 실행을 큐에 넣고, `enqueue_after`는 지연시킵니다. 페이로드는 id나 작은 JSON 같은 최대 4000바이트의 텍스트입니다.
- 워커는 1분의 리스로 기한이 된 실행을 가져가고, 실행이 진행되는 동안 리스를 갱신합니다. 서버가 멈추면 리스가 끝난 후 그 실행을 다시 가져가며, 이 시도도 횟수에 포함됩니다.
- 실패한 시도는 백오프만큼 기다리며, 백오프는 최대 10분까지 두 배로 늘어나다가 재시도를 모두 소진합니다. 그러면 실행은 오류 코드와 함께 `failed`로 테이블에 남습니다. 일정은 영구적으로 실패하지 않고 다음 간격으로 넘어갑니다.
- PostgreSQL은 `FOR UPDATE SKIP LOCKED`로 가져가고, Oracle(11g 이상)은 id를 선택하는 방식으로 같은 일을 합니다. 테이블은 `zeyn_jobs`이며, `create_job_tables(dialect, prefix)`가 마이그레이션용 구문을 제공합니다.
- 메모리 스토어(`MemoryJobStore`)는 최대 10,000개의 실행을 보관합니다.
