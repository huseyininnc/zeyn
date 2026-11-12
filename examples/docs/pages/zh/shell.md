# Shell 和命令

shell 是每个 screen 外围的框架：菜单、语言、主题和时区选择器、toast 提示以及命令面板。像桌面工具那样工作的应用还会增加菜单栏、工具箱、属性检查器、状态栏和停靠面板。

## 菜单

菜单来自各 feature 的 `.menu(...)` 调用。用户无权打开的项会从 HTML 中省略，guard 拒绝该用户的 feature 则什么也不显示。当前 screen 的链接带有 `aria-current="page"`，并绘制在品牌色的淡色背景上。

在 `.item(...)` 之后调用 `.icon(Icon::...)` 会在该项标签前放一个图标；一个组的各项有图标后，组标签就不再绘制自己的图标。侧边菜单以应用的徽标和名称开头：`App::logo("/static/logo.svg")`（深色主题用 `App::logo_dark(...)`）设置一个由应用提供的标志，否则 shell 会绘制一个 `--z-brand` 色的圆角方块，里面是名称的首字母。shell 保持中性；品牌色只标记当前项、主按钮、焦点环和徽标。

图标来自一套 82 个图形（`Icon::ALL`），绘制在 20 px 网格上，使用 1.5 px 圆角线条，以当前文字颜色按 16 或 20 px 显示。每个图标都是 `/_zeyn/icons.css` 中的一个 CSS 遮罩，这是由 `crates/zeyn/assets/icons/*.svg` 生成、约 25 KB（压缩后 3 KB）并会被缓存的样式表，因此一个图标在 HTML 中只占一个 class，在脚本中不占任何开销。

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

### 嵌套和可折叠的分组

`.sub(label, |m| ...)` 在最后一个分组中添加嵌套分组；在其中，`item`、`icon`、`open` 和 `sub` 的用法与分组相同。嵌套分组排在其所在分组的条目之后，最多比顶层分组深三层（`MENU_DEPTH`）；更深的菜单会在启动时停止应用，并给出指明该分组的消息。

```rust
.menu(|m| {
    m.group(t!("sales.menu"), Icon::Cart)
        .item(t!("sales.orders"), orders)
        .sub(t!("sales.reports"), |m| m.item(t!("sales.by_city"), by_city).item(t!("sales.archive"), archive))
        .accordion()
})
```

- **可折叠。** 每个分组标题都是带 `aria-expanded` 和 `aria-controls` 的按钮（W3C APG 展开式导航模式）。服务器把包含当前屏幕的分组渲染为展开，其余为折叠；每次导航后 `nav.js` 会展开新屏幕所在的分支。没有脚本时所有分组保持展开，因此不会有内容无法访问。
- **始终展开。** 在分组后调用 `.open()` 使其保持展开：标题是不带按钮的普通文本。
- **手风琴。** 在 app shell 中任一 feature 的菜单上调用 `.accordion()`，整个侧边菜单变为手风琴：展开一个分组会折叠其已展开的同级分组。
- **键盘。** 上和下在可见的标题和链接之间移动，Home 和 End 跳到第一个和最后一个。右键展开折叠的分组或进入已展开的分组；左键折叠已展开的分组或移到外层分组的标题。Enter 和空格切换分组。在从右到左的语言中左右互换。
- **权限与合并。** 同名分组在每一层跨 feature 合并；用户无法打开其中任何条目的分组（包括嵌套分组）不会显示。`App::header(|h| h.breadcrumb())` 的面包屑显示完整的分组路径。site shell 把嵌套分组的链接列在其顶层分组中。

### 图标栏、徽章、筛选和手机

```rust
.menu(|m| {
    m.group(t!("mail.menu"), Icon::Mail)
        .item(t!("mail.inbox"), inbox)
        .count(unread)
        .count_listens("mail")
        .item(t!("mail.rules"), rules)
        .badge(t!("menu.new"))
})
```

