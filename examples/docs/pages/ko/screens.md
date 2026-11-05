# Screen과 매개변수

Screen은 경로가 있는 페이지, 또는 다른 Screen 위에 열리는 창입니다. `#[screen]`은 action처럼 접근 권한을 선언합니다.

## 페이지

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)`은 페이지이고 `Screen::window(title)`은 창입니다.
- 여러 번의 `.body(...)` 호출은 여러 컴포넌트를 위아래로 배치합니다.
- Screen은 `Screen` 또는 `Result<Screen>`을 반환합니다.

## 매개변수

Screen의 첫 번째 인수는 매개변수입니다. `#[params]` struct이며, 없으면 `_: ()`입니다. 다른 인수는 action처럼 주입됩니다.

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

매개변수는 사용되는 모든 곳에서 타입이 있습니다. 편집기가 `ProductParams`를 받으므로 `Window::open(product_editor, 7)`은 컴파일되지 않습니다.

`route`가 없는 Screen에는 URL이 없습니다. 링크, 툴바 버튼, 메뉴, 명령에서 창으로 열립니다.

## 경로의 매개변수

경로는 params 필드를 경로 세그먼트로 지정할 수 있습니다. 각 `{name}`은 같은 이름의 필드에 바인드되며 그 필드의 타입으로 읽힙니다.

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42`는 주문 42를 엽니다. 경로에 없는 필드는 이전처럼 JSON 쿼리로 전달됩니다. `/orders/42?params={"tab":"lines"}`.
- 세그먼트는 정수, entity 키(`OrderId`), `String`, `Slug`를 받습니다. 값은 프레임워크가 쓰는 형태로만 파싱됩니다(`007`이나 `+7`이 아니라 `7`). 그 밖의 값은 404로 응답합니다.
- params의 어떤 필드도 가리키지 않는 세그먼트, params가 없는 Screen의 세그먼트가 있는 경로, 세그먼트가 될 수 없는 필드 타입은 컴파일되지 않으며, 오류가 무엇을 바꿔야 하는지 알려 줍니다.
- 여러 세그먼트도 같은 방식으로 동작합니다. `slug: Slug`와 `n: u32`가 있는 `#[screen(route = "/catalog/{slug}/{n}", public)]`입니다.
- 프레임워크가 쓰는 링크는 경로 형태를 사용합니다. `Effect::navigate`, 이동 경로(breadcrumb), 명령, 로그인 후의 `return_to`가 그렇습니다. 세그먼트가 없는 Screen은 `?params={json}`을 유지합니다.
- 세그먼트가 있는 Screen은 채울 방법이 없으므로 메뉴 항목이나 홈 Screen이 될 수 없습니다.
- 접근 권한은 다른 Screen과 마찬가지로 모든 요청에서 검사됩니다.

### Slug

`Slug::from_title("Çay Bardağı")`는 `cay-bardagi`입니다. 소문자 ASCII이고 단어는 `-`로 연결되며, 터키어 문자는 변환되고(`ı`와 `İ`는 `i`로, `ş`는 `s`로, `ğ`는 `g`로, `ç`는 `c`로, `ö`는 `o`로, `ü`는 `u`로), 다른 라틴 문자의 악센트는 제거되며, 그 밖의 것은 제외됩니다. slug는 최대 80바이트입니다. 경로에서는 정규 형태만 파싱되므로 `/catalog/Cay-Bardagi/1`은 404로 응답합니다.

### 찾을 수 없음과 이동됨

