# Screen 和参数

screen 是一个带路由的页面，或者是在另一个 screen 之上打开的窗口。`#[screen]` 像 action 一样声明其访问权限。

## 页面

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` 是页面；`Screen::window(title)` 是窗口。
- 多次调用 `.body(...)` 会将多个组件上下排列。
- screen 返回 `Screen` 或 `Result<Screen>`。

## 参数

screen 的第一个参数是它的参数（params）：一个 `#[params]` 结构体，没有参数时写 `_: ()`。其他参数与 action 中一样被注入。

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

参数在所有使用之处都带有类型：`Window::open(product_editor, 7)` 无法编译，因为编辑器接受的是 `ProductParams`。

当 `#[params]` 结构体的所有字段都是 `Option` 时，屏幕也可以不带参数打开（来自菜单链接或普通地址），所有字段都是 `None`。非可选字段仍然需要值：缺少时屏幕返回 404。

没有 `route` 的 screen 没有 URL。它通过链接、工具栏按钮、菜单或命令以窗口形式打开。

## 路径中的参数

路由可以将 params 的字段命名为路径段。每个 `{name}` 绑定同名字段，并按该字段的类型读取：

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` 打开订单 42。不在路径中的字段仍像以前一样通过 JSON 查询传递：`/orders/42?params={"tab":"lines"}`。
- 路径段接受整数、实体键（`OrderId`）、`String` 和 `Slug`。值只有按框架写出的形式才能解析（`7`，而不是 `007` 或 `+7`）；其他任何形式都返回 404。
- 路径段没有对应 params 的任何字段、在没有 params 的 screen 上使用带路径段的路由，或字段类型不能作为路径段，都无法编译，并且错误信息会说明该如何修改。
- 多个路径段的工作方式相同：`#[screen(route = "/catalog/{slug}/{n}", public)]`，配合 `slug: Slug` 和 `n: u32`。
- 框架写出的链接使用路径形式：`Effect::navigate`、面包屑、命令以及登录后的 `return_to`。没有路径段的 screen 保持 `?params={json}`。
- 带路径段的 screen 不能作为菜单项或主页 screen，因为没有任何东西可以填充这些路径段。
- 与任何 screen 一样，每个请求都会检查访问权限。

### Slug

`Slug::from_title("Çay Bardağı")` 得到 `cay-bardagi`：小写 ASCII，单词以 `-` 连接，土耳其字母被映射（`ı` 和 `İ` 变为 `i`，`ş` 变为 `s`，`ğ` 变为 `g`，`ç` 变为 `c`，`ö` 变为 `o`，`ü` 变为 `u`），其他拉丁字母的重音符号被去除，其余内容被舍弃。slug 最长 80 字节。在路径中只有规范形式才能解析，因此 `/catalog/Cay-Bardagi/1` 返回 404。

### 未找到与已迁移

当记录不存在时返回 `Err(Error::NotFound)`；用户会看到 404 页面。当地址发生了变化（例如产品被重命名）时，用新地址作答。浏览器会收到永久重定向（301），应用内导航会无需整页加载地跟随它：

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)` 把一个请求转交给另一个 screen，例如随机选取或最新条目，同时地址本身依然有效。浏览器收到 `303 See Other`，其中带有目标的规范路径，以及 `Cache-Control: no-store`；应用内导航会无需整页加载地跟随它，地址栏显示目标地址。目标始终是带路由的类型化 screen，因此重定向绝不会指向应用之外的地址，应用未提供的目标会返回 404。

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## 访问权限

| 选项 | 含义 |
|---|---|
| `permission = "..."` | 身份需要具有此权限 |
| `public` | 任何人。没有会话的访客获得不带 cookie 的页面（参见[公共页面](public-pages.md)） |
| `cache = 300` | 仅限公共 screen：浏览器和共享缓存可以在这么多秒内保留没有会话的访客所看到的页面（最多 86400） |
| `sensitive` | 打开它会被记录为对个人数据的一次读取 |
| `reason` | 用户必须先给出理由才能打开（参见[审计、脱敏和作业](enterprise.md)） |

页面还可以通过 `Screen` 上的 `.description(..)`、`.canonical(..)`、`.og_image(..)`、`.json_ld(..)` 和 `.robots(..)` 向搜索引擎和链接预览介绍自己；参见[公共页面](public-pages.md#search-engines-and-link-previews)。

用户无权打开的 screen，其菜单项、工具栏按钮和命令会从 HTML 中省略。

面向未登录访客的 screen 声明 `public`。它的 action 需要各自的访问声明：列出数据的公共 screen 调用 `public` action，否则列表对访客来说是空的。

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

同时省略 `permission` 和 `public` 是编译错误，因此 screen 绝不会意外对外开放。

## 工具栏和上下文菜单

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

像 `Alt+N` 这样的快捷键在 screen 上生效，并显示在命令面板中。浏览器自己使用的快捷键（`Ctrl+P`、`Ctrl+S`、`F5`）不会被绑定，除非按钮调用 `.override_browser()`；浏览器从不转交的快捷键（`Ctrl+N`、`Ctrl+T`、`Ctrl+W`）则永远不会被绑定；可用的组合是 `Alt+Shift+<字母>` 和 `Ctrl+Alt+<字母>`。

## 页眉

`.subtitle(text)` 在标题下方放一行简短说明，`.primary(|t| ...)` 把屏幕的主要操作作为主按钮放在标题行的末尾。两者合起来就是大多数管理界面都有的页眉。

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary` 接受与 `.toolbar` 相同的按钮，快捷键和命令面板条目也相同；工具栏仍在页眉下方，放其他工具。副标题是屏幕阅读器读到的屏幕描述（`aria-describedby`）。行太窄时，操作会换到标题下方，在手机上靠起始边对齐。没有副标题和主要操作时，屏幕照旧只渲染一个普通标题。

## Feature 和菜单

screen 属于某个 feature。feature 的路由前缀位于路由之前，其 guard 在 screen 打开之前运行，其菜单列出各个 screen：

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` 使 `/` 打开一个 screen。

## 错误页面

`App::not_found(screen)` 和 `App::server_error(screen)` 为应用提供自己的 404 页面和服务器错误页面：

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- 404 screen 响应未知路径以及每个返回 `Err(Error::NotFound)` 的 screen，包括无法解析的路径值。
- 服务器错误 screen 响应 `Error::Internal`、超时（504）和处理程序中的 panic。错误的详细信息只进入服务器日志；该 screen 不接收 params，也无从得知请求的任何内容。
- 两者都在 shell 内部以正确的状态码显示。应用内导航只获取该 screen，并无需整页加载地将其替换进去。框架端点和 API 仍然以 JSON 作答。
- 错误 screen 不接受 params，不需要路由，也不属于任何 feature。请将它们设为 `public`：无权打开它的用户，或自身也失败的错误 screen，会得到内置的简短消息。

## screen 如何到达浏览器

第一个请求得到完整页面：菜单、screen 和资源。之后应用内的链接只加载 screen 并原位替换，因此菜单、展开状态和滚动位置得以保留。后退按钮、新标签页和书签的工作方式与普通链接相同。
