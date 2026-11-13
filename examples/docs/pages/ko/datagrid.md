# DataGrid

`DataGrid`는 `DataQuery`를 받아 `Page<E>`를 반환하는 action의 행을 보여 줍니다. 페이징, 정렬, 필터, 합계가 서버에서 실행되므로 백만 행 위의 그리드도 브라우저에는 백 행 위의 그리드와 같은 비용이 듭니다.

## 그리드

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

헤더를 클릭하면 정렬되고, 필터 상자에 입력할 수 있으며, 헤더를 끌면 열이 이동하고 가장자리를 끌면 크기가 바뀝니다. 합계는 페이지만이 아니라 필터링된 모든 행을 대상으로 합니다.

## action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply`는 메모리의 `Vec`을 필터링, 정렬, 합산, 페이징하며, 작은 목록과 테스트에 적합합니다. 데이터베이스를 쓰는 스토어는 같은 `DataQuery`(페이지, 정렬, 필터, 합계)를 읽고 SQL로 응답합니다. [데이터와 데이터베이스](database.md)를 참고하십시오.

## 페이징

| 페이징 | 용도 |
|---|---|
| `Paging::pages(n)` | 번호가 있는 페이지이며, 지정하지 않으면 50행 |
| `Paging::infinite(n)` | 스크롤하는 동안 행을 더 불러옵니다 |
| `Paging::virtual_scroll()` | 매우 큰 목록. 보이는 행만 페이지에 있습니다 |
| `Paging::none()` | 한 페이지에 들어가는 짧은 목록 |

클라이언트가 무엇을 요청하든 서버가 페이지 크기에 상한을 둡니다.

`.page_sizes(&[10, 25, 50, 100])`는 번호 페이지의 페이저에 "페이지당 행 수" 목록을 더합니다. 그리드 자체의 크기는 항상 제공되고, 선택은 사용자의 뷰와 함께 저장되며, 서버는 그리드가 제공하는 크기만 받습니다.

## 열

| 호출 | 결과 |
|---|---|
| `.width(px)`, `.flex(n)` | 고정 너비 또는 남는 너비의 비율 |
| `.filter()` | 타입에 맞는 필터 상자: 텍스트, 숫자 범위, 날짜 범위, 선택 목록 |
| `.sum()`, `.avg()`, `.min()`, `.max()` | 바닥글의 합계. 텍스트 열에 `sum`을 쓰면 컴파일되지 않습니다 |
| `.badge(\|v\| tone)` | 값을 톤의 은은한 색조 위에 레이블로 표시합니다 |
| `.status(\|v\| tone)` | 값을 상태로 표시합니다. 일반 텍스트 앞에 톤 색의 점을 두며 배지보다 차분합니다 |
| `.progress(max)` | 0부터 `max`까지의 막대 |
| `.check_icon()` | `true`에 대한 체크 표시 |
| `.link(\|row\| effect)` | 셀을 effect를 실행하는 링크로 만듭니다 |
| `.pin_start()`, `.hidden()` | 시작 쪽에 고정하거나, 사용자가 표시할 때까지 숨깁니다 |
| `.editable()` | 그 자리에서 편집할 수 있으며 `.edit_with(action)`으로 저장됩니다 |
| `.plain()` | 정수를 자릿수 구분 기호 없이 표시합니다. 코드와 연도용 |
| `.wrap()` | 긴 값을 여러 줄로 줄바꿈합니다. 설정하지 않으면 셀은 한 줄로 유지되고 말줄임표로 끝나므로 모든 행의 높이가 같습니다 |

행은 40 px(촘촘한 밀도에서 32 px)이고 선으로만 구분됩니다. 아주 넓은 그리드에서는 `DataGrid::zebra()`가 한 행 걸러 음영을 넣습니다. 셀 안의 링크는 굵기 500의 텍스트로 보이고 호버하면 밑줄이 생깁니다.