레코드가 없으면 `Err(Error::NotFound)`를 반환하십시오. 사용자는 404 페이지를 받습니다. 제품 이름이 바뀐 것처럼 주소가 바뀌었다면 새 주소로 응답합니다. 브라우저는 영구 리다이렉트(301)를 받고, 앱 내 탐색은 페이지 로드 없이 이를 따라갑니다.

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)`는 주소 자체는 유효하게 둔 채 요청 하나를 다른 Screen으로 넘깁니다. 예를 들어 무작위 선택이나 최신 항목이 있습니다. 브라우저는 대상의 정식 경로와 `Cache-Control: no-store`가 있는 `303 See Other`를 받으며, 앱 내 이동은 페이지 로드 없이 이를 따라가고 주소 표시줄에는 대상이 표시됩니다. 대상은 항상 경로가 있는 타입이 지정된 Screen이므로 리디렉션이 앱 밖의 주소로 이어질 수 없으며, 앱이 제공하지 않는 대상은 404로 응답합니다.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## 접근 권한

| 옵션 | 의미 |
|---|---|
| `permission = "..."` | ID에 이 권한이 필요합니다 |
| `public` | 누구나 가능합니다. 세션이 없는 방문자는 쿠키 없이 페이지를 받습니다([공개 페이지](public-pages.md) 참고) |
| `cache = 300` | 공개 Screen 전용: 브라우저와 공유 캐시가 세션이 없는 방문자의 페이지를 이 시간(최대 86400)초 동안 보관할 수 있습니다 |
| `sensitive` | 열면 개인 데이터의 읽기로 기록됩니다 |
| `reason` | 열기 전에 사용자가 사유를 입력해야 합니다([감사, 마스킹, 작업](enterprise.md) 참고) |

페이지는 `Screen`의 `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)`, `.robots(..)`로 검색 엔진과 링크 미리 보기에 자신을 알리기도 합니다. [공개 페이지](public-pages.md#search-engines-and-link-previews)를 참고하십시오.

사용자가 열 수 없는 Screen을 여는 메뉴 항목, 툴바 버튼, 명령은 HTML에서 제외됩니다.

로그인하지 않은 방문자를 위한 Screen은 `public`을 선언합니다. 그 action에는 자체 접근 권한이 필요합니다. 데이터를 나열하는 공개 Screen은 `public` action을 호출해야 하며, 그렇지 않으면 방문자에게 목록이 비어 있습니다.

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

`permission`과 `public`을 둘 다 생략하면 컴파일 오류이므로 Screen이 우연히 열려 있는 일은 없습니다.

## 툴바와 컨텍스트 메뉴

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

`Alt+N` 같은 단축키는 Screen에서 동작하며 팔레트에 표시됩니다. 브라우저가 사용하는 단축키(`Ctrl+P`, `Ctrl+S`, `F5`)는 버튼이 `.override_browser()`를 호출하지 않는 한 바인드되지 않으며, 브라우저가 전달하지 않는 단축키(`Ctrl+N`, `Ctrl+T`, `Ctrl+W`)는 절대 바인드되지 않습니다. 사용할 수 있는 조합은 `Alt+Shift+<letter>`와 `Ctrl+Alt+<letter>`입니다.

## 헤더

`.subtitle(text)`는 제목 아래에 짧은 한 줄을 넣고, `.primary(|t| ...)`는 화면의 주요 작업을 기본 버튼으로 제목 줄 끝에 둡니다. 둘을 함께 쓰면 대부분의 관리 화면에 있는 페이지 헤더가 됩니다.

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary`는 `.toolbar`와 같은 버튼을 같은 단축키와 팔레트 항목으로 받습니다. 도구 모음은 다른 도구를 위해 헤더 아래에 그대로 남습니다. 부제목은 스크린 리더가 읽는 화면 설명입니다(`aria-describedby`). 줄이 좁으면 작업이 제목 아래로 내려가고, 휴대폰에서는 시작 쪽에 맞춰 정렬됩니다. 부제목도 주요 작업도 없으면 화면은 이전처럼 일반 제목으로 그려집니다.

## Feature와 메뉴

Screen은 Feature에 속합니다. Feature의 경로 접두사가 경로 앞에 붙고, Feature의 가드는 Screen이 열리기 전에 실행되며, Feature의 메뉴가 Screen을 나열합니다.

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)`은 `/`가 Screen을 열도록 합니다.

## 오류 페이지

`App::not_found(screen)`과 `App::server_error(screen)`은 앱에 404와 서버 오류를 위한 자체 페이지를 제공합니다.

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- 404 Screen은 알 수 없는 경로와, 파싱되지 않는 경로 값을 포함해 `Err(Error::NotFound)`를 반환하는 모든 Screen에 응답합니다.
- 서버 오류 Screen은 `Error::Internal`, 타임아웃(504), 핸들러의 panic에 응답합니다. 오류의 세부 정보는 서버 로그에만 남으며, Screen은 매개변수를 받지 않고 요청에 대해 아무것도 알지 못합니다.
- 둘 다 올바른 상태 코드와 함께 셸 안에 표시됩니다. 앱 내 탐색은 Screen만 받아 페이지 로드 없이 교체합니다. 프레임워크 엔드포인트와 API는 계속 JSON으로 응답합니다.
- 오류 Screen은 매개변수를 받지 않고, 경로가 필요 없으며, Feature에 속하지 않습니다. `public`으로 만드십시오. 이를 열 수 없는 사용자나 스스로 실패하는 오류 Screen은 내장된 짧은 메시지를 받습니다.

## Screen이 브라우저에 도달하는 방법

첫 요청은 메뉴, Screen, 에셋을 포함한 전체 페이지를 받습니다. 이후 앱 안의 링크는 Screen만 불러와 그 자리에서 교체하므로 메뉴, 열림 상태, 스크롤 위치가 유지됩니다. 뒤로 가기 버튼, 새 탭, 북마크는 일반 링크처럼 동작합니다.
