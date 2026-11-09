# Effect 与窗口

Effect 是浏览器在点击或 action 成功之后所做的事情：打开窗口、跳转到某个界面、显示提示（toast）、请求确认，或关闭窗口并刷新其下方的界面。它们用 Rust 声明并由运行时执行，因此无需编写 JavaScript。

## 各种 effect

| Effect | 构建方式 | 结果 |
|---|---|---|
| 打开窗口 | `Window::open(screen, params)` | 在模态窗口中打开该界面 |
| 导航 | `Effect::navigate(screen, params)` | 像链接一样就地跳转到该界面 |
| 提示 | `Effect::Toast { message }` | 显示一条简短消息 |
| 带语气的提示 | `Toast::success(message).action(label, effect).into()` | 带有语气（info、success、warning、danger）的提示，可附带一个运行另一个 effect 的按钮 |
| 确认 | `Effect::confirm(message, then)` | 用户确认后运行 `then` |
| 关闭并刷新 | `Effect::CloseAndRefresh` | 关闭窗口，并重新加载打开它的界面 |
| 刷新主题 | `Effect::refresh("orders")` 或 `ui.refresh(["orders", "stock"])` | 就地刷新页面上监听某个主题的节点（见 [主题](#topics)） |

参数是有类型的：为参数错误的界面构造 effect 无法通过编译。

`Effect::navigate` 到路由含有路径段的界面时（参见[路径中的参数](screens.md)），会落在路径形式上，例如 `/orders/42`；不在路径中的参数则放在 `params` 查询中。

## action 之后

表单和 action 按钮接受 `on_success` 处理函数。它会收到 action 的结果，以及一个收集 effect 的 `UiCtx`：

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

`UiCtx` 提供 `toast`、`notify`、`close_and_refresh`、`navigate`，以及用于其他情形（例如确认）的 `effect`：

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## 窗口

窗口是没有路由的界面，用 `Screen::window(title)` 构建。窗口可以堆叠：一个窗口可以打开另一个窗口。按 Escape 或点击关闭按钮会关闭最上层的窗口，焦点回到打开它的元素。`.width(px)` 设置其宽度。

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

## effect 的来源

- 表格链接与行打开：`.link(|p| Window::open(...))`、`.on_open(|p| ...)`。
- 工具栏按钮：`.opens(screen, params)`。
- 上下文菜单：`.item(label, effect)`。
- 命令：`Command::open`、`Command::navigate`、`Command::effect`。
- 表单和 action 按钮的 `on_success` 处理函数。

Effect 绝不会重新加载页面。导航只加载新的界面；刷新只重新加载窗口下方的界面。

## 主题

界面的一部分变化时，其他部分往往显示同样的数据：保存的订单会改变合计，筛选会改变网格和图表。主题把它们连接起来，数据不经过浏览器。用 `.listens(topic)` 标记显示这些数据的节点；主题被点名时，它们就地刷新。

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

点名主题有三种方式：

- action 改变数据之后，在 `on_success` 处理器中调用 `ui.refresh("sales")`（或 `Effect::refresh`）。
- 带有 `.drives("sales")` 的筛选控件：筛选标签、搜索框，或没有提交按钮的表单（选择框、日期范围、文本）。见 [筛选表单](forms.md#filter-forms)。
- 别人改变数据时，来自 realtime 通道（feature `realtime`）的 `hub.notify(&ctx, "sales")`。见 `docs/realtime.md`。

`.listens` 适用于任何组件：在组件自己的 builder 方法之后最后调用。包含网格的监听者通过网格端点重新加载网格，保留页、排序和筛选（由筛选引起的变化从第一页开始）。其他所有监听者在服务器上重新渲染，全部只用一次 `POST /_zeyn/nodes`，所以一个 KPI 加一个图表只需一次请求。传递的只有主题名；服务器像处理任何请求一样，以用户的权限和 params 打开界面，因此刷新绝不会显示用户无权看到的内容。

同一动画帧内点名的主题只处理一次，同一界面上较新的刷新会取消仍在进行的刷新，并且只刷新页面上存在的节点。如果新内容中有相同的元素（按 id、test id 或列表的行键），焦点和展开的 `<details>` 会保持原位。

主题名由 1 到 64 个字母、数字、`-`、`_` 或 `.` 组成。`App::check_page` 会报告页面上被驱动却无人监听的主题；`zeyn check` 会对被刷新或被驱动却从未被监听，或被监听却从未被刷新、驱动或通知的主题发出警告。

@live topics

## 提示

`ui.toast(text)` 显示 info 提示。`ui.notify(Toast::success(text))` 可选择语气：`Toast::info`、`success`、`warning` 或 `danger`。`.action(label, effect)` 会添加一个运行另一个 effect 的按钮，例如打开刚保存的记录：

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

提示会停留四秒（danger 提示为八秒），当指针位于提示上方或提示持有焦点时，计时会暂停，以便用户可以操作其中的按钮。Info、success 和 warning 提示由状态区域（status region）以礼貌方式朗读；danger 提示是警报（alert），会立即播报。每个提示都有关闭按钮，语气同样通过其颜色条和文字表达，并且同时最多显示三个提示（`MAX_TOASTS`）：新的提示会移除最早的那个。

@live toasts