숫자와 금액은 오른쪽 정렬되고 고정폭 숫자로 표시됩니다. 정수는 그리드 셀과 `DescriptionList` 같은 표시 값에서 사용자 로캘에 따라 묶어 표시됩니다(영어 `1,234,567`, 터키어 `1.234.567`). `#[key]` id는 묶지 않으며, `.plain()`은 한 열의 묶음 표시를 끕니다.

## 행

- `.on_open(|row| effect)`: 행에서 더블 클릭 또는 Enter.
- `.row_action(label, action, |row| input)`: 각 행의 버튼.
- `.bulk_action(label, action, M::id)`: 선택한 행에서 실행됩니다.
- `.row_action`이나 `.bulk_action` 바로 뒤의 `.confirm(message)`와 `.tone(Tone::Danger)`: 그 작업이 실행되기 전에 `ActionButton::confirm`과 같은 대화 상자로 묻고(포커스는 먼저 취소에 있음), 버튼을 빨간색으로 그립니다.
- `.row_menu(|row| ContextMenu)`: 행의 오른쪽 클릭 메뉴.
- `.row_style(|row| Option<RowStyle>)`: danger, warning, success 행.
- `.add_with(create)`, `.delete_with(action, M::id)`, `.batch_edit()`: "행 추가" 버튼, 삭제 버튼과
  키, 함께 저장되는 편집. [DataGrid 참조](grid.md)를 보세요.
- 열의 `.groupable()`, `.summary(label, |t| ...)`: 사용자가 열 메뉴나 그룹 패널에서 그 열로 그룹화하고, 합계로 계산한 값이 열 아래에 표시됩니다.
- `.filter_builder()`: 필터 조건을 만드는 대화 상자를 여는 "필터" 버튼. 필터 행, 저장된 뷰, AI 에이전트가 쓰는 것과 같은 목록입니다.
- `.exportable()`: 필터링된 행의 CSV 내보내기이며 서버가 스트리밍합니다.

## 실행 시점에 정해지는 열

`DataGrid::dynamic(action)`은 SQL 미리보기처럼 열이 행과 함께 오는 결과를 위한 것입니다. action은 열(이름, 레이블, `Integer`, `Decimal`, `Date` 같은 `DynamicKind`)과 JSON으로 된 한 페이지의 행을 담은 `DynamicPage`를 반환합니다:

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

페이지 나누기, 가상 스크롤, 키보드 사용, 사용자 언어에 맞춘 숫자와 날짜 서식은 타입이 있는 grid와 똑같이 동작합니다. 이름, 레이블, 셀은 이스케이프되며 서버가 열 수, 행 수, 텍스트 길이를 제한합니다. 이 grid는 정렬하거나 필터링하지 않으며 행은 읽기 전용입니다. [DataGrid 레퍼런스](grid.md)를 참고하세요.

## 검색과 밴드

`.search()`는 그리드 위에 검색 상자를 둡니다. 액션은 텍스트를 `DataQuery::q`로 받고,
`DataQuery::apply`는 마스킹된 필드를 뺀 텍스트 필드와 비교합니다.
`.band(label, |b| b.column(..).column(..))`는 열을 하나의 헤더 셀 아래로 묶습니다.
[DataGrid 참조](grid.md)를 보세요.

## 제자리 새로 고침

마지막에 호출한 `.listens(topic)`은 토픽이 지정될 때 그리드를 제자리에서 다시 불러오게 합니다([토픽](effects.md#topics) 참고). `ui.refresh(topic)` 뒤에는 정렬과 필터를 유지한 채 현재 페이지를 다시 불러오고, `.drives(topic)`을 가진 필터 컨트롤이 바뀐 뒤에는 새 Screen params로 첫 페이지부터 시작합니다. 그리드는 모든 권한 검사를 거치는 `POST /_zeyn/grid`로 다시 불러와지며, 필터가 바꾼 Screen param은 Screen 함수를 거쳐, 예를 들어 scope로 그리드에 전달됩니다.

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

[DataGrid 레퍼런스](grid.md)는 저장된 뷰, 마스터 디테일, 그룹화, 트리 데이터, 인라인 편집, 셀 탐색, 복사와 붙여넣기, 열 가상화, 접근성을 다룹니다.
