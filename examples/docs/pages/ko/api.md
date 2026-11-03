# API와 OpenAPI

모든 action은 JSON 엔드포인트가 될 수도 있습니다. 같은 입력 형식, 같은 규칙, 같은 권한이 적용되므로 API가 Screen과 어긋날 수 없습니다.

## action 게시하기

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

각 action은 `POST /api/v1/{feature}/{action}`이 되며, 예를 들면 `POST /api/v1/stock/save_product`입니다. 본문은 JSON 형태의 action 입력이고 응답은 JSON 형태의 출력입니다.

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")`로 접두사를 바꿀 수 있습니다.

## 호출자를 위한 규칙

- 호출에는 `Authorization` 헤더가 필요합니다. 앱의 인증기가 이를 읽으며, 브라우저 세션 쿠키만으로는 거부되므로 API에는 CSRF 토큰이 필요하지 않습니다.
- 접근 권한, 가드, 인터셉터, 감사, 속도 제한은 Screen과 똑같이 적용됩니다.
- `Valid<M>`을 받는 입력은 모든 `#[rule]`을 실행합니다. 필드 오류는 각 필드의 메시지와 함께 `422`로 돌아옵니다.
- 목록은 `DataQuery`를 받아 `Page`를 반환하며, 서버가 페이지 크기에 상한을 둡니다.

## 오류

오류는 RFC 9457 problem details(`application/problem+json`)입니다.

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| 상태 | `type`의 끝 | 발생 시점 |
|---|---|---|
| 403 | `forbidden` | `Authorization` 헤더가 없거나 권한이 없는 경우 |
| 404 | `not_found` | 알 수 없는 레코드이거나 파싱되지 않는 입력 |
| 409 | `conflict` | 행 버전이 바뀐 경우 |
| 422 | `validation` | 필드 오류 |
| 429 | `forbidden` | 요청이 너무 많음. `Retry-After`를 확인하십시오 |
| 504 | `timeout` | 호출이 서버 제한 시간보다 오래 걸린 경우 |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

`api_docs`를 사용하면 앱이 `{api_prefix}/openapi.json`(OpenAPI 3.1이며 Postman, 클라이언트 생성기, 모든 OpenAPI 뷰어로 가져올 수 있습니다)과 `{api_prefix}/docs`의 읽기 쉬운 레퍼런스를 게시합니다. 스키마는 모델에서 만들어집니다. 형식, 필수 필드, 길이, 범위, choice 값이 포함됩니다.

문서에는 모든 엔드포인트와 필요한 권한이 나열되므로 기본적으로 꺼져 있습니다. 누가 읽을 수 있는지 선택합니다.

| 접근 | 대상 |
|---|---|
| `ApiDocs::open()` | 누구나 |
| `ApiDocs::basic(credentials)` | HTTP 기본 인증, 예를 들어 `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | 해당 권한을 가진 로그인한 사용자 |

## 다른 출처의 브라우저 앱

`App::cors(["https://app.example.com"])`는 정확히 이 출처의 브라우저 앱이 `Authorization` 헤더와 함께 API를 호출하도록 허용합니다. 쿠키는 출처를 넘어 전송되지 않습니다.
