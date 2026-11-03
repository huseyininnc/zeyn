# 라인 편집기

모델의 `Vec<M>` 필드는 청구서의 품목처럼 라인의 목록입니다. `.lines(M::f, ...)`는 이를 폼 안의 작은 그리드로 편집합니다. 라인을 추가, 제거, 이동하고, 키보드로 셀 사이를 이동하며, 입력하는 동안 합계를 볼 수 있습니다. 라인을 포함한 폼 전체가 하나의 action으로 검증되고 저장됩니다.

```rust
#[model]
pub struct Line {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<Line>,
}

Form::of(invoice)
    .field(Invoice::customer)
    .lines(Invoice::lines, |l| {
        l.column(Line::item)
            .column(Line::quantity)
            .column(Line::unit_price)
            .total(Line::quantity, Line::unit_price)
    })
    .submit(t!("docs.live.save"), save_invoice)
```

@live lines

## 열과 합계

`.column(Line::f)`는 해당 타입의 컨트롤을 가진 편집 가능한 열을 추가합니다. `.total(quantity, price)`는 각 라인의 수량 곱하기 가격과 라인 아래의 합계를 폼의 통화로 보여 줍니다.

## 검증

각 라인은 모델의 규칙으로 검사되며 오류는 셀에 나타납니다. `Vec` 필드의 `#[rule(required)]`는 라인이 최소 하나 있어야 한다는 뜻입니다. 라인의 수는 클라이언트가 무엇을 보내든 서버가 제한합니다(`MAX_LINES`).

## 저장

action은 `Valid<Invoice>`를 받으므로 action이 실행되기 전에 모든 라인이 규칙을 통과한 상태입니다.

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
