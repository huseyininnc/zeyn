# Effect와 창

effect는 클릭이나 성공한 action 후에 브라우저가 하는 일입니다. 창 열기, Screen 이동, 토스트 표시, 확인 요청, 창을 닫고 그 아래 Screen을 새로 고치기가 있습니다. Rust로 선언하고 런타임이 실행하므로 JavaScript를 작성하지 않습니다.

## effect 종류

| Effect | 만드는 방법 | 결과 |
|---|---|---|
| 창 열기 | `Window::open(screen, params)` | Screen을 모달 창에서 엽니다 |
| 이동 | `Effect::navigate(screen, params)` | 링크처럼 그 자리에서 Screen으로 이동합니다 |
| 토스트 | `Effect::Toast { message }` | 짧은 메시지를 표시합니다 |
| 톤이 있는 토스트 | `Toast::success(message).action(label, effect).into()` | 톤(info, success, warning, danger)이 있는 토스트이며, 다른 effect를 실행하는 버튼을 선택적으로 붙일 수 있습니다 |
| 확인 | `Effect::confirm(message, then)` | 사용자가 확인하면 `then`을 실행합니다 |
| 닫고 새로 고침 | `Effect::CloseAndRefresh` | 창을 닫고 그 창을 연 Screen을 다시 불러옵니다 |
| 토픽 새로 고침 | `Effect::refresh("orders")` 또는 `ui.refresh(["orders", "stock"])` | 토픽을 listen하는 페이지의 노드를 제자리에서 새로 고칩니다([토픽](#topics) 참고) |

매개변수에는 타입이 있습니다. 잘못된 매개변수로 Screen에 대한 effect를 만들면 컴파일되지 않습니다.

경로 세그먼트가 있는 경로의 Screen으로 `Effect::navigate`하면([경로의 매개변수](screens.md) 참고) `/orders/42` 같은 경로 형태로 이동하며, 경로에 없는 매개변수는 `params` 쿼리에 들어갑니다.

## action 이후

폼과 action 버튼은 `on_success` 핸들러를 받습니다. 핸들러는 action의 결과와 effect를 모으는 `UiCtx`를 받습니다.

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx`에는 `toast`, `notify`, `close_and_refresh`, `navigate`, 그리고 확인 같은 그 밖의 모든 것을 위한 `effect`가 있습니다.

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## 창

창은 경로가 없는 Screen이며 `Screen::window(title)`로 만듭니다. 창은 쌓입니다. 창이 다른 창을 열 수 있습니다. Escape 또는 닫기 버튼은 맨 위 창을 닫고 포커스는 그 창을 연 요소로 돌아갑니다. `.width(px)`로 너비를 설정합니다.

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## effect의 출처

- 그리드 링크와 행 열기: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`.
- 툴바 버튼: `.opens(screen, params)`.
- 컨텍스트 메뉴: `.item(label, effect)`.
- 명령: `Command::open`, `Command::navigate`, `Command::effect`.
- 폼과 action 버튼의 `on_success` 핸들러.

effect는 페이지를 새로 고치지 않습니다. 이동은 새 Screen만 불러오고, 새로 고침은 창 아래의 Screen만 다시 불러옵니다.

## 토픽

Screen의 한 부분이 바뀌면 다른 부분도 같은 데이터를 보여 주는 경우가 많습니다. 저장한 주문은 합계를 바꾸고, 필터는 그리드와 차트를 바꿉니다. 토픽은 데이터가 브라우저를 거치지 않고도 이들을 연결합니다. 데이터를 보여 주는 노드에 `.listens(topic)`을 붙이면, 토픽이 지정될 때 제자리에서 새로 고쳐집니다.

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

토픽은 세 가지 방법으로 지정됩니다.

- action이 데이터를 바꾼 뒤 `on_success` 핸들러에서 `ui.refresh("sales")`(또는 `Effect::refresh`).
- `.drives("sales")`를 가진 필터 컨트롤: 필터 칩, 검색 상자, 제출 버튼이 없는 폼(선택 상자, 날짜 범위, 텍스트). [필터 폼](forms.md#filter-forms)을 참고하세요.
- 다른 사람이 데이터를 바꿨을 때 realtime 채널(feature `realtime`)의 `hub.notify(&ctx, "sales")`. `docs/realtime.md`를 참고하세요.

`.listens`는 모든 컴포넌트에서 동작합니다. 컴포넌트 자체의 builder 메서드 다음에 마지막으로 호출하세요. 그리드를 가진 리스너는 그리드 엔드포인트로 그리드를 다시 불러오며 페이지, 정렬, 필터를 유지합니다(필터로 인한 변경은 첫 페이지부터 시작합니다). 다른 모든 리스너는 한 번의 `POST /_zeyn/nodes`로 함께 서버에서 다시 렌더링되므로, KPI와 차트에 요청 하나면 됩니다. 오가는 것은 토픽 이름뿐이고, 서버는 다른 요청과 마찬가지로 사용자의 접근 권한과 params로 Screen을 열기 때문에 새로 고침이 사용자가 볼 수 없는 내용을 보여 주는 일은 없습니다.

한 애니메이션 프레임 안에서 지정된 토픽은 한 번만 처리되고, 같은 Screen에서 더 새로운 새로 고침은 아직 진행 중인 새로 고침을 취소하며, 페이지에 있는 노드만 새로 고쳐집니다. 새 내용에 같은 요소(id, test id, 목록의 행 키로 판단)가 있으면 포커스와 열린 `<details>`는 그 자리에 남습니다.

토픽 이름은 1에서 64자의 영문자, 숫자, `-`, `_`, `.`입니다. `App::check_page`는 페이지에서 drive되지만 그 페이지에서 아무도 listen하지 않는 토픽을 보고하고, `zeyn check`는 새로 고쳐지거나 drive되지만 아무도 listen하지 않는 토픽, 또는 listen되지만 새로 고침, drive, 알림이 전혀 없는 토픽에 대해 경고합니다.

@live topics

## 토스트

`ui.toast(text)`는 info 토스트를 표시합니다. `ui.notify(Toast::success(text))`는 톤을 고릅니다. `Toast::info`, `success`, `warning`, `danger`가 있습니다. `.action(label, effect)`는 저장된 레코드를 여는 것처럼 다른 effect를 실행하는 버튼을 추가합니다.

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

토스트는 4초 동안 유지되며(danger는 8초), 포인터가 토스트 위에 있거나 포커스를 가지고 있는 동안에는 시간이 멈추므로 버튼에 접근할 수 있습니다. info, success, warning 토스트는 상태 리전이 정중하게 읽고, danger 토스트는 알림이므로 즉시 안내됩니다. 모든 토스트에는 닫기 버튼이 있고, 톤은 색상 막대와 텍스트로도 전달되며, 한 번에 최대 세 개의 토스트만 표시됩니다(`MAX_TOASTS`). 새 토스트는 가장 오래된 것을 제거합니다.

@live toasts
