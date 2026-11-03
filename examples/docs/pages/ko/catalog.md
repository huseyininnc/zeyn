# 카드, 레일, 검색

쇼핑몰 같은 공개 페이지는 행을 표가 아니라 카드로 보여 줍니다. `CardGrid`, `Rail`, `SearchBox`는 DataGrid가 사용하는 것과 같은 종류의 action으로 행을 불러옵니다. 이 action은 `DataQuery`를 받아 `Page`를 반환합니다. 페이징, 정렬, 검색 텍스트는 서버에서 실행되며, DataGrid와 마찬가지로 읽기에는 action의 접근 권한이 필요합니다.

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## 카드 그리드

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **카드.** `Card::image(image, title)`은 행 하나로 카드 하나를 만들며, `text`, `badge`, `alt`, `link`를 사용할 수 있습니다. `link`는 Screen과 그 매개변수이며, 카드 전체가 하나의 링크이고 페이지 새로 고침 없이 열립니다. 이미지는 지연 로드되며, 페이지가 흔들리지 않도록 `image_size`의 너비와 높이를 사용합니다. 이미지의 대체 텍스트는 비어 있습니다. 옆의 제목이 카드를 설명하기 때문입니다. 그림이 더 많은 내용을 전달한다면 `alt`를 설정하십시오. 경로, `https:`, `data:image/`가 아닌 이미지 주소는 제외됩니다.
- **카드의 추가 요소.** `subtitle`은 제목 아래에 더 흐린 두 번째 줄을 추가합니다(저자, 카테고리 등). `corner(badge)`는 이미지의 끝 모서리 위에 배지를 놓으며, 시작 모서리에는 `badge`가 놓입니다. `badges([...])`는 텍스트 아래에 태그 같은 배지 한 줄을 추가합니다. `progress(fraction, label)`은 이미지 아래쪽 가장자리를 따라 진행 정도를 나타내는 얇은 막대를 그립니다. 비율은 0에서 1까지이며, 스크린 리더는 레이블과 백분율을 읽어 줍니다(`role="progressbar"`와 `aria-valuenow`). `horizontal()`은 카드를 한 행으로 배치하여 시작 쪽에 작은 이미지를, 그 옆에 텍스트를 둡니다. 이런 카드가 있는 카드 그리드나 rail은 더 넓은 열을 줍니다. 모두 `CardGrid`와 `Rail`에서 동작합니다.

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **페이징.** `Paging::infinite(n)`(기본값이며 24)은 "더 보기" 버튼을 표시하고, 이 버튼이 화면에 스크롤되어 들어오면 자동으로도 동작합니다. 버튼을 누르면 포커스가 첫 번째 새 카드로 이동합니다. `Paging::pages(n)`은 이전과 다음 버튼 및 페이지 번호를 표시합니다. 페이지 크기는 서버가 정하며, 클라이언트는 어디서부터 이어갈지만 알립니다.
- **정렬.** 각 `sort_option`은 사용자가 선택 목록에서 고를 수 있는 순서입니다. 첫 번째가 카드가 처음 나열되는 순서입니다. 서버는 이 순서만 받아들이고, 클라이언트가 보낸 필터와 검색 텍스트는 버립니다.
- **검색.** `.search(text)`는 자유 텍스트([`DataQuery::q`](#free-text))와 일치하는 행만 표시합니다. 예를 들어 검색 결과 화면의 텍스트가 있습니다.
- **마스킹된 필드.** 카드는 원본 행을 보는 클로저가 만듭니다. `#[mask]` 필드가 있는 모델에서는 이를 공개할 수 있는 사용자만 카드를 받고, 그 외에는 `Forbidden`을 받습니다. 공개 카드에는 마스킹된 필드가 없는 모델을 사용하십시오.

### 호버 미리보기

링크가 있는 `ImageCard` 또는 `ListItem`의 `.preview(screen, params)`는 그 화면을 카드 옆 패널에 표시합니다. 마우스로 약 0.5초 동안 올려 두었을 때, 또는 카드가 키보드 포커스를 받으면 즉시 표시됩니다. 화면은 그때서야 창과 같은 접근 검사를 거쳐 `GET /_zeyn/screen/...`에서 로드되며, 각 미리보기는 페이지 수명 동안 유지됩니다(최대 24개, `nav.js`가 다른 화면을 열면 삭제됨). 프레임, 객체, 미디어 플레이어, 스크립트는 패널이 표시되기 전에 제거되므로 미리보기는 iframe을 로드하지 않습니다. 패널은 최상위 레이어에 있으므로 콘텐츠를 잘라내는 rail과 그리드가 패널을 자르지 않으며, 공간이 더 넓은 쪽으로 열리고, 화면 가장자리에서 뒤집히며, 내부에서 스크롤되고, 페이지를 스크롤하지 않습니다. WCAG 1.4.13을 따릅니다. 포인터가 패널 위로 이동할 수 있고, Escape로 닫히며, 포인터나 포커스가 떠날 때까지 유지됩니다. 터치 화면과 거친 포인터에서는 미리보기가 없고 탭하면 링크가 열립니다. 스크립트(`preview.js`, 2.5 KB gzip)는 그곳에서는 로드되지 않으며, 그 외에는 그러한 카드에 처음 호버하거나 포커스할 때 로드됩니다. 동작 줄이기에서는 패널이 짧은 확대 애니메이션 없이 나타납니다.

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview


## 레일

레일은 한 줄로 배치되어 옆으로 스크롤되는 카드 그리드입니다. 스크롤 스냅과 한 화면 너비만큼 스크롤하는 이전, 다음 버튼이 있으며, 사용자가 동작 줄이기를 요청하지 않았다면 부드럽게 스크롤됩니다. 사용자가 끝 근처까지 스크롤하면 다음 묶음을 12개씩 불러옵니다. 카드는 탭 순서에 있는 링크이며, 포커스를 받으면 화면 안으로 스크롤됩니다.

스크롤바는 숨겨져 있습니다. 터치, 트랙패드, Shift+휠로는 계속 스크롤할 수 있으며 `.scrollbar(true)`는 얇은 막대를 남깁니다. 마우스로 rail을 누른 채 옆으로 끌 수 있습니다. 5픽셀을 넘게 움직이면 놓을 때의 클릭이 취소되어 포인터 아래의 카드가 열리지 않고, rail은 가장 가까운 카드에 맞춰집니다. 더 작게 움직이면 일반 클릭입니다. 끄는 동안에는 텍스트가 선택되지 않고, 이미지가 밖으로 끌려 나가지 않으며, 카드 미리보기도 열리지 않습니다. 터치에서는 브라우저 자체 스크롤이 유지됩니다. 이전 및 다음 버튼은 항상 있으며 양 끝에서 비활성화됩니다. 숨겨진 스크롤바로는 키보드로 스크롤할 수 없기 때문입니다. JavaScript가 없으면 버튼이 숨겨지고 rail은 기본 스크롤로 움직입니다.

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## 검색 상자

제안 목록이 있는 검색 필드이며, 리스트 박스 팝업을 사용하는 W3C APG 콤보박스 패턴을 따릅니다. 사용자가 입력하면 `instant` 밀리초 동안 기다린 후 서버에 제안을 요청합니다. action은 그 텍스트를 `DataQuery::q`로 받아 실행되고, 각 행의 제안 텍스트가 돌아옵니다(`suggestion`으로 달리 지정하지 않으면 마스킹되지 않은 첫 번째 텍스트 필드). Enter를 누르거나 제안을 선택하면 페이지 새로 고침 없이 그 텍스트로 결과 화면이 열립니다.

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

키보드: 아래쪽 화살표와 위쪽 화살표는 제안 사이를 이동하며(아래쪽 화살표는 제안을 엽니다), Enter는 강조된 제안이나 입력한 텍스트로 검색하고, Escape는 제안을 닫은 다음 필드를 비웁니다. 라이브 리전이 스크린 리더에 제안의 개수를 알려 줍니다.

결과 화면은 `SearchParams`를 받습니다(`q`이며, 메뉴에서 화면을 열면 비어 있습니다). 서버는 텍스트를 `MAX_SEARCH_QUERY`(100)자로 자르고, 보낼 제안의 개수를 정하며(`limit`으로 달리 지정하지 않으면 8개, 최대 20개), Screen의 접근 권한, Feature의 가드, action의 권한을 검사하고, 모든 제안 호출을 쓰기 속도 제한(`App::write_limit`)에 포함합니다.

## 자유 텍스트

`DataQuery::q`는 자유 텍스트입니다. 텍스트 필드에 이 텍스트가 포함된 행을 대소문자를 구분하지 않고 찾습니다. 터키어의 점 있는 `i`와 점 없는 `i`(`İ`, `ı`)는 `i`와 일치하므로 "istanbul"로 "İSTANBUL"을 찾을 수 있습니다. `#[mask]`가 있는 필드는 검색하지 않으며, 빈 텍스트는 모든 행과 일치하고, 100자보다 긴 텍스트는 무시됩니다. `DataQuery::apply`와 SQL 데이터 계층(바인드 매개변수를 쓰는 `LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...`)은 이를 같은 방식으로 읽습니다.