- **图标栏。** 在 1024 px 及以上的屏幕上，菜单栏中的按钮可以把菜单折叠为顶层分组的图标（或其首字母）。分组名称以提示显示，分组在图标栏旁以弹出面板打开；Escape、点击外部和导航会关闭它。选择保存在 `zeyn_rail` cookie 中，因此下一页以同样方式绘制，不会跳动。
- **徽章和计数。** 在菜单项后使用 `.badge(text)` 添加简短的固定文字，例如“新”。`.count(action)` 添加一个由接受 `()` 并返回 `u64` 的 action 计算的数字：页面显示后用一次请求获取菜单中的所有计数（`GET /_zeyn/menu/counts`），所以缓慢的计数不会拖慢页面。没有该 action 权限的用户看不到数字，0 不显示，大于 99 显示为“99+”。当某个 action 通过 `ui.refresh(..)` 提到其中一个主题或实时 socket 发出通知时，`.count_listens(topics)` 会重新计数。
- **筛选和收藏。** 拥有 10 个或更多链接的侧边菜单（`MENU_FILTER_FROM`）会有一个筛选框：输入时隐藏不匹配的链接（忽略大小写和重音），展开匹配链接所在的分组并告知剩余数量；Escape 清空。每个链接都有一个固定按钮，固定的链接显示在顶部的收藏分组中，按用户保存在浏览器里。
- **手机。** 小于 1024 px 时，菜单折叠到菜单栏中的按钮后面，并以抽屉形式覆盖在屏幕上打开。焦点进入抽屉并停留在抽屉与按钮之间，后面的屏幕不可交互也不会滚动；按钮、Escape、背景、链接或朝菜单边缘的滑动都会关闭抽屉，并把焦点还给按钮。

### 每次请求构建的菜单

`App::menu_source(source)` 添加在每次请求时构建的菜单项，例如来自按用户角色筛选的菜单表。菜单源是一个 async 函数，接收 `Ctx`（身份、租户、语言）并返回 `MenuNode`：顶层是 `MenuNode::group(label, children)`，其中包含 `MenuNode::screen(label, "screen_name")` 链接（`.params(json)` 像 `screen_url` 一样填充路径片段和查询）、`MenuNode::path(label, "/reports/sales")` 链接以及嵌套分组。`.icon(...)`、`.open()` 和 `.permission("menu.reports")` 可用于任何节点。标签可以是纯文本（与其他文本一样会被转义），也可以是通过 `Text::key` 指定的目录键。

这些分组排在各 feature 的静态菜单之后，与静态分组标签相同的分组会合并进去。面包屑、命令面板和 `nav.js` 像对待其他菜单链接一样对待这些链接。

下面的表可用于 PostgreSQL 和 Oracle。一条扁平的 `SELECT` 读取它，树在 Rust 中按 `parent_id` 构建：

```sql
CREATE TABLE menu_items (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES menu_items (id),
    label       VARCHAR(100) NOT NULL,
    screen      VARCHAR(100),
    path        VARCHAR(500),
    icon        VARCHAR(40),
    permission  VARCHAR(100),
    sort_order  INTEGER DEFAULT 0 NOT NULL
);
```

```rust
#[model]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub label: String,
    pub screen: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub permission: Option<String>,
    pub sort_order: i64,
}

async fn menu(ctx: Ctx) -> Result<Vec<MenuNode>> {
    let db = ctx.service::<Db>().ok_or_else(|| Error::Internal("no database".into()))?;
    let rows: Vec<MenuRow> = db
        .query("SELECT id, parent_id, label, screen, path, icon, permission, sort_order FROM menu_items ORDER BY sort_order, id", vec![])
        .await?;
    Ok(tree(&rows, None))
}

fn tree(rows: &[MenuRow], parent: Option<i64>) -> Vec<MenuNode> {
    rows.iter()
        .filter(|r| r.parent_id == parent)
        .map(|r| {
            let mut node = match (&r.screen, &r.path) {
                (Some(screen), _) => MenuNode::screen(r.label.clone(), screen.clone()),
                (None, Some(path)) => MenuNode::path(r.label.clone(), path.clone()),
                (None, None) => MenuNode::group(r.label.clone(), tree(rows, Some(r.id))),
            };
            if let Some(icon) = Icon::ALL.iter().find(|i| Some(i.name()) == r.icon.as_deref()) {
                node = node.icon(*icon);
            }
            match &r.permission {
                Some(p) => node.permission(p.clone()),
                None => node,
            }
        })
        .collect()
}

App::new("Admin").menu_source(menu)
```

