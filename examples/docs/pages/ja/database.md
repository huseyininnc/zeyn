# データとデータベース

Zeyn は、データベースをオブジェクトマッパーの陰に隠しません。データは、アプリが登録するサービス（ストア）の中にあり、アクションはそれを引数として受け取ります。ストアは、行をメモリに保持しても、データベースを呼び出しても、別のサービスを呼び出してもかまいません。画面、グリッド、API が見るのはアクションだけです。

## ストア

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

`App::service(store)` で一度だけ登録します。画面や API が、登録されていないストアを必要とする場合、アプリは起動を拒否し、どれが足りないかを示します。

## 一覧: DataQuery と Page

一覧アクションは `DataQuery` を受け取り、`Page<M>` を返します。

| フィールド | 意味 |
|---|---|
| `page` | 番号付きページには `Offset { offset, limit }`、無限スクロールには `After { cursor, limit }` |
| `sort` | フィールドと方向（順序付き） |
| `filters` | フィールド、演算子、値。列によって型が決まります |
| `with_total` | クライアントが総件数を必要とするかどうか |
| `aggregates` | フィルター後のすべての行に対して計算する列の合計 |
| `time_zone` | `DateTime` 列の日付フィルター用のゾーン。サーバーが設定します |
| `group` | フィールドで行をグループ化します。グリッドが設定します |

`DataQuery::apply(vec)` は、そのすべてをメモリ上で行います。SQL データベースには `zeyn::sql` を使います。`Db::find` は、同じクエリを PostgreSQL または Oracle 向けの `WHERE`、`ORDER BY`、`LIMIT`、`GROUP BY` に変換します。フィルターや並べ替えができるのはエンティティ自身のフィールドだけで、すべての値はパラメータとしてバインドされ、ページサイズにはサーバーが上限を設けるため、クライアントの入力が SQL の文字列に届くことはありません。`Db::find` では表現できないクエリに限って SQL を手で書き、その場合もクライアントからのフィールド名や値を貼り付けてはいけません。

```toml
zeyn = { version = "0.0.5", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

[データベースリファレンス](/reference/database) では、接続設定、Oracle、マイグレーション、トランザクション、ストリーム、各クエリが変換される SQL を扱います。

`Page` は、行、要求された場合の総件数、次のバッチのカーソル、合計を持ちます。

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## 行のバージョン

`#[version] pub version: i64` フィールドは、楽観的ロックを有効にします。フォームは読み込んだバージョンを送り、ストアはそれを比較して、その間に誰かが保存していれば `Error::Conflict` を返します。フォームはその場合、同僚の変更を上書きせず、ユーザーに知らせます。

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## ストアでの検証

`unique` のように、データを必要とするルールはストアで検査され、フィールドのエラーとして返されます。

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## テナント

`App::tenancy(resolver)` を使うと、すべてのリクエストがテナント（`ctx.tenant()`）を持ちます。テナントは、ホスト名（`SubdomainTenant`）かヘッダー（`HeaderTenant`）から、またはリクエストがテナントを示さない場合はユーザー自身のテナントから決まります。サインイン済みのユーザーはリクエストのテナントに属している必要があり（`Identity::member_of`）、そうでなければリクエストはサインアウト状態で実行されます。ストアはこれを使ってスキーマを選んだり行を絞り込んだりし、`TenantRequired` はテナントのないリクエストを拒否します。

## 保存されるユーザーの状態

グリッドのビュー（列、並べ替え、フィルター）、ドックのレイアウト、選択したテーマは、`App::datagrid_views(store)` により、ユーザーごとにサーバー上に保持できます。`MemoryViewStore` は開発用にそれらをメモリに保持し、本番ではアプリが自分のデータベースで `DataGridViewStore` を実装します。

## バックグラウンド処理

インポート、エクスポート、その他の長い処理は、進捗付きのバックグラウンドジョブ（`Jobs`）として実行されるため、リクエストがそれを待つことはありません。[監査、マスキング、ジョブ](enterprise.md) を参照してください。

ジョブは、アプリがストアを与えない限りメモリ上にあります。`SqlJobStore` を使うと、キューに入った実行と各スケジュールの次回の実行がデータベースに保持されます。再起動後も残り、複数のサーバーが 1 つのキューを共有でき、リースによって 2 つのワーカーが同じ実行を行うことはありません。

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

- `jobs.enqueue(name, payload)` は実行をキューに入れ、`enqueue_after` は遅延させます。ペイロードは最大 4000 バイトのテキストで、ID や小さな JSON などです。
- ワーカーは、期限の来た実行を 1 分間のリースで確保し、実行中は更新し続けます。サーバーが停止した場合は、リースが切れた後にその実行が再び確保され、その試行も数えられます。
- 失敗した試行はバックオフを待ちます。バックオフは 10 分まで倍々に増え、再試行が尽きるまで続きます。その後、実行は `failed` としてエラーコードとともにテーブルに残ります。スケジュールが完全に失敗することはなく、次の間隔に進みます。
- PostgreSQL は `FOR UPDATE SKIP LOCKED` で確保し、Oracle（11g 以降）は ID を選択する形で同じことを行います。テーブルは `zeyn_jobs` で、`create_job_tables(dialect, prefix)` がマイグレーション用の文を提供します。
- メモリ上のストア（`MemoryJobStore`）は、最大 10,000 件の実行を保持します。
