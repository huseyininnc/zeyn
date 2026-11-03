# Action

action은 Screen, 그리드, 폼, 명령, JSON API, 에이전트가 호출할 수 있는 서버 코드입니다. `#[action]`은 누가 호출할 수 있는지 선언하며, 프레임워크는 코드를 실행하기 전에 이를 검사합니다.

## action 선언하기

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

- 첫 번째 인수는 입력입니다. 클라이언트에서 오므로 프레임워크는 이를 신뢰할 수 없는 JSON으로 읽습니다.
- 나머지 인수는 모두 주입됩니다. `Ctx` 또는 등록된 `Service`입니다.
- 반환 형식은 `Result<T>`입니다.

## 접근 권한

모든 action은 다음 중 정확히 하나를 선언합니다.

| 옵션 | 의미 |
|---|---|
| `permission = "feature.resource.verb"` | ID에 이 권한이 필요합니다 |
| `public` | 로그인하지 않은 사용자를 포함해 누구나 가능합니다 |

접근 권한이 없는 action은 컴파일되지 않습니다. 권한 이름은 `sales.order.write`처럼 소문자에 점으로 구분합니다.

그 밖의 옵션은 다음과 같습니다.

| 옵션 | 의미 |
|---|---|
| `audit` | 호출할 때마다 감사 기록을 남깁니다 |
| `reauth` | 사용자가 최근에 로그인했어야 합니다 (`App::reauth_within`) |

## 오류

| 오류 | 상태 | 용도 |
|---|---|---|
| `Error::NotFound` | 404 | 레코드가 없거나 사용자가 존재 여부를 알아서는 안 되는 경우 |
| `Error::Forbidden` | 403 | 접근 거부 |
| `Error::Denied(t!("..."))` | 403 | 사용자가 읽을 수 있는 사유와 함께 거부 |
| `Error::Conflict` | 409 | 폼을 불러온 이후 행 버전이 바뀐 경우 |
| `Error::Validation(errors)` | 422 | 필드 오류, 필드 옆에 표시됩니다 |
| `Error::Internal("detail")` | 500 | 상세 내용은 서버 로그에만 남습니다 |

코드에서 필드 오류를 만드는 방법은 다음과 같습니다.

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## 목록

목록은 `DataQuery`를 받아 `Page<M>`을 반환합니다. 목록 action은 모든 행을 반환하지 않습니다. 그리드는 페이지 단위로 요청하며, 클라이언트가 무엇을 보내든 서버가 페이지 크기에 상한을 둡니다.

## 컨텍스트

`Ctx`는 요청입니다. `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()`, `ctx.t(&text)`를 제공합니다. 시스템 시계 대신 `ctx.now()`와 `ctx.today()`를 사용하면 테스트에서 `App::fixed_now`로 시간을 고정할 수 있습니다.

## action이 쓰이는 곳

- `DataGrid::of(action)`은 행을 불러오고, `.edit_with(action)`, `.row_action(...)`, `.bulk_action(...)`은 행을 변경합니다.
- `Form::submit(label, action)`은 폼을 저장합니다.
- `ActionButton::new(label, action, input)`은 버튼에서 action을 실행합니다.
- `Command::action(...)`은 명령 팔레트나 단축키에서 action을 실행합니다.
- `Feature::api(action)`은 action을 JSON으로 게시합니다 ([API와 OpenAPI](api.md) 참고).
- `Feature::search(action)`은 명령 팔레트에 서버 검색을 추가합니다.

모든 경로가 같은 검사를 거칩니다. 브라우저 호출에는 CSRF, 그다음 Feature의 가드, action의 접근 권한, 마지막으로 인터셉터 순서입니다.
