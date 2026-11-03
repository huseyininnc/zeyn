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

## 提示

`ui.toast(text)` 显示 info 提示。`ui.notify(Toast::success(text))` 可选择语气：`Toast::info`、`success`、`warning` 或 `danger`。`.action(label, effect)` 会添加一个运行另一个 effect 的按钮，例如打开刚保存的记录：

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

提示会停留四秒（danger 提示为八秒），当指针位于提示上方或提示持有焦点时，计时会暂停，以便用户可以操作其中的按钮。Info、success 和 warning 提示由状态区域（status region）以礼貌方式朗读；danger 提示是警报（alert），会立即播报。每个提示都有关闭按钮，语气同样通过其颜色条和文字表达，并且同时最多显示三个提示（`MAX_TOASTS`）：新的提示会移除最早的那个。

@live toasts
