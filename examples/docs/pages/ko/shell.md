# 셸과 명령

셸은 모든 Screen을 둘러싼 틀입니다. 메뉴, 언어, 테마, 시간대 선택기, 토스트, 명령 팔레트가 있습니다. 데스크톱 도구처럼 동작하는 앱은 메뉴 막대, 도구 상자, 속성 검사기, 상태 표시줄, 도킹 패널을 추가합니다.

## 메뉴

메뉴는 Feature의 `.menu(...)` 호출에서 만들어집니다. 사용자가 열 수 없는 항목은 HTML에서 제외되며, 가드가 사용자를 거부하는 Feature는 아무것도 표시하지 않습니다. 현재 Screen의 링크에는 `aria-current="page"`가 붙고 브랜드의 은은한 색조 위에 그려집니다.

`.item(...)` 뒤의 `.icon(Icon::...)`은 항목의 레이블 앞에 아이콘을 둡니다. 그룹의 항목에 아이콘이 있으면 그룹의 레이블은 자기 아이콘 없이 그려집니다. 사이드 메뉴는 앱의 로고와 이름으로 시작합니다. `App::logo("/static/logo.svg")`(다크 테마에는 `App::logo_dark(...)`)는 앱이 제공하는 마크를 설정하고, 그렇지 않으면 셸이 `--z-brand` 색의 둥근 사각형에 이름의 첫 글자를 그립니다. 셸은 중립적으로 유지되며, 브랜드 색은 현재 항목, 기본 버튼, 포커스 링, 로고를 표시합니다.

아이콘은 20 px 격자에 1.5 px 둥근 선으로 그린 82개 그림의 한 세트(`Icon::ALL`)에서 오며, 현재 텍스트 색으로 16 또는 20 px 크기로 표시됩니다. 각 아이콘은 `/_zeyn/icons.css`의 CSS 마스크입니다. 이 파일은 `crates/zeyn/assets/icons/*.svg`에서 생성된 약 25 KB(압축 시 3 KB)의 캐시되는 스타일시트이므로, 아이콘 하나의 비용은 HTML의 클래스 하나이고 스크립트에서는 비용이 없습니다.

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

## 사이트 셸

