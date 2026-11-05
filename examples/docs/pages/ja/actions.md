# アクション

アクションは、画面、グリッド、フォーム、コマンド、JSON API、あるいはエージェントが呼び出せるサーバーコードです。`#[action]` が呼び出せる人を宣言し、フレームワークはコードが実行される前にそれを検査します。

## アクションの宣言

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

- 最初の引数が入力です。クライアントから来るため、フレームワークは信頼できない JSON として読み取ります。
- それ以外の引数はすべて注入されます。`Ctx` か、登録済みの `Service` です。
- 戻り値の型は `Result<T>` です。

## アクセス

すべてのアクションは、次のどちらか一方を必ず宣言します。

| オプション | 意味 |
|---|---|
| `permission = "feature.resource.verb"` | ID にこの権限が必要です |
| `public` | サインインしていないユーザーを含め、誰でも呼び出せます |

アクセスを宣言しないアクションはコンパイルできません。権限名は小文字のドット区切りで、`sales.order.write` のようにします。

その他のオプション:

| オプション | 意味 |
|---|---|
| `audit` | 呼び出しのたびに監査レコードを書きます |
| `reauth` | ユーザーが最近サインインしている必要があります（`App::reauth_within`） |

## エラー

| エラー | ステータス | 用途 |
|---|---|---|
| `Error::NotFound` | 404 | レコードが存在しない、またはユーザーが存在を知ってはならない場合 |
| `Error::Forbidden` | 403 | アクセス拒否 |
| `Error::Denied(t!("..."))` | 403 | ユーザーが読める理由付きの拒否 |
| `Error::Conflict` | 409 | フォームを読み込んでから行のバージョンが変わった場合 |
| `Error::Validation(errors)` | 422 | フィールドのエラー。フィールドの横に表示されます |
| `Error::Internal("detail")` | 500 | 詳細はサーバーログにのみ出力されます |

コードからのフィールドエラー:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## 一覧

一覧は `DataQuery` を受け取り、`Page<M>` を返します。一覧アクションがすべての行を返すことはありません。グリッドはページ単位で要求し、サーバーはクライアントが何を送ってもページサイズに上限を設けます。

アプリが自分で計算する、または独自のインデックスに持つテーブル（数百万行、ID から組み立てるイベント）では、`q.span()` が要求された行を、ページ番号でもカーソルでも、オフセットと件数で返し、`Page::window(rows, total, span)` がそれらを総数と次の行へのカーソル付きで返します。作られるのはそのページの行だけです。並べ替えとフィルターは、アプリが `q.sort` と `q.filters` を読んで自分で適用します。

```rust
#[action(permission = "audit.read")]
pub async fn search_events(q: DataQuery, log: EventLog) -> Result<Page<Event>> {
    let span = q.span();
    let rows = log.range(span.offset, span.limit)?;
    Ok(Page::window(rows, log.len()?, span))
}
```

## コンテキスト

`Ctx` はリクエストを表します。`ctx.identity()`、`ctx.locale()`、`ctx.time_zone()`、`ctx.now()`、`ctx.today()`、`ctx.tenant()`、`ctx.license()`、`ctx.config()`、`ctx.theme()`、`ctx.t(&text)` があります。システムクロックの代わりに `ctx.now()` と `ctx.today()` を使うと、テストで `App::fixed_now` により時刻を固定できます。

## アクションの使われ方

- `DataGrid::of(action)` は行を読み込み、`.edit_with(action)`、`.row_action(...)`、`.bulk_action(...)` は行を変更します。
- `Form::submit(label, action)` はフォームを保存します。
- `ActionButton::new(label, action, input)` はボタンからアクションを実行します。
- `Command::action(...)` はコマンドパレットやショートカットから実行します。
- `Feature::api(action)` は JSON として公開します（[API と OpenAPI](api.md) を参照）。
- `Feature::search(action)` はコマンドパレットにサーバー検索を追加します。

どの経路でも同じ検査が実行されます。ブラウザからの呼び出しには CSRF、次に Feature のガード、アクションのアクセス権、そしてインターセプターの順です。
