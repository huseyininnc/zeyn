# 데이터 그리드

상태: `zeyn::DataGrid`, `zeyn::server`의 그리드 엔드포인트, `assets/zeyn/*.js`(`/_zeyn/zeyn.js`로 제공됨)로 구현되어 있습니다.

그리드는 서버에서 렌더링됩니다. 행은 `DataQuery`를 받아 `Page<E>`를 반환하는 action에서 가져옵니다. 브라우저 런타임은 페이지를 요청하고, 사용자가 하는 일(정렬, 필터, 레이아웃, 편집, action)을 보내며, 돌려받은 HTML을 제자리에 넣습니다. 모든 호출은 서버에서 권한을 검사하며, 클라이언트가 보내는 어떤 것도 신뢰하지 않습니다.

```rust
DataGrid::of(search_products)
    .mode(DataMode::Server)
    .paging(Paging::pages(25))
    .column(Product::code).width(120).pin_start().filter()
        .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
    .column(Product::name).flex(1).filter().editable()
    .column(Product::unit).filter().editable().badge(|u| match u {
        Unit::Kg => Tone::Info,
        _ => Tone::Neutral,
    })
    .column(Product::price).filter().editable().avg().max()
    .column(Product::stock).filter().sum().progress(120)
    .column(Product::active).filter().check_icon().editable()
    .edit_with(save_product)
    .row_action(t!("stock.restock"), restock_product, |p| p.id)
    .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
    .row_menu(|p| ContextMenu::new()
        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
        .action(t!("stock.restock"), restock_product, p.id))
    .exportable()
```

## 실행 시점에 정해지는 열

`DataGrid::dynamic(action)`은 SQL 미리보기나 데이터 소스 탐색기처럼 쿼리를 실행한 뒤에야 열을 알 수 있는 결과를 보여 줍니다. action은 `DataQuery`를 받아 `DynamicPage`를 반환합니다. 여기에는 열(`DynamicColumn { name, label, kind }`)과 한 페이지의 행이 들어 있고, 각 행은 열 순서대로 된 JSON 배열이거나 열 이름으로 된 JSON 객체입니다.

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

grid는 페이지마다 열을 만들고 타입이 있는 grid의 코드로 행을 그리므로 페이지 나누기, 가상 스크롤, 열 가상화, 키보드 사용, 복사, 열 크기 조절, 이동, 숨기기가 똑같이 동작합니다. `DynamicKind`는 열이 값을 읽고 보여 주는 방식을 고릅니다: `Text`, `Integer`(사용자 로캘에 맞게 자릿수 구분), `Decimal`, `Bool`(사용자 언어로 예와 아니요), `Date`, `Time`, `DateTime`(사용자 시간대), `LocalDateTime`. 종류에 맞게 읽히지 않는 값은 텍스트로 표시됩니다. 각 열은 종류에 맞는 너비로 시작하므로(텍스트 180 px, 숫자 130, 날짜 120) 넓은 결과는 가로로 스크롤됩니다.

action이 반환하는 모든 것은 사용자 데이터로 다룹니다. 열 이름, 레이블, 셀은 다른 값처럼 이스케이프됩니다. 서버는 최대 `MAX_DYNAMIC_COLUMNS`(200)개의 열과 쿼리의 페이지 크기만큼의 행(`DataQuery::limit`, 최대 500)만 남기고, 이름과 레이블은 `MAX_COLUMN_NAME`(200)자, 텍스트 셀은 `MAX_CELL_TEXT`(1,000)자에서 자릅니다. 정렬 키, 필터, 합계, 그룹은 action이 실행되기 전에 쿼리에서 제거되고 헤더에는 정렬 버튼이 없으므로 action은 클라이언트에게서 열 이름을 받지 않습니다. 접근 권한은 다른 grid와 마찬가지로 action의 것입니다.

빈 이름은 `col_<순번>`이 되고 반복되는 이름에는 `_2`, `_3`이 붙습니다. 객체 행은 여전히 action 자신의 이름으로 읽습니다. 한 페이지는 최대 `MAX_PAGE_CELLS`(50,000)개의 셀과 `MAX_PAGE_TEXT`(8 MB)의 셀 텍스트만 유지합니다. 각 텍스트 셀이 텍스트 예산을 똑같이 나누므로 넓은 페이지는 더 짧은 텍스트를 보여 주고, 셀 예산을 넘는 행은 페이지의 `next`가 가리키는 다음 페이지로 옵니다. 중첩된 JSON 값은 자기 몫까지만 기록됩니다. action에서 403 MB를 차지한 500행 200열 1,000개 이모지 응답은 action 자신의 응답 외에 메모리를 더 쓰지 않았고 4.6 MB를 보냈습니다.