작업 Screen은 측면 메뉴가 있는 앱 셸에서 열립니다. 쇼핑몰이나 제품 카탈로그 같은 공개 페이지는 대신 사이트 셸에서 열 수 있습니다. 브랜드 링크로 쓰이는 앱 이름, 가로 메뉴, 좁은 화면의 메뉴 버튼, 선택적 action, 바닥글이 있습니다. Feature가 이를 선택하며, 다른 Feature의 Screen은 앱 셸을 유지하므로 한 앱에 둘 다 공존합니다.

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **메뉴.** 상단 메뉴는 사이트 셸에 있는 모든 Feature의 메뉴를 나열합니다. 항목이 하나뿐인 그룹은 일반 링크이고, 더 큰 그룹은 링크를 여는 버튼입니다(W3C APG 디스클로저 내비게이션 패턴). Enter, Space, 클릭은 그룹을 토글하고, 아래쪽 화살표는 그룹을 열어 첫 번째 링크로 이동하며, 위쪽 화살표, 아래쪽 화살표, Home, End는 그룹의 링크 사이를 이동하고, Escape는 그룹을 닫고 버튼으로 돌아가며, 그룹 밖으로 Tab하거나 바깥을 클릭하면 닫힙니다. 왼쪽과 오른쪽 화살표는 상단 항목 사이를 이동합니다.
- **좁은 화면.** 1024 px 미만에서는 메뉴가 `aria-expanded`가 있는 메뉴 버튼 뒤로 접히며, Escape로 닫습니다. 스크립트가 없으면 메뉴가 열린 채로 유지되므로 접근할 수 없게 되는 것이 없습니다.
- **헤더 슬롯.** 상단 바에는 논리적인 이름을 가진 앱 바처럼 어떤 컴포넌트든 받는 슬롯이 넷 있습니다. `start(node)`는 브랜드 앞, `center(node)`는 메뉴와 끝 사이, `end(node)`는 바의 끝, `bottom(node)`는 그 아래의 전체 너비 행(예: chips)입니다. 각 슬롯은 여러 노드를 순서대로 담으며, start와 end는 오른쪽에서 왼쪽으로 쓰는 언어에서 자리를 바꿉니다. 슬롯의 컴포넌트는 사용자의 언어, 권한, 마스크로 렌더링됩니다. `SearchBox`처럼 자체 action으로 데이터를 불러오는 컴포넌트도 그곳에서 동작하며, Feature의 가드와 action의 권한을 기준으로 검사됩니다. 그리드와 폼은 Screen에 속합니다. 휴대폰에서는 start와 end 슬롯이 바에 남고, center 슬롯은 앱이 정한 곳으로 갑니다. `center_on_phone(CenterOnPhone::Row)`(기본값)는 바 아래의 행에 두고, `CenterOnPhone::Icon`은 이를 여는 돋보기 버튼 뒤에 두며(W3C APG 디스클로저) 포커스를 그 안으로 옮깁니다. Escape는 이를 닫고 버튼으로 돌아갑니다. `center_button(icon, label)`은 그 버튼을 바꿉니다. 휴대폰 레이아웃은 스크립트가 실행되기 전에 CSS로 정해지므로 페이지를 불러오는 동안 헤더가 움직이지 않습니다.
- **Action.** `sign_in()`은 앱에 로그인 페이지가 있으면 방문자에게 로그인 링크를, 로그인한 사용자에게 로그아웃 링크를 표시합니다. `language()`와 `theme()`는 앱 셸과 같은 선택기를 표시합니다. 이들은 end 슬롯 뒤에 오며 휴대폰에서는 메뉴로 접힙니다.
- **하단 바.** `bottom_nav([...])`는 휴대폰과 태블릿(1024 px보다 좁은 화면)에서 화면 아래에 고정되는 링크를 최대 다섯 개 추가하며, 사람들이 가장 자주 가는 곳을 위한 것입니다. 다섯 개를 넘으면 컴파일되지 않습니다. 각 항목은 레이블, 라우트가 있는 화면, 선택적 아이콘을 가진 `BottomNavItem`입니다. 표시 중인 페이지의 링크에는 `aria-current="page"`가 있고(`nav.js`가 최신으로 유지), 사용자가 열 수 없는 화면으로의 링크는 제외되며, 바는 `env(safe-area-inset-bottom)`으로 홈 인디케이터를 피하고, 바가 아무것도 가리지 않도록 페이지 아래에 여백이 생깁니다. 나머지를 위해 상단 메뉴는 그대로 있습니다. 바는 스크립트가 필요 없습니다.
- **바닥글.** Screen으로 가는 링크(`link`, 열 수 없는 사용자에게는 제외됨) 또는 주소(`url`: 경로, `https:`, `http:`, `mailto:`. `App::check`는 다른 스킴과 등록되지 않은 Screen으로의 링크를 거부합니다)의 열, 그리고 한 줄의 텍스트입니다.

사이트 셸 Screen과 앱 셸 Screen 사이의 이동은 페이지 로드 없이 `nav.js`를 거칩니다. 부분 응답이 어느 셸에 속하는지 알려 주고(`x-zeyn-shell`), 이것이 현재 프레임과 다르면 런타임은 언어 변경과 같은 방식으로 전체 프레임을 교체하고 다른 셸의 스타일시트와 스크립트를 불러옵니다. `/_zeyn/route/{screen}`으로 가는 링크(이동 경로의 링크)도 같은 방식으로 따라갑니다.

## 로그인 셸

로그인 페이지는 자기만의 프레임을 갖습니다. `Shell::auth()`에는 메뉴, 상단 막대, 명령 팔레트가 없습니다. 앱의 로고(`App::logo`)와 이름이 너비 400 px의 카드 하나 위에 놓이고, 언어와 테마 선택기는 페이지 아래쪽에 있어 둘 다 로그인 전에 쓸 수 있습니다. `.split()`은 1024 px부터 한쪽 절반에 폼을, 다른 쪽에 `--z-brand` 색의 브랜드 패널을 둡니다. 더 좁은 화면은 가운데 정렬된 카드를 보여 줍니다. 휴대폰에서는 카드가 곧 페이지가 되고 입력과 버튼의 높이가 44 px입니다. 스타일(`auth.css`, 3.5 KB)은 이 페이지에서만 불러옵니다.

