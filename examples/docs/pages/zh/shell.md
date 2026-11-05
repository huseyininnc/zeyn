# Shell 和命令

shell 是每个 screen 外围的框架：菜单、语言、主题和时区选择器、toast 提示以及命令面板。像桌面工具那样工作的应用还会增加菜单栏、工具箱、属性检查器、状态栏和停靠面板。

## 菜单

菜单来自各 feature 的 `.menu(...)` 调用。用户无权打开的项会从 HTML 中省略，guard 拒绝该用户的 feature 则什么也不显示。当前 screen 的链接带有 `aria-current="page"`。

## 站点 shell

工作 screen 在带侧边菜单的应用 shell 中打开。公共页面（例如商店或产品目录）可以改为在站点 shell 中打开：应用名称作为品牌链接、横向菜单、窄屏幕上的菜单按钮、可选的操作和页脚。由 feature 选择使用哪一个；其他 feature 的 screen 保持应用 shell，因此两者可以共存于同一个应用。

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

- **菜单。** 顶部菜单在站点 shell 中列出每个 feature 的菜单。只有一项的组是普通链接；较大的组是打开其链接的按钮（W3C APG 披露式导航模式）。Enter、Space 或点击切换一个组，Down 打开它并移到其第一个链接，Up、Down、Home 和 End 在其链接之间移动，Escape 关闭它并回到按钮，用 Tab 离开该组或点击外部会将其关闭。Left 和 Right 在顶层项之间移动。
- **窄屏幕。** 在 800 px 以下，菜单折叠到带 `aria-expanded` 的菜单按钮后面；Escape 将其关闭。没有脚本时菜单保持展开，因此没有任何内容变得无法访问。
- **头部插槽。** 顶部栏有四个可放置任意组件的插槽，类似使用逻辑名称的应用栏：`start(node)` 在品牌之前，`center(node)` 在菜单与末端之间，`end(node)` 在栏的末端，`bottom(node)` 是其下方的整宽一行，例如放 chip。每个插槽按顺序容纳多个节点，在从右到左的语言中 start 和 end 会交换位置。插槽中的组件使用用户的语言、权限和掩码渲染；通过自己的 action 加载数据的组件（例如 `SearchBox`）也能在其中工作，并按该 feature 的 guard 和 action 的权限检查。grid 和表单属于 screen。在手机上，start 和 end 插槽留在栏中，center 插槽则按应用指定的位置放置：`center_on_phone(CenterOnPhone::Row)`（默认）把它放在栏下方的一行，`CenterOnPhone::Icon` 把它放在放大镜按钮之后，由按钮打开（W3C APG 披露模式）并把焦点移入其中；Escape 将其关闭并回到按钮。`center_button(icon, label)` 可更改该按钮。手机布局由 CSS 在脚本运行之前设定，因此页面加载时头部不会移动。
- **操作。** 当应用有登录页面时，`sign_in()` 对访客显示登录链接，对已登录用户显示退出链接；`language()` 和 `theme()` 显示与应用 shell 相同的选择器。它们排在 end 插槽之后，并在手机上折叠进菜单。
- **底部栏。** `bottom_nav([...])` 添加最多五个固定在手机屏幕底部（宽度 800 px 及以下）的链接，用于人们最常去的地方；超过五个则无法编译。每个条目是 `BottomNavItem`，包含标签、带路由的屏幕和可选图标。当前显示页面的链接带有 `aria-current="page"`（由 `nav.js` 保持更新），用户无权打开的屏幕的链接会被省略，底部栏通过 `env(safe-area-inset-bottom)` 避开主屏指示条，页面底部会留出空间，因此底部栏不会遮挡任何内容。顶部菜单仍用于其余内容。底部栏不需要脚本。
- **页脚。** 若干列链接，指向 screen（`link`，对无权打开的用户省略）或地址（`url`：路径、`https:`、`http:` 或 `mailto:`；`App::check` 会拒绝其他协议以及指向未注册 screen 的链接），外加一行文字。

在站点 shell screen 与应用 shell screen 之间切换，通过 `nav.js` 进行，无需整页加载：局部响应会说明它属于哪个 shell（`x-zeyn-shell`），当它与当前框架不同时，运行时会像切换语言那样替换整个框架，并加载另一个 shell 的样式表和脚本。指向 `/_zeyn/route/{screen}` 的链接（面包屑的链接）也以同样方式被跟随。

## 屏幕中的主题选择器

`ThemePicker::new()` 把主题选择器放进屏幕里，例如设置页面的“外观”部分。它提供 `App::themes` 中的主题（应用跟随设备时还有“系统”），并显示当前使用的主题。

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

它与外壳中的选择器是同一个控件：选择会立即生效，不重新加载页面，保存在 `zeyn_theme` cookie 中（已登录用户还会保存在视图存储中），页面上的另一个选择器也会同步显示。应用不提供选择时（`theme_picker(false)` 或只有一个主题），它不渲染任何内容。

## 命令

命令在 Rust 中一次性声明，包含其标签、快捷键以及它要做的事情。在 feature 上注册后，命令面板会列出它，其快捷键在每个页面都有效。

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

| 构造函数 | 运行 |
|---|---|
| `Command::open(id, label, screen, params)` | 以窗口形式打开一个 screen |
| `Command::navigate(id, label, screen, params)` | 跳转到一个 screen |
| `Command::effect(id, label, effect)` | 浏览器中的 effect，例如 toast |
| `Command::action(id, label, action, input)` | 服务器上的 action；input 绝不会传到浏览器 |

`App::check` 会报告无法解析的快捷键、被两个命令共用的快捷键，以及命令面板为自己保留的快捷键。

## 命令面板

默认情况下 **Alt+Shift+P** 打开命令面板（`App::palette_shortcut` 可以更改，也可以特意改为 `Ctrl+Shift+P` 之类的浏览器按键）：它是一个 ARIA 组合框，搜索应用的命令、当前 screen 的工具栏和上下文菜单，以及菜单中的 screen。最近使用的命令排在前面。

feature 可以为它添加服务器搜索。用户输入时，命令面板调用该 action，并将其结果列在命令之下；选择其中一项会原位打开该页面。本站就是以这种方式搜索自己的页面：

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

服务器以 feature 的 guard、action 的权限和拦截器运行用户可以运行的每次搜索，将查询截断为 100 个字符，丢弃指向应用之外的结果，最多返回 20 条。

## 菜单栏、工具箱和状态栏

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

- `MenuBar` 遵循 ARIA menubar 模式，包含与命令面板相同的命令。
- `Toolbox` 是一组纵向的切换按钮；所选工具以 `zeyn:tool` 事件发送。
- `PropertyInspector::new(title, form)` 用被 `.section(...)` 分组的表单编辑所选对象。
- `StatusBar` 带有 live region，因此发送给它的消息会被屏幕阅读器读出。

## 停靠面板

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

用户可以在区域之间拖动标签页，用指针或键盘调整区域大小，并关闭面板；菜单中的 `MenuList::panels` 可以将它们找回。布局按用户和 screen 保存在应用的视图存储中；应用没有视图存储时则保存在浏览器中。