`DynamicColumn::masked(MaskStyle::LastFour)`는 페이지를 읽자마자 열을 마스킹하므로 원래 값은 HTML, 로그, AI 에이전트에 절대 닿지 않으며 누구도 드러낼 수 없습니다. `DynamicGrid::sensitive(model, key)`는 보여 준 각 행을 `key` 열을 키로 한 `model` 읽기로 감사 기록합니다. `sensitive` 엔티티의 행과 같습니다. 이것들이 없으면 결과가 무엇을 보여 줄지는 action이 정합니다.

열이 바뀌면 브라우저는 이전 열로 보관하던 행을 버립니다. 저장된 뷰는 그 뷰를 만든 열에만 쓰입니다.

헤더에는 열의 지문(`data-zeyn-shape`)이 있습니다. 브라우저는 페이지를 요청할 때마다 이를 보내고, 서버는 열이 바뀌었을 때만 새 헤더를 보냅니다. 예를 들어 `.listens(topic)`이 새 쿼리로 grid를 다시 불러온 뒤입니다. 행은 읽기 전용입니다. 동적 grid에는 편집, 행 작업, 선택, 상세, 그룹화, 내보내기가 없습니다.

## 열 합계

`sum()`과 `avg()`는 정수와 금액에, `min()`과 `max()`는 날짜, 시간, 순간에도, `count()`는 모든 열에 사용할 수 있습니다. 다른 타입에 쓰면 컴파일되지 않습니다. 합계는 페이지만이 아니라 필터와 일치하는 모든 행을 대상으로 합니다.

`DataQuery::apply`는 메모리 데이터에 대해 이를 계산합니다. SQL로 페이징하는 데이터 계층은 `query.aggregates`를 읽고 하나의 집계 쿼리에서 `Page::with_total(field, op, value)`를 추가합니다. 그리드는 열이 선언한 합계만 유지하므로 클라이언트는 다른 합계를 요청할 수 없습니다. 합계는 서버에서 사용자의 언어로 서식화됩니다. 정수의 평균은 소수 둘째 자리까지(`Value::Decimal`) 표시되고, 금액의 평균은 0에서 멀어지는 방향으로 반올림하여 가장 작은 단위에 맞춥니다.

## 필터와 정렬

`filter()`는 필터 행에 필터를 추가하며, 입력은 열의 타입에 따라 선택됩니다.

| 열 | 필터 | 쿼리 |
|---|---|---|
| 텍스트 | 포함, 시작, 일치 | `contains`, `starts_with`, `equals` |
| 정수, 금액 | 시작, 끝 | `from`, `to`를 `1234.56` 형식으로 |
| 날짜 | 시작, 끝 | `from`, `to`를 `2026-03-31` 형식으로 |
| 순간 (`DateTime`) | 시작, 끝 (날짜) | 사용자의 시간대에서 하루 전체 |
| 시간 | 시작, 끝 | `14:30` |
| Choice | 옵션 중 아무거나 선택 | `any_of` |
| 예/아니요 | 전체, 예, 아니요 | `is` |

값은 사용자의 언어와 상관없이 고정된 기계용 형식으로 전달됩니다. 이전의 `{ "field", "contains" }` 형태도 여전히 동작합니다. 서버는 `filter()`가 없는 열의 필터와 그리드에 없는 열의 정렬을 버리며, 필터(32개), 필터 텍스트(200자), 선택지(100개), 정렬 키(4개)에 상한을 둡니다. 읽을 수 없는 값은 무시됩니다. 날짜 필터의 시간대는 서버에 있는 사용자의 설정에서 가져옵니다.

헤더에서 Shift+클릭하면 그 열이 정렬 키에 추가됩니다. 헤더의 숫자는 순서를 보여 줍니다.

## 열: 고정, 크기 조절, 숨김, 이동

