# 레이아웃

레이아웃 컴포넌트는 다른 컴포넌트를 배치합니다. 데이터를 담지 않고 요청도 추가하지 않으며, 이들로 만든 Screen은 하나의 HTML 응답입니다.

```rust
Stack::new()
    .add(Row::new().col(Card::new().title("Orders")).col(Card::new().title("Stock")).col(Card::new().title("People")))
    .add(
        Panel::new("Products").collapsible().add(
            Tabs::new("Views")
                .tab("Card", Card::new().title("A card inside a tab"))
                .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
        ),
    )
```

@live layout

## Stack

노드를 위아래로 쌓습니다. `.gap(units)`는 그 사이의 간격을 간격 단위(보통 밀도에서 4 px)로 0에서 12까지 설정합니다.

## Row

열을 나란히 배치합니다. 각 `.col(node)`는 너비의 일부를 차지하며, 열 뒤의 `.weight(n)`은 그 열을 더 넓게 만듭니다. 좁은 화면에서는 열이 쌓입니다. `.compact()`는 배지 같은 작은 항목을 촘촘하게 배치합니다.

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card와 Panel

`Card`는 선택적 제목이 있는 단순한 면입니다. `Panel`은 선택적 툴바가 있는 제목 있는 섹션이며, `.collapsible()`은 사용자가 접을 수 있게 하고 `.collapsed()`는 접힌 상태로 시작하게 합니다.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)`과 `.tab(title, node)`입니다. ARIA 탭 패턴에 설명된 대로 화살표 키로 탭 사이를 이동합니다. 닫힌 탭의 내용은 탭이 처음 열릴 때까지 렌더링되지 않으므로 탭이 많은 Screen도 가볍게 유지됩니다.

## Split

`Split::new(start, end)`는 두 노드를 그 사이의 핸들과 함께 보여 줍니다. `.ratio(percent)`는 시작 쪽의 크기를 설정합니다. 핸들은 포인터나 화살표 키로 움직입니다.

## Dock

`Dock`은 중앙 영역 주위에 패널을 배치합니다. 사용자가 이동하고, 크기를 바꾸고, 닫고, 다시 가져올 수 있는 왼쪽, 오른쪽, 아래쪽, 중앙 탭이 있습니다. 각 사용자의 레이아웃은 Screen별로 저장됩니다. [셸과 명령](shell.md)을 참고하십시오.
