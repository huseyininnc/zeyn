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

## 열

| 호출 | 결과 |
|---|---|
| `.width(px)`, `.flex(n)` | 고정 너비 또는 남는 너비의 비율 |
| `.filter()` | 타입에 맞는 필터 상자: 텍스트, 숫자 범위, 날짜 범위, 선택 목록 |
| `.sum()`, `.avg()`, `.min()`, `.max()` | 바닥글의 합계. 텍스트 열에 `sum`을 쓰면 컴파일되지 않습니다 |
| `.badge(\|v\| tone)` | 값을 색상 배지로 표시합니다 |
| `.progress(max)` | 0부터 `max`까지의 막대 |
| `.check_icon()` | `true`에 대한 체크 표시 |
| `.link(\|row\| effect)` | 셀을 effect를 실행하는 링크로 만듭니다 |
| `.pin_start()`, `.hidden()` | 시작 쪽에 고정하거나, 사용자가 표시할 때까지 숨깁니다 |
| `.editable()` | 그 자리에서 편집할 수 있으며 `.edit_with(action)`으로 저장됩니다 |

## 행

- `.on_open(|row| effect)`: 행에서 더블 클릭 또는 Enter.
- `.row_action(label, action, |row| input)`: 각 행의 버튼.
- `.bulk_action(label, action, M::id)`: 선택한 행에서 실행됩니다.
- `.row_action`이나 `.bulk_action` 바로 뒤의 `.confirm(message)`와 `.tone(Tone::Danger)`: 그 작업이 실행되기 전에 `ActionButton::confirm`과 같은 대화 상자로 묻고(포커스는 먼저 취소에 있음), 버튼을 빨간색으로 그립니다.
- `.row_menu(|row| ContextMenu)`: 행의 오른쪽 클릭 메뉴.
- `.row_style(|row| Option<RowStyle>)`: danger, warning, success 행.
- `.exportable()`: 필터링된 행의 CSV 내보내기이며 서버가 스트리밍합니다.

[DataGrid 레퍼런스](grid.md)는 저장된 뷰, 마스터 디테일, 그룹화, 트리 데이터, 인라인 편집, 셀 탐색, 복사와 붙여넣기, 열 가상화, 접근성을 다룹니다.