`zeyn::accounts::ui`의 모든 로그인 화면(로그인, 가입, 비밀번호 찾기와 재설정, 2단계 로그인, 복구 코드, 잠금, 확인, 로그아웃)은 이 셸에서 열리고, 계정과 관리 화면은 앱 셸을 유지합니다. 직접 만든 로그인 화면은 자기 Feature에서 이 셸을 얻습니다:

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

로그인 폼은 사용자 이름을 `Control::Username`(`autocomplete="username"`)으로, 비밀번호를 `current-password`로 채우고, 비밀번호 표시 토글을 필드 안의 눈 아이콘으로 보여 주며, Caps Lock이 켜져 있는 동안 그 아래에 "Caps Lock이 켜져 있습니다"라고 알립니다(polite 라이브 영역).

## 화면 안의 테마 선택기

`ThemePicker::new()`는 설정 페이지의 모양 섹션처럼 화면 안에 테마 선택기를 둡니다. `App::themes`의 테마를 제공하며(앱이 기기를 따르면 시스템 포함) 사용 중인 테마를 보여 줍니다.

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

셸의 선택기와 같은 컨트롤입니다. 선택은 페이지를 다시 불러오지 않고 바로 적용되고, `zeyn_theme` 쿠키에(로그인한 사용자는 뷰 저장소에도) 저장되며, 페이지의 다른 선택기에도 함께 표시됩니다. 앱이 선택을 제공하지 않으면(`theme_picker(false)` 또는 테마 하나) 아무것도 그리지 않습니다.

## 앱 헤더

`App::header(|h| ...)`는 앱 셸의 화면 위에 사이트 헤더와 같은 슬롯(`start`, `center`, `end`, `bottom`)을 가진 막대를 둡니다. 가운데에는 검색 상자를, 끝에는 알림 링크를 둡니다. `sign_in()`, `language()`, `theme()`도 여기서 쓸 수 있지만, 선택기는 이미 사이드 메뉴에 있습니다.

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

준비된 세 부분이 막대를 앱 프레임으로 만듭니다:

- `breadcrumb()`은 막대를 메뉴 그룹과 현재 화면의 제목으로 시작하며, `nav.js`가 이동할 때마다 이를 다시 씁니다.
- `search()`는 명령 팔레트를 여는 검색 필드를 단축키와 함께 추가하고, 팔레트 버튼은 사이드 메뉴에서 빠집니다. 휴대폰에서는 아이콘 버튼입니다.
- `account()`는 막대를 둥근 버튼 위의 사용자 이니셜로 끝냅니다. 이 버튼은 대화 상자(W3C APG 비모달 대화 상자. Escape와 닫기 버튼으로 닫힘)를 여는데, 그 안에는 로그인한 사람, 테마와 언어 선택기, 로그아웃 링크가 있고 방문자에게는 로그인 링크가 있습니다. 선택기는 사이드 메뉴에서 빠집니다.

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

화면을 스크롤해도 막대는 위에 남고, 화면은 페이지를 다시 불러오지 않고 그 아래에서 바뀝니다. 휴대폰에서는 메뉴 버튼 아래에 놓이고 가운데 슬롯은 한 줄을 따로 차지합니다. 막대의 컴포넌트는 화면의 컴포넌트처럼 사용자 기준으로 그려지며, 검색 상자는 자기 액션으로 제안을 불러오고 서버가 그 액션의 권한을 확인합니다. `App::header`가 없으면 앱 셸에는 상단 막대가 없습니다.

## 명령

명령은 레이블, 단축키, 동작과 함께 Rust로 한 번 선언합니다. Feature에 등록하면 팔레트가 이를 나열하고 단축키는 모든 페이지에서 동작합니다.

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| 생성자 | 실행하는 것 |
|---|---|
| `Command::open(id, label, screen, params)` | Screen을 창으로 엽니다 |
| `Command::navigate(id, label, screen, params)` | Screen으로 이동합니다 |
| `Command::effect(id, label, effect)` | 토스트 같은 브라우저의 effect |
| `Command::action(id, label, action, input)` | 서버의 action. 입력은 브라우저로 전달되지 않습니다 |

