# API и OpenAPI

Любое действие (action) может быть и JSON-конечной точкой. Действуют тот же тип входных данных, те же правила и то же разрешение, поэтому API не может разойтись с экранами.

## Публикация действия

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

Каждое действие становится `POST /api/v1/{feature}/{action}`, например `POST /api/v1/stock/save_product`. Тело запроса это входные данные действия в виде JSON, а ответ это его результат в виде JSON.

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` меняет префикс.

## Правила для вызывающих

- Вызовы требуют заголовок `Authorization`. Его читает аутентификатор приложения; одна cookie браузерной сессии отклоняется, поэтому для API CSRF-токен не нужен.
- Доступ, охранники, перехватчики, аудит и лимиты частоты действуют точно так же, как для экранов.
- Входные данные типа `Valid<M>` проходят каждое правило `#[rule]`. Ошибки полей возвращаются с кодом `422` и сообщением для каждого поля.
- Списки принимают `DataQuery` и возвращают `Page`; размер страницы ограничивает сервер.

## Ошибки

Ошибки это сведения о проблеме по RFC 9457 (`application/problem+json`):

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| Статус | `type` оканчивается на | Когда |
|---|---|---|
| 403 | `forbidden` | Нет заголовка `Authorization` или нет разрешения |
| 404 | `not_found` | Неизвестная запись или входные данные, которые не разбираются |
| 409 | `conflict` | Версия строки изменилась |
| 422 | `validation` | Ошибки полей |
| 429 | `forbidden` | Слишком много запросов; см. `Retry-After` |
| 504 | `timeout` | Вызов занял больше времени, чем допускает лимит сервера |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

С `api_docs` приложение публикует `{api_prefix}/openapi.json` (OpenAPI 3.1; импортируйте его в Postman, генератор клиентов или любой просмотрщик OpenAPI) и читаемый справочник по адресу `{api_prefix}/docs`. Схемы берутся из моделей: типы, обязательные поля, длины, границы и значения выбора.

Документация перечисляет каждую конечную точку и нужное для неё разрешение, поэтому по умолчанию она выключена. Выберите, кто может её читать:

| Доступ | Кто |
|---|---|
| `ApiDocs::open()` | Любой |
| `ApiDocs::basic(credentials)` | Базовая HTTP-аутентификация, например `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | Вошедшие пользователи с этим разрешением |

## Браузерные приложения на других источниках

`App::cors(["https://app.example.com"])` позволяет браузерным приложениям именно с этих источников (origins) вызывать API с заголовком `Authorization`. Cookie никогда не отправляются между источниками.