`pin_start()`와 `pin_end()`는 그리드가 가로로 스크롤되는 동안 열을 한쪽 가장자리에 유지합니다(한쪽에 최대 세 개). `hidden()`은 사용자가 표시할 때까지 열을 제외합니다. 모든 헤더에는 정렬, 고정, 왼쪽과 오른쪽으로 이동, 숨김, 필터, 열 선택기, 초기화가 있는 열 메뉴가 있습니다(버튼, 오른쪽 클릭, Shift+F10, 컨텍스트 메뉴 키). 헤더의 가장자리는 끌거나 화살표 키(더 큰 단계는 Shift)로 열 크기를 조절합니다. 헤더를 같은 고정 그룹의 다른 헤더 위로 끌면 열이 이동하며, 짧게 누르면 여전히 정렬됩니다. 이 모든 것은 오른쪽에서 왼쪽 방향에서도 동작합니다.

## 저장된 뷰

사용자의 레이아웃(순서대로 표시된 열, 너비, 고정), 정렬, 필터가 `DataGridView`를 이룹니다. `App::datagrid_views(store)`를 사용하면 테넌트, Screen, 그리드별로 사용자마다 서버에 저장되며, 페이지는 첫 페이지가 이미 정렬되고 필터링된 상태로 그 뷰와 함께 돌아옵니다. 데이터베이스에는 `DataGridViewStore`를 구현하십시오. `MemoryViewStore`는 개발과 테스트용입니다. 스토어가 없으면 브라우저가 `localStorage`에 뷰를 보관합니다. 서버는 받은 모든 뷰를 그리드 자체의 열과 제한에 맞게 정리합니다.

## 셀

| 메서드 | 열 | 표시 |
|---|---|---|
| `badge(\|v\| Tone)` | `#[choice]` enum | 옵션별 색상 배지 |
| `progress(max)` | 숫자, 금액 | 막대와 숫자 |
| `link(\|row\| Effect)` | 모두 | effect를 실행하는 링크로 표시된 값 |
| `check_icon()` | `bool` | 체크 표시 또는 대시, 스크린 리더를 위한 문구 포함 |
| `relative_time()` | `DateTime` | "3 minutes ago", 정확한 시각은 툴팁 |
| `cell(renderer)` | 모두 | 직접 작성한 HTML (`CellRenderer<T>`) |

`CellRenderer`는 사용자 데이터를 이스케이프해야 합니다. 내보내기와 복사는 일반 값을 사용합니다.

## 행 action, 메뉴, 일괄 action

`row_action(label, action, |row| input)`은 각 행에 버튼을 추가합니다. `row_menu(|row| ContextMenu)`는 컨텍스트 메뉴를 추가합니다. `Screen::context_menu(...)`는 Screen의 빈 공간에 하나를 추가합니다. action을 실행하는 항목은 그 action의 권한이 있는 사용자에게만 표시됩니다.

행 항목이 실행될 때 클라이언트는 행 키와 항목만 보냅니다. 서버는 그리드의 action으로 행을 다시 불러오고, 그 행에서 항목을 만들고, action의 권한을 검사하고, 인터셉터를 실행한 다음 action을 실행합니다. 그 행에 더 이상 존재하지 않는 항목(예를 들어 비활성화된 항목)은 거부됩니다. 입력은 클라이언트에서 오지 않습니다.

`bulk_action(label, action, Model::key)`은 선택 막대에 버튼을 추가합니다. action은 키 필드의 타입인 `Vec<K>`를 받으며, 다른 것은 컴파일되지 않습니다. "일치하는 모든 행"을 선택하면 서버가 사용자의 필터로 최대 10,000개까지 스스로 키를 수집합니다. 하나씩 선택한 키는 클라이언트에서 오므로 action은 API action처럼 각 레코드를 검사해야 합니다.

브라우저는 그 밖의 모든 곳에서 자체 컨텍스트 메뉴를 유지합니다. `App::browser_context_menu(false)`는 텍스트 입력과 편집 가능한 텍스트를 제외하고 이를 끕니다. 이것은 아무것도 보호하지 않습니다. 사용자는 여전히 페이지 소스를 읽거나, 개발자 도구를 열거나, 스크립트를 끌 수 있습니다. 또한 맞춤법 검사, 링크 복사, 번역, 보조 기술을 사용할 수 없게 됩니다. 기본값을 유지하십시오.