- **从不信任。** 服务器在每次请求时检查每个链接：指向未知 screen 的链接会被丢弃，并在日志中记一条警告；指向用户无权打开的 screen 的链接（访问权限、feature 的 guard、节点自己的 `permission`）即使菜单源返回了也会被丢弃；params 无法填满路径片段的 screen 链接同样会被丢弃。路径必须以单个 `/` 开头：不能有协议、`//`、反斜杠或控制字符。分组最多嵌套 `MENU_DEPTH` 层，菜单源的一次返回最多保留 `MAX_MENU_NODES`（500）个节点，标签在 `MAX_MENU_LABEL`（100）个字符处截断。
- **缓存。** 返回结果按租户、语言和权限集合缓存 `MENU_TTL`（60 秒），因此角色相同的用户共享一份结果，菜单源为他们每分钟只运行一次。缓存最多保存 `MAX_MENU_CACHE`（256）份结果，并先丢弃最旧的。如果菜单源的结果不仅取决于权限，还取决于用户本身，需要使用 `App::menu_per_user()`。action 修改菜单表后，调用注入的 `Menus` 服务的 `invalidate()`。
- **从不让页面失败。** 当菜单源返回错误、发生 panic 或耗时超过 `MENU_TIMEOUT`（2 秒）时，页面显示静态菜单，服务器记录这次失败。失败不会被缓存。
- **显示位置。** 该菜单属于 app shell：screen 的整页加载和错误页面都包含它，而站点 shell 和登录页面只显示静态菜单。

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
- **窄屏幕。** 在 1024 px 以下，菜单折叠到带 `aria-expanded` 的菜单按钮后面；Escape 将其关闭。没有脚本时菜单保持展开，因此没有任何内容变得无法访问。
- **头部插槽。** 顶部栏有四个可放置任意组件的插槽，类似使用逻辑名称的应用栏：`start(node)` 在品牌之前，`center(node)` 在菜单与末端之间，`end(node)` 在栏的末端，`bottom(node)` 是其下方的整宽一行，例如放 chip。每个插槽按顺序容纳多个节点，在从右到左的语言中 start 和 end 会交换位置。插槽中的组件使用用户的语言、权限和掩码渲染；通过自己的 action 加载数据的组件（例如 `SearchBox`）也能在其中工作，并按该 feature 的 guard 和 action 的权限检查。grid 和表单属于 screen。在手机上，start 和 end 插槽留在栏中，center 插槽则按应用指定的位置放置：`center_on_phone(CenterOnPhone::Row)`（默认）把它放在栏下方的一行，`CenterOnPhone::Icon` 把它放在放大镜按钮之后，由按钮打开（W3C APG 披露模式）并把焦点移入其中；Escape 将其关闭并回到按钮。`center_button(icon, label)` 可更改该按钮。手机布局由 CSS 在脚本运行之前设定，因此页面加载时头部不会移动。
- **操作。** 当应用有登录页面时，`sign_in()` 对访客显示登录链接，对已登录用户显示退出链接；`language()` 和 `theme()` 显示与应用 shell 相同的选择器。它们排在 end 插槽之后，并在手机上折叠进菜单。
- **底部栏。** `bottom_nav([...])` 添加最多五个固定在手机和平板屏幕底部（宽度小于 1024 px）的链接，用于人们最常去的地方；超过五个则无法编译。每个条目是 `BottomNavItem`，包含标签、带路由的屏幕和可选图标。当前显示页面的链接带有 `aria-current="page"`（由 `nav.js` 保持更新），用户无权打开的屏幕的链接会被省略，底部栏通过 `env(safe-area-inset-bottom)` 避开主屏指示条，页面底部会留出空间，因此底部栏不会遮挡任何内容。顶部菜单仍用于其余内容。底部栏不需要脚本。
- **页脚。** 若干列链接，指向 screen（`link`，对无权打开的用户省略）或地址（`url`：路径、`https:`、`http:` 或 `mailto:`；`App::check` 会拒绝其他协议以及指向未注册 screen 的链接），外加一行文字。

在站点 shell screen 与应用 shell screen 之间切换，通过 `nav.js` 进行，无需整页加载：局部响应会说明它属于哪个 shell（`x-zeyn-shell`），当它与当前框架不同时，运行时会像切换语言那样替换整个框架，并加载另一个 shell 的样式表和脚本。指向 `/_zeyn/route/{screen}` 的链接（面包屑的链接）也以同样方式被跟随。

## 登录 shell

登录页面有自己的框架：`Shell::auth()` 没有菜单、没有顶栏，也没有命令面板。应用的徽标（`App::logo`）和名称位于一张 400 px 宽的卡片上方，语言和主题选择器位于页面底部，因此登录前两者都可以使用。`.split()` 从 1024 px 起把表单放在一半，另一半是 `--z-brand` 色的品牌面板；更窄的屏幕显示居中的卡片。在手机上卡片就是整个页面，其中的输入框和按钮高 44 px。它的样式（`auth.css`，3.5 KB）只在这些页面加载。

