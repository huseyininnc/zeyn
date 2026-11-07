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
- **스크립트.** `range.js`(gzip으로 약 2 KB)는 슬라이더가 있는 페이지에서만 불러옵니다. 낮은 핸들이 높은 핸들을 넘지 않게 하고, 두 핸들 사이의 트랙을 채우고, 레이블 옆에 값을 쓰고, 빈 트랙을 누르면 가까운 핸들을 옮깁니다. 트랙 크기는 누를 때마다 한 번만 읽고 핸들이 움직이는 동안에는 읽지 않습니다.

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
- **스크립트.** `quantity.js`(gzip으로 약 2 KB)는 선택기가 있는 페이지에서만 불러옵니다.

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

## 상품 갤러리

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **한 번에 한 장.** 슬라이드는 스크롤 스냅 트랙에 있어 휴대폰에서는 스크립트 없이 스와이프로 넘깁니다. 이미지 아래 썸네일은 버튼이며("사진 5장 중 2번째 보기", 현재 사진에는 `aria-current`), 포커스된 이미지에서 화살표 키는 페이지의 읽기 방향으로 슬라이드를 넘깁니다. 갤러리는 이름이 있는 캐러셀 영역이고 슬라이드는 "2 of 5"라고 알립니다(W3C APG 캐러셀 패턴). 썸네일(`thumb`)이 없으면 번호 점을 보여 줍니다.
- **보이는 것만 불러옵니다.** 첫 이미지는 보통 상품 페이지에서 가장 큰 요소이므로 `fetchpriority="high"`로 바로 불러오고, 나머지는 비활성 템플릿에서 기다립니다. `gallery.js`가 IntersectionObserver 하나로 지켜보며 화면의 이미지와 양옆 이미지를 넣습니다. 사진 12장짜리 갤러리는 처음에 3장만 불러옵니다. 이미지는 `Image`이므로 자리 표시자, 저해상도 파일, 대체 파일이 그대로 유지됩니다. 첫 이미지는 경쟁만 하게 될 저해상도 파일을 쓰지 않습니다.
- **확대와 전체 화면.** 마우스를 올리면 포인터 아래 이미지가 확대됩니다(슬라이드 크기는 포인터가 들어올 때 한 번만 읽고 움직이는 동안에는 읽지 않습니다). 이미지를 누르거나 "전체 화면" 버튼을 누르면 이미지, 이전과 다음 버튼, 확대 토글, 닫기 버튼이 있는 모달 대화상자가 열립니다. 포커스는 안에 머물고, Escape로 닫히며, 연 요소로 포커스를 돌려줍니다. 동작 줄이기 설정에서는 미끄러지거나 커지는 효과가 없습니다.
- **스크립트.** `gallery.js`(gzip으로 약 2.5 KB)는 갤러리가 있는 페이지에서만 불러옵니다.

## 패싯 패널

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **서버가 세는 개수.** 각 패싯은 체크박스 그룹이며, 옵션은 다른 패싯을 적용했을 때 몇 개의 행이 있는지 보여 줍니다("빨강 (124)", 스크린 리더는 "빨강, 결과 124개"를 듣습니다). 화면은 패싯마다 제한이 있는 쿼리 하나로 셉니다. 예를 들어 다른 필터를 `WHERE`에 둔 `SELECT color, count(*) ... GROUP BY color LIMIT 50`입니다. 패싯 하나는 최대 `MAX_FACET_OPTIONS`(50)개의 옵션을 받습니다. 행이 없는 옵션은 선택되지 않았다면 꺼집니다.
- **변경마다 요청 하나.** 패널은 자신의 토픽을 구동하고 스스로도 듣는 필터 폼입니다. 변경이 일어나면 값이 화면 매개변수에 들어가고(패싯은 목록, 범위는 숫자 두 개) 주소가 맞춰지며, `POST /_zeyn/nodes` 한 번으로 모든 리스너가 새로 고쳐집니다. 카드 그리드, 개수 줄, 새 개수를 가진 패널 자체입니다. 새 변경은 아직 진행 중인 요청을 취소합니다.
- **필터는 서버에 남습니다.** 그리드는 같은 매개변수를 `scope_any(field, values)`와 `scope_range(field, from, to)`(`CardGrid`와 `DataGrid`에 있음)로 받으므로 브라우저는 자신의 필터를 보내지 않습니다. 값이 없거나 끝이 열려 있으면 모든 행이 남습니다.
- **선택한 필터**는 그룹 위에 칩으로 "모두 지우기"와 함께 표시됩니다. 각 칩은 그 필터를 뺀 화면으로 가는 링크라 스크립트 없이도 동작하며, 이름은 "빨강 필터 제거"입니다. 그룹은 접히는 `<details>`이고, 열린 그룹은 새로 고친 뒤에도 열려 있습니다.
- **검사.** `App::check_page`는 아무것도 구동하지 않는 패널과 이름이 화면 매개변수의 필드가 아닌 패싯을 알려 줍니다.
- **휴대폰.** 휴대폰 크기(641 px)보다 좁으면 그룹이 선택한 필터 수를 보여 주는 "필터" 버튼 뒤에 접힌 채로 시작하므로("필터 2", "2개 선택됨"으로 읽힘) 첫 화면에 상품이 보이고, 선택한 칩은 그 위에 계속 보입니다. 버튼은 기본 `<details>` 요약이라 Enter와 Space로 열리고, 패널이 새로 고쳐지는 동안에도 열려 있습니다. 더 넓은 화면은 그룹을 바로 보여 줍니다.