## 인라인 편집

`edit_with(save)`는 `Valid<E>`를 받는 action(폼과 같은 규칙)과 열의 `editable()`과 함께 사용합니다. 키, 읽기 전용 필드, 행 버전은 편집 가능해지지 않습니다. 편집 가능한 셀에서 F2 또는 더블 클릭하면 행 편집이 시작되고, Enter는 저장하고, Escape는 취소하며, Ctrl+Z 또는 토스트 버튼은 마지막 저장을 되돌립니다.

서버는 키로 행을 다시 불러오고 `Form::read`를 통해 편집 가능한 열과 행 버전만 적용하므로, 값은 폼처럼 읽히고 검증됩니다. 다른 열의 값은 `403`으로 거부됩니다. 오래된 버전은 `409`로 돌아오며, 검증 오류는 `422`로 돌아와 해당 셀에 표시됩니다.

## 마스터 디테일

`detail(|row| (screen, params))`는 모든 행을 그 행으로 만든 Screen으로 확장할 수 있게 하며, 사용자가 열 때만 불러옵니다.

```rust
DataGrid::of(search_today)
    .column(Order::id)
    .detail(|o| (order_lines, OrderParams { id: o.id }))

#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        DataGrid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .column(OrderLine::product)
            .column(OrderLine::amount).sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}
```

매개변수는 Screen(`ScreenDef::Params`)이 타입을 정하므로 잘못된 매개변수는 컴파일되지 않습니다. 디테일은 완전한 Screen입니다. 폼, 중첩된 그리드, 자체 디테일이 있는 그리드가 될 수 있으며 깊이에 제한이 없습니다. 확장 열은 그 Screen을 열 수 있는 사용자에게만 표시되며, 서버는 디테일을 불러올 때 Screen의 권한을 다시 검사합니다. 매개변수는 창의 경우와 마찬가지로 브라우저를 거쳐 전달되므로, 디테일 Screen은 사용자가 지정한 레코드를 볼 수 있는지 검사해야 합니다.

`scope(field, value)`는 모든 요청에서 `field`가 `value`와 같은 행으로만 그리드를 제한합니다. 필터는 `DataQuery::filters`의 맨 앞에 오며 클라이언트가 제거할 수 없으므로, 중첩된 그리드는 브라우저가 무엇을 보내든 부모 행에 머뭅니다. 필터로 SQL을 만드는 데이터 계층은 이를 다른 필터와 똑같이 읽습니다(숫자와 날짜는 같은 경계의 `Filter::range`, 텍스트는 `equals`, choice는 `any_of`, 예/아니요는 `is`, 빈 값은 `blank`).

확장 버튼, 그 위에서의 Enter, 행에서의 오른쪽과 왼쪽 화살표가 디테일을 열고 닫습니다. 닫으면 제거되므로 다음에 열 때 새로 불러옵니다.

가상 그리드는 어느 위치든 한 번에 도달할 수 있도록 모든 행의 높이를 고정하며, 확장 버튼을 표시하지 않습니다. 디테일이 있는 그리드에는 페이지 방식이나 무한 페이징을 사용하십시오.

## 그룹화

`group_by(field)`는 서버에서 행을 그룹화합니다. 다시 호출하면 각 그룹 안에서 그룹화합니다.

```rust
DataGrid::of(search_today)
    .paging(Paging::pages(20))
    .group_by(Order::city)
    .group_by(Order::status)
    .column(Order::amount).sum().avg()
```

그리드는 그룹마다 열 레이블, 값, 행 수, 그리고 그 그룹에 대한 모든 열 합계를 해당 열 아래에 보여 주는 헤더 행을 표시합니다. 그룹을 열면(그 버튼 또는 오른쪽과 왼쪽 화살표) 하위 그룹이나 행을 한 번에 한 페이지씩 불러오며, 더 남아 있으면 "Load more" 행이 나타납니다. 닫으면 제거됩니다. 최상위 수준은 그룹 단위로 페이징하므로 페이저는 그룹을 셉니다. 바닥글은 일치하는 모든 행에 대한 합계를 유지합니다. 그룹 안의 행은 편집, 행 action, 메뉴, 선택, 디테일 등 나머지 모든 것을 유지합니다.

