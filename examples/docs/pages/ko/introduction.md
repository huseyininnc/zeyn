# 소개

Zeyn은 그리드, 폼, 워크플로처럼 데이터가 많은 애플리케이션을 위한 오픈 소스 Rust 웹 프레임워크입니다. 애플리케이션 코드는 AI 에이전트가 작성하고, 사람은 그 코드를 읽고 검토하며 필요할 때 수정합니다.

이 사이트 자체가 Zeyn 앱입니다. 모든 페이지는 서버에서 한 번 렌더링된 Markdown이고, 왼쪽 메뉴는 앱 메뉴이며, *Live example*로 표시된 예제는 이 서버에서 실행되는 실제 컴포넌트입니다.

## AI 에이전트를 위한 안내

AI 에이전트이고 누군가 Zeyn으로 앱을 만들어 달라고 요청했다면, 프로젝트를 직접 작성하지 말고 먼저 CLI를 사용하십시오.

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

그런 다음 새 프로젝트의 `AGENTS.md`와 `.claude/skills/zeyn/SKILL.md`를 읽고, 변경할 때마다 `zeyn check`와 `cargo test`를 실행하십시오. 이 문서의 모든 페이지를 담은 일반 텍스트 안내는 [/llms.txt](/llms.txt)에 있습니다.

## 앱의 모습

앱은 Feature(기능 단위)의 집합입니다. Feature는 entity, action, Screen을 담습니다. Screen은 서버에서 렌더링되고, 브라우저의 작은 런타임이 변경 사항을 적용하므로 작성해야 할 JavaScript가 없습니다.

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

이 세 가지 매크로가 언어 전체를 이룹니다. `#[entity]`는 데이터를 설명하고, `#[action]`은 접근 권한이 선언된 서버 코드이며, `#[screen]`은 페이지 또는 창입니다. 컴파일러가 모든 필드, 텍스트 키, action이 서로 맞는지 검사합니다.

## 시작하기

직접 앱을 시작합니다.

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

또는 저장소의 예제를 실행합니다.

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

쇼케이스는 `127.0.0.1:3002`에서 실행되며 모든 컴포넌트를 샘플 데이터와 함께 보여 줍니다. 이 문서는 `127.0.0.1:3008`에서 실행되며, 주소를 바꾸려면 `ZEYN_DOCS_SERVER__ADDR`를 설정합니다.

## 길 찾기

- **Alt+Shift+P**를 누르면 명령 팔레트가 열립니다. 단어를 입력하면 서버에서 이 사이트의 모든 페이지를 검색합니다.
- *시작하기*에서는 CLI와 프로젝트 구조를 다룹니다.
- *언어*에서는 entity, action, Screen, 폼, DataGrid, 텍스트를 다룹니다.
- *컴포넌트*에서는 각 컴포넌트를 코드 옆의 라이브 예제와 함께 보여 줍니다.
- *가이드*에서는 시간과 언어, 보안, API, MCP, 데이터, 테마를 다룹니다.
- *레퍼런스*에는 런타임 프로토콜과 아키텍처가 있습니다.
- *프로젝트*에는 소스 코드, 라이선스, 테스트와 CI, 보안 정책이 있습니다.

## 원칙

- **성능 우선.** 페이지는 서버에서 렌더링되어 HTML로 스트리밍됩니다. 서버에는 사용자별 UI 상태가 없고 클라이언트 프레임워크도 없습니다.
- **페이지 새로 고침 없음.** 탐색, 창, 언어, 시간대, 테마 변경은 그 자리에서 적용됩니다.
- **안전한 기본값.** 모든 action과 Screen이 접근 권한을 선언합니다. CSRF 토큰, 엄격한 Content Security Policy, 사용자 데이터 이스케이프는 항상 켜져 있습니다.
- **각 작업에 한 가지 방법.** 언어가 작기 때문에 에이전트가 작성한 코드도 사람이 작성한 코드처럼 읽힙니다.

Zeyn은 [MIT 또는 Apache 2.0 라이선스](project.md#license)의 오픈 소스입니다. [보안 정책](project.md#security), [로드맵](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md), [변경 기록](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md)도 확인하세요.