`zeyn::accounts::ui` 的每个登录相关屏幕（登录、注册、忘记和重置密码、两步登录、恢复码、锁定、确认和退出）都在其中打开，而账户和管理屏幕保留应用 shell。自定义登录屏幕通过其 feature 使用它：

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

登录表单用 `Control::Username`（`autocomplete="username"`）填写用户名，用 `current-password` 填写密码，把显示密码的开关显示为字段内的眼睛图标，并在大写锁定开启时在字段下方提示“大写锁定已开启”（polite 实时区域）。

## 屏幕中的主题选择器

`ThemePicker::new()` 把主题选择器放进屏幕里，例如设置页面的“外观”部分。它提供 `App::themes` 中的主题（应用跟随设备时还有“系统”），并显示当前使用的主题。

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

它与外壳中的选择器是同一个控件：选择会立即生效，不重新加载页面，保存在 `zeyn_theme` cookie 中（已登录用户还会保存在视图存储中），页面上的另一个选择器也会同步显示。应用不提供选择时（`theme_picker(false)` 或只有一个主题），它不渲染任何内容。

## 应用页眉

`App::header(|h| ...)` 在应用外壳的屏幕上方放一个栏，带有站点页眉的插槽：`start`、`center`、`end` 和 `bottom`。搜索框放在中间，通知链接放在末尾。`sign_in()`、`language()` 和 `theme()` 在这里同样可用，不过侧边菜单里已经有这些选择器。

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

三个现成的部件让顶栏成为应用框架：

- `breadcrumb()` 让顶栏以菜单组和当前 screen 的标题开头；`nav.js` 在每次导航后重写它。
- `search()` 添加一个打开命令面板的搜索字段，并显示其快捷键；命令面板按钮会从侧边菜单中移除。在手机上它是一个图标按钮。
- `account()` 让顶栏以一个显示用户姓名首字母的圆形按钮结尾。它打开一个对话框（W3C APG 非模态对话框；Escape 和关闭按钮可关闭它），其中显示当前登录的用户、主题和语言选择器以及退出链接，访客则看到登录链接。这些选择器会从侧边菜单中移除。

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

屏幕滚动时，这个栏固定在顶部，屏幕在它下方切换，不重新加载页面。在手机上，它位于菜单按钮下方，中间插槽单独占一行。栏中的组件像屏幕中的组件一样按用户渲染；搜索框通过自己的动作加载建议，服务器会检查该动作的权限。没有 `App::header` 时，应用外壳没有顶栏。

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

默认情况下 **Alt+Shift+P** 打开命令面板（`App::palette_shortcut` 可以更改，也可以特意改为 `Ctrl+Shift+P` 之类的浏览器按键）：它是一个 ARIA 组合框，搜索应用的命令、当前 screen 的工具栏和上下文菜单，以及菜单中的 screen。最近使用的命令排在前面。焦点不在输入字段中时，按 `/` 也能打开它，`App::header(|h| h.search())` 的搜索字段同样可以。结果按所在位置分组（“前往 · 销售”、screen 标题、“命令”），每项带有图标，匹配的字母加粗，快捷键显示在末尾。菜单项直接在页面上匹配，因此第一次按键就能找到它们。

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
- `PropertyInspector::island(name)` 编辑岛中选中的对象，不使用表单（见 [Island inspector](#island-inspector)）。
- `StatusBar` 带有 live region，因此发送给它的消息会被屏幕阅读器读出。

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

岛发送 `zeyn:inspect` 事件，`detail: { title, schema, values }`，其中 `schema` 是序列化后的 `FormSchema`。字段类型有 `text`、`multiline`、`integer`、`decimal`、`length`、`color`、`boolean`、`choice` 和 `expression`；每个字段都可以有 `required`、`min`、`max`、`max_len`、`default` 和 `help`。

检查器用模型表单的控件绘制字段，并把名称、标签和值作为文本而不是 HTML 填入。离开字段、按 Enter 或关闭选择器时，它会校验值，并向岛的根元素发送带 `detail: { name, value }` 的 `zeyn:property`。无效的值会显示错误，不会发送。超过 200 个字段的 schema 会被忽略。`FieldSpec::check` 在服务器上执行相同的校验。

## 停靠面板

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

用户可以在区域之间拖动标签页，用指针或键盘调整区域大小，并关闭面板；菜单中的 `MenuList::panels` 可以将它们找回。布局按用户和 screen 保存在应用的视图存储中；应用没有视图存储时则保存在浏览器中。