데이터 계층은 `DataQuery::group`에 응답합니다. 이것이 설정되면 `Page::groups`(그룹별 값, 개수, 쿼리가 요청한 합계)를 반환하며, 그 필드의 정렬 키 방향으로 값 기준으로 정렬하고, 행처럼 페이징하며, `total`은 그룹을 셉니다. `DataQuery::apply`는 메모리 데이터에 대해 이를 처리하고, SQL은 `GROUP BY`를 사용합니다. `group`은 그리드가 설정하며 클라이언트에서 읽지 않습니다. 클라이언트는 자신이 여는 그룹 값의 경로만 보내고, 서버는 그 길이를 검사하여 각 값을 사용자의 필터 앞에 오는 해당 그룹 열의 필터로 바꿉니다(빈 그룹은 `blank`).

## 트리 데이터

`tree(Model::parent)`는 선택적 부모 키로 행을 트리로 보여 줍니다. 부모가 없는 행이 맨 위에 오고, 행의 자식은 그 행이 열릴 때 한 번에 한 페이지씩 최대 32단계까지 불러옵니다.

```rust
DataGrid::of(search_employees)
    .tree(Employee::manager_id)
    .column(Employee::name)
    .column(Employee::salary).sum()
```

첫 번째 열에 들여쓰기와 버튼이 있습니다. 어느 행에 자식이 있는지 알기 위해 그리드는 페이지마다 action에 한 번 더 요청합니다. 부모가 그 페이지의 키 중 하나인 행을 부모 필드로 그룹화하여 가져옵니다(키에 대한 `Filter::any_of`와 `DataQuery::group`). 필터와 정렬은 모든 수준에 적용되며, 바닥글 합계는 일치하는 모든 행을 대상으로 합니다. 트리 그리드는 그룹화하지 않으며 행 디테일을 표시하지 않습니다. 인라인 편집 후에도 행은 트리의 자리를 유지합니다.

가상 그리드는 모든 행의 높이가 고정되어 있으므로 그룹화도 트리도 지원하지 않습니다.

## 셀: 탐색, 복사, 붙여넣기

행에서 오른쪽 화살표(또는 셀 클릭)를 누르면 스프레드시트처럼 셀 커서가 놓입니다. 화살표 키로 이동하고, Shift와 화살표(또는 Shift+클릭)로 범위를 넓히며, Home과 End는 행의 양 끝으로, Ctrl+Home과 Ctrl+End는 그리드의 첫 번째와 마지막 셀로 이동합니다. Tab과 Shift+Tab은 행을 따라 다음 행으로 이동하며 그리드의 양 끝에서는 그리드를 벗어납니다. 첫 번째 셀에서 왼쪽 화살표를 누르면 행으로 돌아갑니다. Enter 또는 F2는 편집 가능한 셀을 편집하고, Enter는 셀에 하나뿐인 버튼이나 링크를 누르며, Escape는 범위를 해제합니다. 범위 안의 셀에는 `aria-selected`가 붙습니다.

셀에서 Ctrl+C를 누르면 범위가 탭으로 구분된 텍스트로 복사되며, 값은 사용자에게 보이는 대로입니다. 행에서 Ctrl+C를 누르면 여전히 선택한 행이 복사됩니다.

셀에서 Ctrl+V를 누르면 스프레드시트의 탭으로 구분된 텍스트가 커서 위치부터 붙여넣어지며, 값이 하나이면 범위 전체를 채웁니다. 편집 가능한 셀만 바뀌며(읽기 전용 셀에 붙여넣으면 그렇다고 알려 줍니다), choice는 레이블로, 예와 아니요는 사용자의 언어로 붙여넣을 수 있습니다. 바뀐 각 행은 인라인 편집 경로(행 버전이 포함된 `/_zeyn/grid/edit`)로 한 행씩 차례로 저장되므로, 모든 행이 인라인 편집처럼 서버에서 읽히고, 검증되고, 검사됩니다. 실패한 행은 그 셀에 오류를 표시하고 나머지는 저장됩니다. 10행을 넘으면 먼저 확인을 묻고, 500행을 넘으면 거부하며, Ctrl+Z는 붙여넣기 전체를 되돌립니다. 그리드 편집, 행 action, 일괄 action, 메뉴 action은 `App::write_limit`에 포함되므로, 큰 붙여넣기는 앱의 쓰기 제한에 걸릴 수 있습니다.