`App::check`는 읽을 수 없는 단축키, 두 명령이 공유하는 단축키, 팔레트가 자신을 위해 확보한 단축키를 보고합니다.

## 명령 팔레트

**Alt+Shift+P**는 기본적으로 팔레트를 엽니다(`App::palette_shortcut`으로 변경할 수 있으며, 의도적으로 `Ctrl+Shift+P` 같은 브라우저 키로도 변경 가능). 이는 앱의 명령, 현재 Screen의 툴바와 컨텍스트 메뉴, 메뉴의 Screen을 검색하는 ARIA 콤보박스입니다. 최근에 사용한 명령이 먼저 나옵니다. 포커스가 필드에 없을 때는 `/`로도 열리고, `App::header(|h| h.search())`의 검색 필드로도 열립니다. 결과는 자기 위치("Go to · Sales", 화면 제목, "Commands") 아래에 묶이며, 각각 아이콘, 굵게 표시된 일치 글자, 끝의 단축키를 가집니다. 메뉴 항목은 페이지 안에서 바로 맞춰 보므로 첫 키 입력부터 찾아집니다.

Feature는 여기에 서버 검색을 추가할 수 있습니다. 사용자가 입력하면 팔레트가 action을 호출하고 그 결과를 명령 아래에 나열하며, 하나를 선택하면 페이지가 그 자리에서 열립니다. 이 사이트는 이 방식으로 자체 페이지를 검색합니다.

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

서버는 사용자가 실행할 수 있는 모든 검색을 Feature의 가드, action의 권한, 인터셉터와 함께 실행하고, 쿼리를 100자에서 자르고, 앱을 벗어나는 결과를 제외하며, 최대 20개를 반환합니다.

## 메뉴 막대, 도구 상자, 상태 표시줄

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar`는 ARIA menubar 패턴을 따르며 팔레트와 같은 명령을 담습니다.
- `Toolbox`는 토글 버튼의 세로 그룹이며, 선택한 도구는 `zeyn:tool` 이벤트로 전송됩니다.
- `PropertyInspector::new(title, form)`은 `.section(...)` 그룹으로 나뉜 폼으로 선택한 객체를 편집합니다.
- `PropertyInspector::island(name)` 은 아일랜드에서 선택한 것을 폼 없이 편집합니다([Island inspector](#island-inspector) 참고).
- `StatusBar`에는 라이브 리전이 있으므로 여기에 보낸 메시지를 스크린 리더가 읽습니다.

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

아일랜드는 `detail: { title, schema, values }`와 함께 `zeyn:inspect`를 보냅니다. `schema`는 직렬화한 `FormSchema`입니다. 필드 종류는 `text`, `multiline`, `integer`, `decimal`, `length`, `color`, `boolean`, `choice`, `expression`이며 각각 `required`, `min`, `max`, `max_len`, `default`, `help`를 받습니다.

인스펙터는 모델 폼의 컨트롤로 필드를 그리고 이름, 레이블, 값을 HTML이 아닌 텍스트로 채웁니다. 필드를 벗어나거나 Enter를 누르거나 선택기를 닫으면 값을 검사하고 아일랜드 루트로 `detail: { name, value }`가 담긴 `zeyn:property`를 보냅니다. 잘못된 값은 오류를 보여 주고 보내지 않습니다. 필드가 200개를 넘는 스키마는 무시됩니다. `FieldSpec::check`는 서버에서 같은 검사를 합니다.

## 도킹 패널

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

사용자는 탭을 영역 사이로 끌고, 포인터나 키보드로 영역 크기를 조절하고, 패널을 닫을 수 있습니다. 메뉴의 `MenuList::panels`가 패널을 다시 가져옵니다. 레이아웃은 앱의 뷰 스토어에 사용자와 Screen별로 저장되며, 앱에 스토어가 없으면 브라우저에 저장됩니다.