## 리뷰와 평점

```rust
ReviewSummary::new()
    .count(5, stats.five)
    .count(4, stats.four)
    .count(3, stats.three)
    .count(2, stats.two)
    .count(1, stats.one)
```

- **요약.** `ReviewSummary`는 평균을 숫자와 별로, 리뷰 수, 그리고 별 5개부터 1개까지 비율이 표시된 막대를 하나씩 보여 줍니다. 평균과 비율은 개수에서 계산되므로 항상 일치합니다. 스크린 리더는 "별 5개 중 4.6개"와 막대마다 "별 5개, 68%"를 듣고, 막대 자체는 숨겨집니다. 스크립트 없는 HTML입니다.
- **평점 입력.** 폼 필드의 `Control::Rating`은 별 모양으로 그린 라디오 그룹입니다(W3C APG, 스크립트 없음). 정수 필드는 별 하나 단위, `#[rule(min = 1, max = 5, scale = 1)]`이 있는 `Decimal` 필드는 반 개 단위를 받으며, 반 개마다 "4.5 of 5"라는 이름의 라디오 버튼입니다. 서버는 값을 페이지의 숫자 형식으로 읽고 4.3처럼 반 개 사이에 있는 값은 거부합니다.

```rust
#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
    pub text: String,
}

Form::of(Review::default())
    .field(Review::stars)
    .control(Review::stars, Control::Rating)
    .field(Review::text)
    .submit(t!("reviews.send"), save_review)
```

## 카운트다운

```rust
Countdown::until(sale.ends_at)
    .label(t!("shop.sale_ends"))
    .ended(Badge::new(t!("shop.sale_over")))
```

- **절대 종료 시점.** 페이지는 남은 시간이 아니라 종료를 절대 시점(`data-end`)으로 가지고, 브라우저가 그 시점까지 셉니다. 그래서 캐시에서 온 페이지나 나중에 다시 연 페이지도 바로 올바른 시간을 보여 줍니다. 서버는 스크립트가 없는 방문자를 위해 렌더링할 때 남은 시간을 쓰고, 시점이 지나면 시계 대신 `ended`에 넘긴 종료 상태를 스크립트 없이 그립니다.
- **타이머 하나.** `countdown.js`(gzip으로 약 1.5 KB, 진행 중인 카운트다운이 있는 페이지에서만)는 페이지의 모든 카운트다운에 타이머 하나만 두고, 정각 초마다 움직이며, 탭이 숨겨진 동안에는 멈춥니다. 0이 되면 종료 상태가 시계를 대신하고, 토픽의 리스너가 종료로 바뀌는 다른 내용을 새로 고칠 수 있습니다.
- **스크린 리더.** 숫자는 숨겨집니다. 정중한 라이브 영역이 남은 시간을 말로 알려 주며("2일 3시간 14분 남음") 분이 바뀔 때만 바뀌므로 매초 말하지 않습니다.