## 열 가상화

열이 40개 이상인 그리드는 고정된 열과 보이는 열(양쪽으로 최대 여섯 개 추가)만 페이지에 유지합니다. 모든 열은 고정된 너비를 가지며, 제외된 열은 양쪽에서 하나의 스페이서 셀이 대신하므로 너비, 고정, 스크롤은 그대로 유지됩니다. 행은 셀의 전체 목록을 유지하므로 복사, 붙여넣기, 편집은 모든 열을 대상으로 하며, 셀에는 `aria-colindex`가 붙습니다. 창은 그 밖의 열이 보이게 될 때만 이동합니다. 서버는 여전히 행의 모든 셀을 보냅니다. 이만큼 넓은 가상 그리드(`VIRTUAL_WIDE_COLUMNS`, 선택, 디테일, 행 action 열 포함)는 첫 블록 전체를 라이브 행 64개 대신 `<template data-zeyn-rows-later>` 안에 담아 보내므로, 브라우저는 모든 열을 가진 행을 레이아웃하지 않습니다. 갤러리의 넓은 그리드에서 가장 긴 시작 작업은 259 ms에서 112 ms로, DOMContentLoaded는 402 ms에서 228 ms로 줄었습니다(교차 실행한 8회 로드의 중앙값, 릴리스 서버, 바쁜 머신).

갤러리의 넓은 그리드(62개 열, 100,000행, 가상 스크롤, 디버그 서버, Chromium, 1400 x 900 창, 144 Hz 디스플레이)에서 열 가상화를 켠 경우와 끈 경우를 측정했습니다.

| | 페이지의 셀 수 | 가로, 프레임당 60 px (p50 / p95) | 세로, 프레임당 60 px (p50 / p95) | 세로, 프레임당 2,000 px (p50 / p95) |
|---|---|---|---|---|
| 끔 | 2,976 | 6.9 / 7.5 ms | 42.7 / 64.8 ms | 78.5 / 141 ms, 긴 작업 178개 |
| 켬 | 1,056 | 7.0 / 28.8 ms | 24.6 / 41.7 ms | 28.9 / 48.9 ms, 긴 작업 7개 |

아래로 스크롤하는 속도는 약 두 배 빨라지고 페이지에는 셀의 3분의 1만 남습니다. 가로 스크롤은 원래 부드러웠으며, 창이 이동하는 프레임은 최대 29 ms가 듭니다. 나머지 비용은 모든 열을 가진 200행 블록을 파싱하는 것이며, 열 가상화는 이를 바꾸지 않습니다.

## 내보내기

`exportable()`(그리고 선택적으로 `export_permission(...)`, `export_limit(n)`)은 툴바의 `Export::Csv` 버튼이 일치하는 모든 행을 다운로드하게 합니다. 서버는 그리드의 action을 페이지 단위로 순회하고 각 페이지를 즉시 사용자의 레이아웃과 언어로 스트리밍합니다. 수식은 무력화됩니다. 기본값은 최대 100,000행입니다.

`xlsx` Cargo 기능을 사용하면 툴바의 `Export::Xlsx` 버튼이 같은 행을 Excel 통합 문서로 다운로드합니다(`GET /_zeyn/grid/export?format=xlsx`). 셀은 타입을 유지합니다. 숫자, 통화 형식이 있는 숫자로서의 금액, 사용자의 시간대에서의 실제 Excel 날짜로서의 날짜와 날짜 시간, 불리언이 그렇습니다. choice는 레이블로, 마스킹된 필드는 마스킹된 텍스트로 기록되며, 텍스트는 항상 텍스트이므로 수식이 되는 것은 없습니다. 헤더 행은 사용자의 언어로 된 레이블을 담고, 굵게 표시되며 고정되고, 열은 사용자의 순서와 너비를 유지합니다.

통합 문서는 `rust_xlsxwriter`로 일정 메모리 모드에서 기록됩니다. 각 페이지는 도착하는 대로 임시 파일로 가며(메모리에서 기다리는 것은 최대 두 페이지), 압축된 통합 문서는 마지막에 응답으로 스트리밍됩니다. 열 여섯 개의 100,000행을 내보낼 때 행을 기록하는 동안 힙은 약 134 KiB(10,000행 후와 100,000행 후 동일), 압축하는 동안 460 KiB로 유지되었으며 파일은 3.4 MB였습니다.

