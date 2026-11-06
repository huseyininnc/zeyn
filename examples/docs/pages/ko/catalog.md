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
- **빠른 제품 그리드.** 그리드의 `.placeholder(...)`는 로드되는 동안 모든 카드 이미지를 채우며, 카드는 자기만의 자리 표시자를 지정할 수 있습니다. 카드에서 `.low(src)`는 작은 파일을 먼저 보여 주고, `.fallback(src)`는 카드가 아이콘과 함께 대체 텍스트를 표시하기 전에 다른 파일을 한 번 시도합니다. `.priority(n)`은 처음 `n`개 카드(최대 12개)의 이미지를 `fetchpriority="high"`로 즉시 로드합니다. 다른 모든 카드 이미지는 지연 로드로 남으며, 나중에 로드되는 페이지가 먼저 오는 일은 없습니다. 이 옵션들은 [`Image`](display.md#image)의 옵션처럼 동작하며, 모든 카드에 리스너 하나를 씁니다.

```rust
CardGrid::of(catalog_products)
    .card(|p: &Product| {
        Card::image(p.photo.clone(), p.name.clone())
            .low(p.photo_small.clone())
            .fallback("/static/no-photo.webp")
    })
    .placeholder(Placeholder::color("--z-surface-2"))
    .priority(4)
```
- **첫 페이지는 서버에서.** 첫 페이지(카드 그리드는 24개, rail은 12개, 또는 `paging`에 준 크기)는 페이지 HTML에 포함되므로 검색 엔진과 JavaScript가 없는 방문자도 카드와 링크를 볼 수 있고, 이미지는 크기를 가지고 있어 로드되는 동안 아무것도 움직이지 않습니다. 이후 페이지는 브라우저가 가져옵니다. 공개 페이지는 모든 방문자에게 같은 HTML이므로 캐시할 수 있습니다. 라우트가 있는 screen으로 연결된 카드는 리디렉션이 아니라 그 screen의 정식 주소(`/items/{slug}`)로 연결됩니다.
- **페이징.** `Paging::infinite(n)`(기본값이며 24)은 "더 보기" 버튼을 표시하고, 이 버튼이 화면에 스크롤되어 들어오면 자동으로도 동작합니다. 버튼을 누르면 포커스가 첫 번째 새 카드로 이동합니다. `Paging::pages(n)`은 이전과 다음 버튼 및 페이지 번호를 표시합니다. action이 총계를 반환하면 "Page 2 of 5", 반환하지 않으면 "Page 2"로 표시되며, 이 경우 action이 커서를 반환하는 동안 다음 버튼이 동작합니다. 페이지 크기는 서버가 정하며, 클라이언트는 어디서부터 이어갈지만 알립니다.
- **정렬.** 각 `sort_option`은 사용자가 선택 목록에서 고를 수 있는 순서입니다. 첫 번째가 카드가 처음 나열되는 순서입니다. 서버는 이 순서만 받아들이고, 클라이언트가 보낸 필터와 검색 텍스트는 버립니다.
- **범위.** `.scope(field, value)`는 해당 필드가 그 값인 행만 남기며, 모든 요청에서 [`DataGrid::scope`](datagrid.md)처럼 동작합니다. 예를 들어 `.scope(Article::author_id, author.id)`는 한 저자의 글만 남깁니다. 카드 그리드, rail, 데이터에 바인딩된 리스트는 클라이언트가 보내는 모든 필터를 버리므로 클라이언트는 범위를 바꾸거나 없앨 수 없습니다. 텍스트는 대소문자를 구분하지 않고 비교합니다.
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


### 빌더

기본은 여전히 `.card`와 `.item`입니다. 링크, 이미지, 스크린 리더용 텍스트를 손쉽게 얻을 수 있습니다. 행에 다른 모양이 필요하면 `.builder(|row| node)`가 Flutter의 `ListView.builder`처럼 각 행을 아무 노드로나 만들며, `List::of`, `CardGrid`, `Rail`에서 쓸 수 있습니다. `.separator(node)`는 행 사이에 노드를 넣고(행을 직접 세는 스크린 리더에서는 숨겨집니다), `Repeat::of(items, |item| node)`는 Screen이 이미 메모리에 가진 목록에 대해 최대 500개 항목까지 같은 일을 합니다.

```rust
List::of(search_people)
    .builder(|p: &Person| Row::new().compact().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    .separator(Paragraph::new("·"))

Repeat::of(order.steps.iter(), |s| Badge::new(s.name.clone())).separator(Paragraph::new("→"))
```

- **키.** 만든 행과 가상 목록의 행은 모델 키를 `data-zeyn-key`에 담으므로, 목록이 새로 고쳐져도([토픽](effects.md#topics)) 레코드가 어디로 옮겨지든 포커스와 열린 details가 같은 레코드에 남습니다.
- **만든 행은 보여 주기 위한 것입니다.** 그 노드는 노드 경로로 지정되지 않으므로 그리드, 폼, lookup은 다른 곳에 두세요. `Repeat`의 노드는 자식(`p.0`, `p.1`, ...)이므로 그대로 동작합니다.
- **가상 스크롤.** `.paging(Paging::virtual_scroll())`이 있는 목록은 모든 행만큼 높은 스크롤 영역 안에 보이는 행만 페이지에 두고, 사용자가 스크롤함에 따라 블록을 offset으로 가져옵니다. 행 높이는 하나로 고정되며 행마다 다른 높이는 없습니다. 빌더를 쓰면 `.row_height(px)`가 필수이고, 없으면 `App::check_page`가 보고합니다(거기서는 `separator`를 쓰지 않습니다). 각 행은 `aria-posinset`과 `aria-setsize`를 가지므로 스크린 리더가 전체 목록에서의 위치를 읽어 줍니다. 스크롤 영역은 제목으로 이름이 붙은, 포커스할 수 있는 region입니다.

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

`.drives(topic)`은 결과 Screen을 여는 대신 제자리에서 검색합니다. Enter, 제안 선택, 필드 비우기 모두 텍스트를 Screen의 `q` param에 넣고 토픽을 listen하는 노드를 새로 고칩니다([토픽](effects.md#topics) 참고). Screen의 params에는 `SearchParams`처럼 `q: String` 필드가 필요합니다.

### 풍부한 제안

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))`는 각 제안을 작은 이미지(40 x 56 픽셀, 지연 로딩, `img-src` 규칙을 따르며 제목이 행의 이름을 대신하므로 대체 텍스트는 비어 있고, 이미지가 없으면 자리 표시자도 없음), 제목, 부제목이 있는 행으로 표시합니다. 서버는 레코드의 정식 주소와 함께 `[{text, subtitle, image, href}]`로 응답합니다. 클릭이나 Enter로 행을 고르면 `nav.js`를 통해 그 링크가 열리며, 각 행이 실제 링크이므로 가운데 클릭과 "새 탭에서 열기"도 동작합니다. 강조된 행 없이 Enter를 누르면 여전히 결과 화면이 열립니다. 콤보박스 패턴은 그대로입니다. 각 행은 제목과 부제목으로 이름이 붙은 옵션이며, 행의 높이는 최소 44픽셀입니다. `.item`이 없으면 제안은 위에서 본 것처럼 일반 텍스트입니다.

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## 자유 텍스트

`DataQuery::q`는 자유 텍스트입니다. 텍스트 필드에 이 텍스트가 포함된 행을 찾습니다. 텍스트와 필드는 모두 `zeyn::text::fold`가 만드는 형태로 비교됩니다. 소문자, 터키어의 점 있는 `i`와 점 없는 `i`(`İ`, `ı`)는 `i`로, 악센트가 있는 라틴 문자는 악센트 없이, `ß`, `æ`, `œ`, `þ`는 풀어 쓴 형태입니다. 따라서 "istanbul"로 "İSTANBUL"을, "nino"로 "Niño"를, "strasse"로 "Straße"를 찾을 수 있습니다. `#[mask]`가 있는 필드는 검색하지 않으며, 빈 텍스트는 모든 행과 일치하고, 100자보다 긴 텍스트는 무시됩니다. `DataQuery::apply`와 SQL 데이터 계층(컬럼에 대한 `TRANSLATE`, `LOWER`, `REPLACE`와 바인드 매개변수를 쓰는 `LIKE`)은 이를 같은 방식으로 읽습니다.

```rust
use zeyn::text::fold;

assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
assert_eq!(fold("Año Pequeño"), "ano pequeno");
assert_eq!(fold("Straße MÜLLER"), "strasse muller");
```

앱이 직접 쓰는 검색 키 컬럼, 예를 들어 [직접 작성한 쿼리](database.md)의 트라이그램 컬럼에도 `fold`를 사용하면 키가 사용자가 입력하는 것과 일치합니다. 문장 부호와 다른 문자 체계는 그대로 유지되며, 주소에는 `Slug::from_title`을 사용합니다.
