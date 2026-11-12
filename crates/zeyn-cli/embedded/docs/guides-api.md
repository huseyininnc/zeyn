# API and OpenAPI

Any action can also be a JSON endpoint. The same input type, the same rules and the same permission apply, so the API cannot drift from the screens.

## Publishing an action

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

Each action becomes `POST /api/v1/{feature}/{action}`, such as `POST /api/v1/stock/save_product`. The body is the action's input as JSON and the answer is its output as JSON.

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` changes the prefix.

## Rules for callers

- Calls need an `Authorization` header. The app's authenticator reads it; a browser session cookie alone is refused, so the API needs no CSRF token.
- Access, guards, interceptors, audit and rate limits apply exactly as for screens.
- Inputs that take `Valid<M>` run every `#[rule]`. Field errors come back as `422` with each field's message.
- Lists take a `DataQuery` and return a `Page`; the server caps the page size.

## Errors

Errors are RFC 9457 problem details (`application/problem+json`):

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| Status | `type` ends with | When |
|---|---|---|
| 403 | `forbidden` | No `Authorization` header, or no permission |
| 404 | `not_found` | Unknown record, or an input that does not parse |
| 409 | `conflict` | The row version changed |
| 422 | `validation` | Field errors |
| 429 | `forbidden` | Too many requests; see `Retry-After` |
| 504 | `timeout` | The call took longer than the server limit |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

With `api_docs` the app publishes `{api_prefix}/openapi.json` (OpenAPI 3.1; import it in Postman, a client generator or any OpenAPI viewer) and a readable reference at `{api_prefix}/docs`. Schemas come from the models: types, required fields, lengths, bounds and choice values.

The documentation lists every endpoint and the permission it needs, so it is off by default. Choose who may read it:

| Access | Who |
|---|---|
| `ApiDocs::open()` | Anyone |
| `ApiDocs::basic(credentials)` | HTTP basic authentication, such as `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | Signed in users with the permission |

## Browser apps on other origins

`App::cors(["https://app.example.com"])` lets browser apps on these exact origins call the API with an `Authorization` header. Cookies are never sent across origins.
