# API と OpenAPI

どのアクションも JSON エンドポイントにできます。同じ入力型、同じルール、同じ権限が適用されるため、API が画面とずれることはありません。

## アクションの公開

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

各アクションは `POST /api/v1/{feature}/{action}` になります。たとえば `POST /api/v1/stock/save_product` です。リクエストボディはアクションの入力を JSON にしたもので、応答はその出力を JSON にしたものです。

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` でプレフィックスを変更できます。

## 呼び出し側のルール

- 呼び出しには `Authorization` ヘッダーが必要です。アプリの認証機構がそれを読み取ります。ブラウザのセッション Cookie だけでは拒否されるため、API に CSRF トークンは不要です。
- アクセス権、ガード、インターセプター、監査、レート制限は、画面とまったく同じように適用されます。
- `Valid<M>` を受け取る入力では、すべての `#[rule]` が実行されます。フィールドのエラーは、各フィールドのメッセージとともに `422` で返ります。
- 一覧は `DataQuery` を受け取って `Page` を返し、サーバーがページサイズに上限を設けます。

## エラー

エラーは RFC 9457 の problem details（`application/problem+json`）です。

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| ステータス | `type` の末尾 | 発生条件 |
|---|---|---|
| 403 | `forbidden` | `Authorization` ヘッダーがない、または権限がない |
| 404 | `not_found` | 不明なレコード、または解釈できない入力 |
| 409 | `conflict` | 行のバージョンが変わった |
| 422 | `validation` | フィールドのエラー |
| 429 | `forbidden` | リクエストが多すぎます。`Retry-After` を参照 |
| 504 | `timeout` | 呼び出しがサーバーの制限より長くかかった |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

`api_docs` を指定すると、アプリは `{api_prefix}/openapi.json`（OpenAPI 3.1。Postman、クライアントジェネレーター、任意の OpenAPI ビューアーにインポートできます）と、読みやすいリファレンスを `{api_prefix}/docs` で公開します。スキーマはモデルから作られ、型、必須フィールド、長さ、範囲、選択肢の値が含まれます。

このドキュメントはすべてのエンドポイントと必要な権限を一覧にするため、既定では無効です。誰が読めるかを選んでください。

| アクセス | 対象 |
|---|---|
| `ApiDocs::open()` | 誰でも |
| `ApiDocs::basic(credentials)` | HTTP ベーシック認証。たとえば `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | その権限を持つサインイン済みユーザー |

## 別のオリジンのブラウザアプリ

`App::cors(["https://app.example.com"])` は、これらの正確なオリジンのブラウザアプリが、`Authorization` ヘッダー付きで API を呼び出せるようにします。Cookie はオリジンをまたいで送信されることはありません。
