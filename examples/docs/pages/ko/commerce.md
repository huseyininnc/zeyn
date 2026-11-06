# 쇼핑 컴포넌트

상품 목록과 상품 페이지에는 전용 컴포넌트가 몇 가지 필요합니다. 할인 가격, 개수가 표시되는 필터, 범위 슬라이더, 옵션 선택기, 갤러리, 수량 선택기, 리뷰 요약, 카운트다운입니다. 다른 컴포넌트처럼 서버에서 렌더링됩니다. 브라우저 코드가 필요한 컴포넌트는 자신이 있는 페이지에서만 작은 모듈을 불러오고, 스타일(`commerce.css`)도 그 페이지에서만 연결됩니다.

## 가격

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **할인.** `was(old)`는 현재 가격 옆에 이전 가격을 취소선으로 표시하고, 정수로 반올림한 할인율을 보여 줍니다. 현재 가격보다 높지 않거나 통화가 다른 이전 가격은 생략되므로, 오래된 할인이 음수 비율을 보여 주는 일은 없습니다.
- **스크린 리더**는 "정가 ₺1,599.00, 현재 ₺1,299.00, 19% 할인"이라는 한 문장을 듣습니다. 대부분의 스크린 리더는 취소선을 읽지 않으므로 취소선 가격과 배지는 숨겨집니다.
- **단위 가격.** `unit(price, PriceUnit::Kg)`는 "kg당 ₺259.80" 같은 흐린 줄을 추가합니다. 단위는 `Item`, `Kg`, `Grams100`, `Liter`, `Ml100`, `Meter`입니다.
- **형식.** 금액은 페이지의 숫자 형식을 따르는 `Money`입니다. 영어는 `₺1,299.00`, 터키어는 `1.299,00 ₺`이며, 퍼센트 기호를 앞에 두는 언어에서는 앞에 옵니다(`%19`). `large()`는 상품 페이지에서 현재 가격을 크게 표시합니다.
- **스크립트 없음.** 가격은 일반 HTML입니다.


### 카드의 가격

`ImageCard::price(Price)`는 같은 가격을 `CardGrid`나 `Rail` 카드의 텍스트 아래, 카드 링크 안에 넣으므로 스크린 리더는 제목과 가격 문장을 함께 듣습니다. 스크립트는 추가되지 않으며, 쇼핑 스타일시트는 가격이 있는 첫 카드와 함께 불러옵니다.

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

## 범위 슬라이더

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **두 개의 기본 핸들.** 각 핸들은 두 이름 중 하나를 가진 `<input type="range">`이므로 브라우저가 W3C APG 슬라이더 동작을 제공합니다. 핸들마다 Tab 정지점이 하나씩 있고, 화살표 키는 한 단계씩 움직이며, Home과 End는 양 끝으로 가고, 스크린 리더는 서식이 적용된 값(`aria-valuetext`, 예: "₺500.00")과 레이블("Price, 시작", "Price, 끝")을 듣습니다. Page Up과 Page Down은 범위의 10분의 1씩 움직입니다.
- **필터.** `.drives(topic)`는 슬라이더를 필터 폼에 넣습니다. 핸들을 놓으면 두 값이 같은 이름의 화면 매개변수에 들어가고 그 토픽의 리스너가 제자리에서 새로 고쳐집니다. 패싯 패널 안에서는 패널의 폼이 이 일을 하고, 다른 폼에서는 핸들이 다른 필드처럼 전송됩니다.
- **값.** `value(low, high)`는 매개변수를 받습니다(`None`이면 트랙 끝을 유지). 범위를 벗어난 값은 잘리고, 낮은 값이 높은 값보다 크면 서로 바뀌므로 손으로 고친 주소가 슬라이더를 망가뜨릴 수 없습니다. `money(currency)`는 통화의 정수 단위를 페이지 형식으로 보여 줍니다.
- **날짜.** `RangeSlider::dates(label, names, first, last)`와 `.date_value(from, to)`로 날짜를 고릅니다. 폼은 `2026-03-31` 같은 ISO 날짜를 숨은 필드로 보냅니다.
- **스크립트.** `range.js`(gzip으로 약 1 KB)는 슬라이더가 있는 페이지에서만 불러옵니다. 낮은 핸들이 높은 핸들을 넘지 않게 하고, 두 핸들 사이의 트랙을 채우고, 레이블 옆에 값을 쓰고, 빈 트랙을 누르면 가까운 핸들을 옮깁니다. 트랙 크기는 누를 때마다 한 번만 읽고 핸들이 움직이는 동안에는 읽지 않습니다.

## 수량 선택기

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **서버에서 확인.** 변경(빼기나 더하기 누르기, 숫자 입력)은 사용자가 멈춘 뒤 400 ms 후에 `POST /_zeyn/data`로 전송되며, CSRF 확인과 쓰기 속도 제한을 거치고, 사용자로서 액션의 권한을 확인합니다. 서버는 액션이 실행되기 전에 수량을 선택기의 범위로 자르고, 액션은 재고처럼 자신만 아는 것을 확인합니다. 항목(`target`)은 화면이 정하며 브라우저에서 읽지 않습니다.
- **응답이 우선.** 액션은 유지한 수량과 허용되는 최대값(`QuantityState::new(kept).max(stock)`)으로 응답합니다. 사용자가 요청한 것보다 적으면 선택기가 그 값을 보여 주고 라이브 영역에서 "7개만 남아 있습니다."라고 알립니다. 새 변경은 아직 진행 중인 요청을 취소하므로 마지막 응답만 표시되며, 요청이 실패하면 마지막으로 유지된 수량으로 되돌리고 그 사실을 알립니다.
- **키보드와 터치.** 버튼은 최소 24 × 24 픽셀(WCAG 2.5.8)이고 "줄이기", "늘리기"라는 이름을 가지며 범위 끝에서 꺼집니다. 누른 버튼이 꺼지면 포커스가 입력란으로 갑니다. 입력란은 숫자와 화살표 키를 받습니다.
- **스크립트.** `quantity.js`(gzip으로 약 1 KB)는 선택기가 있는 페이지에서만 불러옵니다.

## 옵션 선택기

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **라디오 그룹.** 각 그룹은 기본 라디오 버튼으로 된 fieldset이며, 스크립트 없는 W3C APG 라디오 그룹 패턴입니다. Tab은 선택된 옵션으로 가고, 화살표 키는 이동하며 선택하고, legend는 그룹과 선택된 옵션("색상 네이비")을 알립니다. 색상 견본은 hex 색이나 테마 토큰을 받고, 칩은 텍스트를 보여 줍니다.
- **고를 수 없는 옵션**은 이유와 함께 꺼집니다(`unavailable(value, reason)`). 스크린 리더는 옵션과 함께 이유를 읽고 툴팁도 보여 줍니다. 색상 견본에는 대각선이, 칩에는 취소선이 그어집니다.
- **가격, 사진, 재고가 따라옵니다.** 선택기는 자신의 토픽을 구동하고 스스로도 듣습니다. 옵션을 고르면 그 값이 그룹 이름의 화면 매개변수에 들어가고, 주소가 맞춰지므로 고른 옵션의 링크를 공유할 수 있으며, 그 토픽을 듣는 모든 노드가 한 번의 요청으로 새로 고쳐집니다. 가격, 갤러리, 재고 줄에 `.listens("variant")`를 붙이면 화면이 새 선택에 맞게 옵션을 다시 끕니다. 토픽 모듈 말고는 스크립트가 필요 없습니다.
- **검사.** `App::check_page`는 아무것도 구동하지 않는 선택기와 올바른 색이 없는 견본을 알려 줍니다.