`pdf` Cargo 기능을 사용하면 `Export::Pdf`가 같은 행을 목록 보고서로 인쇄합니다(최대 2,000행). [PDF 문서](operations.md#pdf-documents)를 참고하십시오.

## 선택

`selectable()`(`bulk_action`이 암시함)은 체크박스 열을 추가합니다. Shift는 범위를 선택하고, Space는 포커스된 행을 선택하며, 헤더 상자는 페이지를 선택합니다. 선택 막대는 일치하는 모든 행을 제안합니다.

## 접근성

테이블에는 `role="grid"`, `aria-rowcount`(가상 그리드), `aria-colcount`, `aria-colindex`, `aria-sort`가 있습니다. 행은 roving tab stop을 사용합니다. 활성 행 안의 버튼은 Tab으로 접근할 수 있습니다. 그룹화된 그리드와 트리 그리드는 `role="treegrid"`이며 행에 `aria-level`과 `aria-expanded`가 있습니다. 메뉴는 `menuitem`, `menuitemcheckbox`, `menuitemradio`와 함께 `role="menu"`를 사용하며 포커스는 있던 자리로 돌아갑니다. 빈 상태와 오류 상태는 안내됩니다(`role="status"`).

## 성능

Performance API로 쇼케이스 아카이브(1,000,000행, 가상 스크롤, 디버그 서버, Chromium, 144 Hz 디스플레이)에서 측정했습니다.

| 스크롤 | 프레임 p50 | 프레임 p95 | 최대 | 긴 작업 |
|---|---|---|---|---|
| 휠 속도 (프레임당 60 px) | 13.9 ms | 20.9 ms | 34.6 ms | 0 |
| 빠름 (프레임당 2,000 px) | 14.1 ms | 27.8 ms | 54.9 ms | 0 |
| 전체 목록 훑기 | 13.9 ms | 27.8 ms | 48.6 ms | 0 |

DOM에는 62개 행(약 660개 요소)이 있으며 JS 힙은 4 MB 근처를 유지합니다. 같은 아카이브에서 100,000행을 내보내면 3.4초 만에 7 MB가 스트리밍되었습니다.

### 화면 빌더를 가볍게 유지하기

요청은 상태를 갖지 않으므로 그리드 요청마다 그리드를 담은 화면이 다시 만들어집니다. 가상 그리드가 가져오는 블록마다(200행을 스크롤할 때마다), 페이지, 정렬, 필터, 내보내기마다, 인라인 편집과 행 action마다 그리드의 action을 실행하기 전에 `#[screen]` 함수가 다시 실행됩니다. 서버는 노드 경로로 그리드를 찾고 접근 권한을 검사하려고 화면이 필요하며, 이것이 사용자별 상태를 보관하지 않는 데 따르는 비용입니다.

컴포넌트를 배치하기만 하는 빌더는 마이크로초면 됩니다. 데이터베이스를 조회하는 빌더는 그 모든 요청마다 조회 비용을 치릅니다. 따라서 다음과 같이 합니다.

- 행은 그리드의 action(`DataGrid::of(search)`)에서 불러옵니다. 어차피 요청마다 한 번 실행되며 페이지, 정렬, 필터가 담긴 `DataQuery`를 받습니다.
- 선택지와 조회는 필드 타입(`#[choice]`), 폼 조회(`Form::lookup`, 사용자가 입력할 때 자신의 action에 묻습니다), 또는 상한이 있는 캐시에 보관하는 서비스에 두고, 빌더가 실행하는 쿼리에는 두지 않습니다.
- 화면이 레코드를 불러와야 한다면(`Form::of(record)`) 크거나 가상인 그리드는 그 화면에 두지 않습니다. 별도 화면, 디테일 화면(`DataGrid::detail`) 또는 창에 두면 그 블록 요청은 아무것도 불러오지 않는 화면을 다시 만듭니다.
- 화면 params는 빌더에 필요한 입력이므로 키로 한정하고, 그것으로 거르는 일은 그리드의 action에 맡깁니다(`DataGrid::scope`).
