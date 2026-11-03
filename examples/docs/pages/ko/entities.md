# Entity와 모델

데이터는 속성이 붙은 Rust struct로 한 번만 설명합니다. 이 하나의 설명이 폼, 그리드 열, 서버와 브라우저의 검증, JSON API, 그리고 OpenAPI 스키마를 이끕니다.

## Entity

`#[entity(table = "...")]`는 테이블의 행을 표시합니다. `#[key]` 필드가 하나 필요하며, `ProductId` 같은 타입이 있는 id와 `Product::code` 같은 각 필드의 필드 참조를 만듭니다.

```rust
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}
```

필드 참조에는 타입이 있습니다. `Product::price`는 `Field<Product, Money>`입니다. 텍스트 열에 그리드 합계를 쓰거나 다른 모델의 폼 필드를 쓰면 컴파일되지 않습니다.

## 모델

`#[model]`은 테이블 행이 아닌 데이터입니다. 필터, 마법사 단계, 폼 입력이 그 예입니다.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]`는 단위 variant로 이루어진 enum을 선택 목록, 라디오 그룹, 세그먼트 컨트롤로 바꿉니다. 레이블은 로케일 파일에서 가져옵니다.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## 필드 속성

| 속성 | 의미 |
|---|---|
| `#[key]` | entity의 기본 키 |
| `#[version]` | `i64` 행 버전. 이전 버전으로 저장하면 `Error::Conflict`로 실패합니다 |
| `#[readonly]` | 표시되지만 폼이 쓰지는 않습니다 |
| `#[label("...")]` | 로케일 파일에 없을 때 쓰는 대체 레이블 |
| `#[rule(...)]` | 검증 규칙, 아래 참고 |

## 규칙

| 규칙 | 대상 | 의미 |
|---|---|---|
| `required` | 모두 | 값이 있어야 합니다 |
| `min_len = n`, `max_len = n` | 텍스트 | 문자 단위 길이 |
| `email` | 텍스트 | 이메일 주소 |
| `embed` | 텍스트 | `Embed`로 삽입할 주소: 앱의 `frame-src`(`App::csp`)에 있는 호스트의 `https`. 폼은 붙여 넣은 `<iframe>` 코드도 받아 추적 매개변수 없는 정규 주소만 저장합니다. [표시](display.md#embed) 참고 |
| `min = n`, `max = n` | 숫자, 금액, decimal, 날짜 | 범위. 날짜는 `2026_01_01`로 씁니다 |
| `scale = n`, `precision = n` | `Decimal` | 소수 자릿수의 최대값(규칙이 없으면 2)과 전체 자릿수의 최대값(규칙이 없으면 18). 열 타입 `NUMERIC(precision, scale)`도 결정합니다 |
| `mask = "..."` | 텍스트 | 원시 값이 패턴을 채웁니다. `9`는 숫자, `a`는 문자, `A`는 대문자, `*`는 둘 중 하나이며, 다른 문자는 입력하는 동안 표시되지만 저장되지 않습니다. [컨트롤](controls.md#masked-input) 참고 |
| `unique` | 모두 | 스토어가 검사합니다. 폼은 action의 오류를 표시합니다 |

필드 타입에 맞지 않는 규칙은 해야 할 일을 알려 주는 메시지와 함께 컴파일 오류가 됩니다. 여러 필드에 걸친 규칙은 함수에 둡니다.

```rust
#[model(validate = check_stay)]
pub struct Stay {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn check_stay(s: &Stay, errors: &mut ValidationErrors) {
    if let (Some(a), Some(b)) = (s.from, s.to) {
        if b < a {
            errors.add(Stay::to, t!("stay.before_start"));
        }
    }
}
```

## Valid

저장하는 action은 `Valid<M>`을 받습니다. `Valid` 값은 모든 규칙을 실행해야만 만들 수 있으므로 action이 검증을 잊을 수 없습니다.

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## 필드 타입

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, `#[choice]` enum, entity id, 그리고 이들 각각의 `Option<T>`입니다. 모델의 `Vec<M>`은 폼에서 라인 편집기가 됩니다([라인 편집기](lines.md) 참고).

Money는 통화와 함께 정수 최소 단위로 저장됩니다. 금액에 `f64`를 사용하지 마십시오. `Decimal`은 수량, 소수 둘째 자리를 넘는 단가, 이율처럼 소수부가 있는 그 밖의 모든 것을 위한 정확한 숫자입니다. 최대 38자리를 담고, 부동 소수점을 거치지 않으며, JSON에서는 문자열(`"12.50"`)로 전달됩니다. 날짜와 시간은 [시간과 언어](time-and-languages.md)에서 다룹니다.
