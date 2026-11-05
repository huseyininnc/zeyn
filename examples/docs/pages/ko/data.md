# 트리와 차트

## 트리

`Tree::of(action)`은 필요할 때 불러오는 계층 구조를 보여 줍니다. action은 `TreeQuery`를 받아 `q.parent`의 자식을 반환합니다(최상위 수준은 `None`). 가지는 처음 열릴 때 자식을 불러오므로, 큰 계층 구조 위의 트리는 열린 가지마다 작은 요청 하나만 듭니다.

```rust
#[action(permission = "stock.product.read")]
pub async fn sample_tree(q: TreeQuery, store: SampleStore) -> Result<Vec<TreeItem>> {
    Ok(match q.parent.as_deref() {
        None => vec![TreeItem::branch("piece", "Piece"), TreeItem::branch("kg", "Kilogram")],
        Some(unit) => store.by_unit(unit)?.into_iter().map(|s| TreeItem::leaf(s.id.0.to_string(), s.name).hint(s.code)).collect(),
    })
}

Tree::of(sample_tree).label("Products by unit")
```

@live tree

트리는 ARIA 트리 패턴을 따릅니다. 화살표 키로 항목 사이를 이동하고 가지를 열거나 닫습니다. 서버는 가지당 자식 수에 상한을 둡니다(`MAX_TREE_CHILDREN`).

## 차트

`Chart::bar`, `stacked_bar`, `line`, `area`, `combo`, `scatter`, `donut`, `gauge`, `funnel`은 서버에서 SVG를 그립니다. 브라우저에는 차트 라이브러리가 없으며, 차트는 페이지 HTML의 일부입니다.

```rust
Row::new()
    .col(Chart::bar("Sales").labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]))
    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0]))

Chart::stacked_bar("Channels").labels(months).series("Shop", shop).series("Web", web)
Chart::combo("Sales and margin").labels(months).series("Sales", sales).line_series("Margin %", margin).right_axis()
Chart::area("Visits").labels(months).series("Visits", visits)
Chart::scatter("Price and units").points("Tea", [(1.5, 40.0), (2.5, 22.0)])
Chart::gauge("Capacity").value(72.0).range(0.0, 100.0).target(85.0)
Chart::funnel("Checkout").labels(["Visited", "Cart", "Paid"]).series("People", [1000.0, 420.0, 150.0])
```

@live chart

각 차트에는 제목, 스크린 리더를 위한 텍스트 요약, 범례가 있으며, 수치는 *Show data* 아래의 표로 제공됩니다. 계열 색상은 `--z-chart-1`부터 `--z-chart-6`까지의 토큰입니다. 테스트(`chart_colors_reach_three_to_one_in_both_themes`)가 라이트 테마와 두 다크 테마에서 `--z-surface` 및 `--z-surface-2`에 대한 3:1 대비를 검사합니다([테마](theming.md) 참고).

모든 막대, 점, 조각, 단계, 게이지 값은 레이블과 값을 이름으로 가진 마크입니다. SVG는 제목과 요약으로 이름이 붙은 그룹이며, 차트는 탭 정지 지점 하나입니다.

| 키 | 동작 |
|---|---|
| Tab | 차트 안으로, 마지막으로 포커스된 마크로 이동합니다(처음에는 첫 번째 마크) |
| 왼쪽, 오른쪽 | 같은 계열의 이전 또는 다음 마크 (`dir="rtl"`에서는 서로 바뀝니다) |
| 위쪽, 아래쪽 | 이전 또는 다음 계열의 같은 지점 |
| Home, End | 계열의 첫 번째 또는 마지막 마크 |
| Escape | 툴팁을 숨깁니다 |

마우스 오버, 포커스, 탭은 같은 텍스트를 툴팁으로 보여 주며, 툴팁은 중복해서 읽히지 않도록 스크린 리더에서 숨겨집니다. 스크립트가 없어도 마크에는 이름이 있고 표가 남습니다. 작은 마크(8 px 점)는 24 px 대상 크기에 못 미칩니다. *Show data* 아래의 표가 동등한 대체 경로입니다(WCAG 2.5.8).

## 핵심 수치, 스파크라인, 불릿

`Kpi`는 값, 이전 기간 대비 변화(화살표와 방향을 나타내는 단어, 변화가 좋으면 녹색, 나쁘면 빨간색이며 비용과 대기 시간에는 `.lower_is_better()`), 달성 비율이 있는 목표, 최근 값의 스파크라인을 보여 줍니다.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money`는 금액을 사용자 로캘에 맞춰 통화와 함께 보여 주며(영어에서 `₺184,250.50`, 터키어에서 `184.250,50 ₺`), 목표도 같은 통화로 읽습니다.

한 `Row`나 `AutoGrid` 안의 Kpi는 추세가 있든 없든 가장 높은 것과 같은 높이이며, 스파크라인은 각 카드의 아래쪽에 놓입니다. Kpi로 된 `Row`는 각 Kpi에 너비의 4분의 1을 줍니다. 데스크톱에서는 네 개가 한 줄을 채우고, 약 830 px보다 좁으면 한 줄에 두 개, 480 px보다 좁으면 한 개로 줄바꿈합니다(행에 대한 컨테이너 쿼리이므로 좁은 열 안의 Kpi 행은 더 일찍 줄바꿈합니다). 차트는 컨테이너 너비를 채우고 비율을 유지합니다. 도넛과 게이지는 높이가 최대 18rem이며 가운데에 놓입니다.

```rust
Row::new()
    .col(Kpi::money("Revenue", revenue).delta(12.4).trend(weekly))
    .col(Kpi::new("Orders", 1_284.0).delta(3.1))
    .col(Kpi::new("New customers", 312.0).delta(-2.0))
    .col(Kpi::money("Average order", average).delta(0.0))
```

`DataGrid`에서 `Vec<f64>` 열은 스파크라인으로, 숫자 또는 금액 열은 목표에 대한 불릿으로 그려집니다.

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

둘 다 셀당 작은 SVG 하나이며 "Trend of 12 values from 40 to 61, lowest 12, highest 99" 같은 이름을 가집니다. 예산: 두 셀을 합쳐 행당 HTML 약 0.65 KB입니다. 둘 다 있는 10,000행은 디버그 빌드에서 약 0.4초에 렌더링되며(`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`), 그리드는 보이는 행만 렌더링합니다. `charts.js`는 원본 3.7 KB(gzip 1.4 KB), `charts.css`는 4.1 KB(gzip 1.3 KB)를 추가합니다.
