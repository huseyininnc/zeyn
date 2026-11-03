# API 与 OpenAPI

任何 action 也可以作为 JSON 端点。输入类型、规则和权限都相同，因此 API 不会与界面产生偏差。

## 发布 action

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

每个 action 都会成为 `POST /api/v1/{feature}/{action}`，例如 `POST /api/v1/stock/save_product`。请求体是 action 输入的 JSON，响应是其输出的 JSON。

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` 可更改前缀。

## 调用方须知

- 调用需要 `Authorization` 头。应用的认证器（authenticator）会读取它；仅有浏览器会话 Cookie 会被拒绝，因此 API 不需要 CSRF 令牌。
- 访问控制、guard、拦截器、审计和速率限制的应用方式与界面完全相同。
- 接受 `Valid<M>` 的输入会运行每一条 `#[rule]`。字段错误以 `422` 返回，并附带每个字段的消息。
- 列表接受 `DataQuery` 并返回 `Page`；服务器会限制页大小。

## 错误

错误采用 RFC 9457 问题详情（problem details，`application/problem+json`）：

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| 状态码 | `type` 结尾为 | 触发条件 |
|---|---|---|
| 403 | `forbidden` | 没有 `Authorization` 头，或没有权限 |
| 404 | `not_found` | 记录未知，或输入无法解析 |
| 409 | `conflict` | 行版本已更改 |
| 422 | `validation` | 字段错误 |
| 429 | `forbidden` | 请求过多；参见 `Retry-After` |
| 504 | `timeout` | 调用耗时超过服务器限制 |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

使用 `api_docs` 后，应用会发布 `{api_prefix}/openapi.json`（OpenAPI 3.1；可导入 Postman、客户端生成器或任何 OpenAPI 查看器），并在 `{api_prefix}/docs` 提供可读的参考文档。模式（schema）来自模型：类型、必填字段、长度、范围和选项值。

文档会列出每个端点及其所需权限，因此默认关闭。请选择谁可以阅读它：

| 访问方式 | 对象 |
|---|---|
| `ApiDocs::open()` | 任何人 |
| `ApiDocs::basic(credentials)` | HTTP 基本认证，例如 `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | 具备该权限的已登录用户 |

## 其他来源的浏览器应用

`App::cors(["https://app.example.com"])` 允许这些确切来源上的浏览器应用携带 `Authorization` 头调用 API。Cookie 绝不会跨来源发送。
