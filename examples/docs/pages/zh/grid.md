# 数据网格

状态：由 `zeyn::DataGrid`、`zeyn::server` 中的网格端点以及 `assets/zeyn/*.js`（以 `/_zeyn/zeyn.js` 提供）实现。

网格在服务器端渲染。它的行来自一个接受 `DataQuery` 并返回 `Page<E>` 的 action。浏览器运行时请求页面，发送用户的操作（排序、过滤、布局、编辑、action），并把收到的 HTML 放回原位。每次调用都在服务器上检查权限，客户端发送的任何内容都不被信任。

```rust
DataGrid::of(search_products)
    .mode(DataMode::Server)
    .paging(Paging::pages(25))
    .column(Product::code).width(120).pin_start().filter()
        .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
    .column(Product::name).flex(1).filter().editable()
    .column(Product::unit).filter().editable().badge(|u| match u {
        Unit::Kg => Tone::Info,
        _ => Tone::Neutral,
    })
    .column(Product::price).filter().editable().avg().max()
    .column(Product::stock).filter().sum().progress(120)
    .column(Product::active).filter().check_icon().editable()
    .edit_with(save_product)
    .row_action(t!("stock.restock"), restock_product, |p| p.id)
    .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
    .row_menu(|p| ContextMenu::new()
        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
        .action(t!("stock.restock"), restock_product, p.id))
    .exportable()
```

## 运行时才确定的列

`DataGrid::dynamic(action)` 显示只有在查询运行后才知道列的结果,例如 SQL 预览或数据源浏览器。action 接收一个 `DataQuery` 并返回一个 `DynamicPage`:列(`DynamicColumn { name, label, kind }`)和一页行,每行是按列顺序排列的 JSON 数组,或按列名索引的 JSON 对象。

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

grid 从每一页构建自己的列,并用类型化 grid 自己的代码渲染行,因此分页、虚拟滚动、列虚拟化、键盘操作、复制以及调整列宽、移动和隐藏列都相同。`DynamicKind` 决定列如何读取和显示值:`Text`、`Integer`(按用户的区域设置分组)、`Decimal`、`Bool`(用用户的语言显示是和否)、`Date`、`Time`、`DateTime`(用户的时区)和 `LocalDateTime`。无法按其类型读取的值显示为文本。每列以适合其类型的宽度开始(文本 180 px,数字 130,日期 120),因此很宽的结果会横向滚动。

action 返回的一切都被视为用户数据。列名、标签和单元格像其他值一样被转义。服务器最多保留 `MAX_DYNAMIC_COLUMNS`(200)列和查询页大小的行(`DataQuery::limit`,最多 500),并将名称和标签截断为 `MAX_COLUMN_NAME`(200)个字符、文本单元格截断为 `MAX_CELL_TEXT`(1,000)个字符。排序键、筛选、合计和分组在 action 运行前从查询中移除,表头也没有排序按钮,因此 action 不会从客户端得到任何列名。访问权限与所有 grid 一样取决于 action。

空名称会变成 `col_<序号>`,重复的名称会加上 `_2`、`_3`;对象行仍按 action 自己的名称读取。一页最多保留 `MAX_PAGE_CELLS`(50,000)个单元格和 `MAX_PAGE_TEXT`(8 MB)的单元格文本。每个文本单元格平分文本预算,因此很宽的页面显示的文本更短;超出单元格预算的行会随下一页返回,由该页的 `next` 指向。嵌套的 JSON 值只写到其份额为止。一个在 action 中占 403 MB 的回答(500 行、200 列、每格 1,000 个表情符号)在 action 自身的回答之外没有占用内存,并发送了 4.6 MB。

`DynamicColumn::masked(MaskStyle::LastFour)` 在读取页面时立即对列进行掩码,原始值永远不会到达 HTML、日志或 AI 代理,也没有人能显示它们。`DynamicGrid::sensitive(model, key)` 会把显示的每一行记录为对 `model` 的一次读取并审计,以 `key` 列作为键,就像 `sensitive` 实体的行一样。没有这些设置时,由 action 决定结果可以显示什么。

列发生变化时,浏览器会丢弃它为旧列保留的行。已保存的视图只用于创建它时的那些列。

表头带有列的指纹(`data-zeyn-shape`)。浏览器在每次请求页面时发送它,服务器仅在列发生变化时才发送新的表头,例如 `.listens(topic)` 用新查询重新加载 grid 之后。行是只读的:动态 grid 没有编辑、行操作、选择、明细、分组和导出。

## 列合计

`sum()` 和 `avg()` 适用于整数和金额，`min()` 和 `max()` 还适用于日期、时间和时刻，`count()` 适用于每一种列。用在其他类型上无法编译。合计覆盖所有符合过滤条件的行，而不仅仅是当前页。

`DataQuery::apply` 为内存中的数据计算合计。在 SQL 中分页的数据层则读取 `query.aggregates`，并通过一次聚合查询添加 `Page::with_total(field, op, value)`。网格只保留其列所声明的合计，因此客户端无法请求其他合计。合计在服务器上按用户的语言格式化。整数的平均值保留两位小数（`Value::Decimal`），金额的平均值按“四舍五入（远离零）”舍入到最小单位。

## 过滤与排序

`filter()` 会在过滤行中添加一个过滤器，其输入控件由列类型决定：

| 列 | 过滤器 | 查询 |
|---|---|---|
| 文本 | 包含、开头为、等于 | `contains`、`starts_with`、`equals` |
| 整数、金额 | 从、到 | `from`、`to`，如 `1234.56` |
| 日期 | 从、到 | `from`、`to`，如 `2026-03-31` |
| 时刻（`DateTime`） | 从、到（日期） | 用户时区下的整天 |
| 时间 | 从、到 | `14:30` |
| Choice | 选取任意选项 | `any_of` |
| 是/否 | 全部、是、否 | `is` |

无论用户使用什么语言，值都以固定的机器格式传输。旧的 `{ "field", "contains" }` 形式仍然可用。服务器会丢弃针对没有 `filter()` 的列的过滤，以及针对网格中不存在的列的排序，并限制过滤条件（32 个）、过滤文本（200 个字符）、选项（100 个）和排序键（4 个）。无法读取的值会被忽略。日期过滤所用的时区来自服务器上的用户设置。

在表头上按住 Shift 点击，会把该列加入排序键。表头中的数字表示排序的顺序。

## 列：固定、调整大小、隐藏、移动

`pin_start()` 和 `pin_end()` 让某一列在网格横向滚动时停留在某一边缘（每一侧最多三列）。`hidden()` 使一列保持隐藏，直到用户显示它。每个表头都有一个列菜单（通过按钮、右键点击、Shift+F10 或上下文菜单键打开），包含排序、固定、左移和右移、隐藏、过滤、列选择器和重置。表头的边缘可以调整列宽，通过拖动或方向键（按住 Shift 步长更大）。把一个表头拖到同一固定组内的另一个表头上可移动该列；短按仍然是排序。以上全部功能都支持从右到左的布局。

## 搜索

`search()` 在表格上方显示一个搜索框。用户输入的内容在短暂停顿后作为 `DataQuery::q` 传给表格的
动作,随每一页、每个块、每个分支、对所有匹配行的批量操作和导出一起发送;新的搜索会取消之前的请求。
`DataQuery::apply` 和 SQL 数据层在文本字段中匹配它,忽略大小写和重音,并且从不搜索掩码字段。
没有 `search()` 的表格会丢弃客户端发送的任何 `q`。结果会在表格的实时区域中播报,搜索没有结果时
显示 `empty_filtered`。搜索文本不属于已保存的视图。AI 代理可以在表格的工具模式中找到 `q`。

## 分组表头

`band(label, |b| ...)` 把闭包添加的列归到同一个表头单元格下,该单元格位于列标题上方的一行:

```rust
DataGrid::of(search_products)
    .search()
    .column(Product::code).pin_start()
    .band(t!("stock.price_and_stock"), |b| b
        .column(Product::price).avg()
        .column(Product::stock).sum())
    .column(Product::active)
```

分组是一个带 `scope="colgroup"` 的 `th`,其 `colspan` 覆盖它显示的列;不属于任何分组的列上方的
单元格保持为空。分组行跟随用户的布局:隐藏的列离开分组,分组为其在同一固定侧保持相邻的每一段列显示
一个单元格(把列移开会把分组拆开),固定的段与它们的列一起停留,列虚拟化也会保持分组行完整。导出和
复制不包含分组。

## 已保存的视图

用户的布局（显示的列及其顺序、宽度、固定）、排序和过滤构成一个 `DataGridView`。使用 `App::datagrid_views(store)` 时，它会按用户保存在服务器上，范围是每个租户、屏幕和网格，页面返回时就带着该视图，首页已经完成排序和过滤。请为数据库实现 `DataGridViewStore`；`MemoryViewStore` 用于开发和测试。没有存储时，浏览器会把视图保存在 `localStorage` 中。服务器会用网格自身的列和限制清理它收到的每一个视图。

## 单元格

| 方法 | 列 | 显示 |
|---|---|---|
| `badge(\|v\| Tone)` | `#[choice]` 枚举 | 每个选项一个彩色徽章 |
| `progress(max)` | 数字、金额 | 一个进度条和数字 |
| `link(\|row\| Effect)` | 任意 | 作为链接的值，点击运行一个 effect |
| `check_icon()` | `bool` | 一个对勾或一个短横线，并为屏幕阅读器提供文字 |
| `relative_time()` | `DateTime` | “3 分钟前”，精确时间显示为提示 |
| `cell(renderer)` | 任意 | 你自己的 HTML（`CellRenderer<T>`） |

`CellRenderer` 必须对用户数据进行转义。导出和复制使用纯值。

## 行 action、菜单和批量 action

`row_action(label, action, |row| input)` 为每一行添加一个按钮。`row_menu(|row| ContextMenu)` 添加一个上下文菜单。`Screen::context_menu(...)` 在屏幕的空白区域添加一个。运行 action 的菜单项只对拥有该 action 权限的用户显示。

当某一行的菜单项运行时，客户端只发送行主键和该菜单项。服务器会通过网格的 action 重新加载该行，基于该行构建菜单项，检查该 action 的权限，运行拦截器，然后运行 action。对该行已不存在的菜单项（例如变为禁用的那个）会被拒绝。输入绝不来自客户端。

`bulk_action(label, action, Model::key)` 在选择栏中添加一个按钮。该 action 接受 `Vec<K>`，即主键字段的类型；其他类型无法编译。选择“所有匹配的行”会让服务器根据用户的过滤条件自行收集主键，最多 10,000 个。逐个选中的主键来自客户端，因此 action 必须像 API action 一样检查每条记录。

在其他所有地方，浏览器保留自己的上下文菜单。`App::browser_context_menu(false)` 会将其关闭，文本输入框和可编辑文本除外。这起不到任何保护作用：用户仍然可以查看页面源码、打开开发者工具或关闭脚本。它还会使拼写检查、复制链接、翻译和辅助工具失效。请保留默认设置。

## 行内编辑

对一个接受 `Valid<E>` 的 action 使用 `edit_with(save)`（与表单的规则相同），并在列上使用 `editable()`。主键、只读字段和行版本永远不会变为可编辑。F2 或双击可编辑单元格开始编辑该行；Enter 保存，Escape 取消，Ctrl+Z 或提示条上的按钮撤销上一次保存。

服务器按主键重新加载该行，并只通过 `Form::read` 应用可编辑的列和行版本，因此值的读取和验证与表单一致。对任何其他列提交的值会以 `403` 被拒绝。过期的版本返回 `409`，验证错误返回 `422` 并显示在对应的单元格中。

## 添加、删除和批量编辑行

```rust
DataGrid::of(search_products)
    .column(Product::name).editable()
    .column(Product::price).editable()
    .edit_with(save_product)
    .add_with(create_product)
    .delete_with(delete_products, Product::id)
    .batch_edit()
```

`add_with(create)` 在表格上方显示"添加行"按钮。它在顶部打开一个空行,每个 `editable()` 列都有一个
输入框;Enter 保存,Escape 放弃。服务器从模型的默认值开始,像表单一样只读取可编辑的列(其他字段
以 `403` 拒绝,无效的值以 `422` 返回并显示在对应单元格中),检查 `create` 的权限后运行它,然后
重新加载表格。虚拟表格不添加行。
模型不能保留默认值的字段(例如非可选的日期)需要一个可编辑的列。

`delete_with(action, Model::key)` 在选择栏中加入一个运行前会先询问的红色"删除"按钮,在某一行上按
Delete 键会删除选中的行,没有选中时删除该行。动作像批量动作一样接收键,因此必须检查每条记录。

`batch_edit()` 会保留行内编辑,直到用户全部保存。按 Enter 后,改动过的行保持编辑状态并在开头带有
标记,表格上方的栏显示改动行的数量以及"保存更改"和"放弃更改"。保存时各行逐一通过行内编辑的路径
发送,因此每行都单独读取、验证并检查版本;失败的行保留其改动并显示原因。加载其他行(翻页、排序、
筛选、搜索)前,会先询问是否放弃未保存的更改。

## 主从明细

`detail(|row| (screen, params))` 让每一行都能展开成一个基于该行构建的屏幕，只在用户打开时才加载：

```rust
DataGrid::of(search_today)
    .column(Order::id)
    .detail(|o| (order_lines, OrderParams { id: o.id }))

#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        DataGrid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .column(OrderLine::product)
            .column(OrderLine::amount).sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}
```

params 的类型由屏幕决定（`ScreenDef::Params`），所以错误的 params 无法编译。明细是一个完整的屏幕：可以是表单、嵌套的网格，或者带有自己明细的网格，嵌套深度不限。展开列只对有权打开该屏幕的用户显示，并且在明细加载时，服务器会再次检查该屏幕的权限。params 与窗口的 params 一样经由浏览器传递，因此明细屏幕必须检查用户是否有权查看其所指明的记录。

`scope(field, value)` 会在每次请求中把网格限定在 `field` 等于 `value` 的行上。该过滤条件排在 `DataQuery::filters` 的最前面，客户端无法移除它，所以无论浏览器发送什么，嵌套的网格都停留在其父行上。根据过滤条件构建 SQL 的数据层会像读取其他过滤条件一样读取它（数字和日期使用上下界相等的 `Filter::range`，文本使用 `equals`，choice 使用 `any_of`，是/否使用 `is`，空值使用 `blank`）。

展开按钮、在其上按 Enter，或在该行上按右方向键和左方向键，都可以展开和收起明细。收起会将其移除，所以下次会重新加载。

虚拟网格让每一行保持同一个固定高度，以便立即到达任何位置；它们不显示展开按钮。对于带有明细的网格，请使用分页或无限分页。

## 分组

`group_by(field)` 在服务器上对行分组；再次调用则在每个组内部继续分组：

```rust
DataGrid::of(search_today)
    .paging(Paging::pages(20))
    .group_by(Order::city)
    .group_by(Order::status)
    .column(Order::amount).sum().avg()
```

网格为每个组显示一个表头行，包含列标签、值、行数，以及该组内每一列的合计（显示在对应的列下）。展开一个组（通过其按钮，或右方向键和左方向键）会加载其子组或行，每次一页，还有更多时显示“加载更多”行；收起会将它们移除。最上层按组分页，因此分页器统计的是组数。页脚保留所有匹配行的合计。组内的行保留其他所有功能：编辑、行 action、菜单、选择和明细。

数据层负责响应 `DataQuery::group`：设置它之后，数据层返回 `Page::groups`（每个组的值、计数以及查询所要求的合计），按该字段上排序键的方向以值排序，像行一样分页，`total` 统计的是组数。`DataQuery::apply` 为内存中的数据完成这项工作；SQL 使用 `GROUP BY`。`group` 由网格设置，绝不从客户端读取。客户端只发送它所展开的组值路径；服务器检查其长度，并把每个值转换成针对该组所在列的过滤条件，放在用户的过滤条件之前（空组使用 `blank`）。

## 树形数据

`tree(Model::parent)` 通过一个可选的父主键以树形显示行：没有父级的行位于顶层，一行的子行在其展开时加载，每次一页，最多 32 层。

```rust
DataGrid::of(search_employees)
    .tree(Employee::manager_id)
    .column(Employee::name)
    .column(Employee::salary).sum()
```

第一列带有缩进和按钮。为了知道哪些行有子行，网格会对每一页再向其 action 请求一次：父级是该页某个主键的那些行，按父字段分组（对主键使用 `Filter::any_of`，以及 `DataQuery::group`）。过滤和排序在每一层都适用；页脚合计覆盖所有匹配的行。树形网格不分组，也不显示行明细。行内编辑之后，该行在树中保持原位。

虚拟网格既不分组也不显示树，因为那里的每一行都是同一个固定高度。

## 单元格：导航、复制和粘贴

向右进入一行（或点击一个单元格）会在其中放置一个单元格光标，就像在电子表格中一样。方向键移动光标，Shift 加方向键（或 Shift+点击）扩展范围，Home 和 End 跳到行的两端，Ctrl+Home 和 Ctrl+End 跳到网格的第一个和最后一个单元格。Tab 和 Shift+Tab 沿行移动并进入下一行，到达两端时离开网格。在第一个单元格上按左方向键会回到该行。Enter 或 F2 编辑可编辑的单元格，Enter 按下单元格中唯一的按钮或链接，Escape 取消范围。范围内的单元格带有 `aria-selected`。

在单元格上按 Ctrl+C 会把范围复制为以制表符分隔的文本，值与用户所见一致。在行上按 Ctrl+C 仍然复制选中的行。

在单元格上按 Ctrl+V 会从光标处粘贴来自电子表格的以制表符分隔的文本；单个值会填满整个范围。只有可编辑的单元格会改变（粘贴到只读单元格时会给出提示），choice 可以按标签粘贴，是和否按用户所用的词。每个被改变的行都通过行内编辑的路径保存（`/_zeyn/grid/edit`，附带行版本），一行接一行地进行，因此每一行都像行内编辑那样在服务器上被读取、验证和检查；失败的行会在其单元格中显示错误，其余的行照常保存。超过 10 行会先询问，超过 500 行会被拒绝，Ctrl+Z 会撤销整个粘贴。网格编辑、行 action、批量 action 和菜单 action 都计入 `App::write_limit`，因此一次大型粘贴可能触及应用的写入限制。

## 列虚拟化

有 40 列或更多列的网格，页面中只保留固定的列和视野内的列（两侧各最多再多六列）。每一列都有固定宽度，每一侧被省略的列由一个占位单元格代替，因此宽度、固定和滚动保持原样。行保留其完整的单元格列表，所以复制、粘贴和编辑能看到每一列，并且单元格带有 `aria-colindex`。只有当窗口之外的列进入视野时，窗口才会移动。服务器仍然发送一行的每一个单元格。这样的宽虚拟网格（`VIRTUAL_WIDE_COLUMNS`，选择列、明细列和行 action 列也计入）会把整个第一个块放进 `<template data-zeyn-rows-later>`，而不是 64 个实际存在的行，因此浏览器从不会对包含所有列的行做布局：在展示库的宽网格上，最长的启动任务从 259 ms 降到 112 ms，DOMContentLoaded 从 402 ms 降到 228 ms（8 次交替加载的中位数，release 服务器，机器繁忙）。

在展示库的宽网格上测量（62 列、100,000 行、虚拟滚动、debug 服务器、Chromium、1400 x 900 窗口、144 Hz 显示器），对比启用和未启用列虚拟化的情况：

| | 页面中的单元格 | 横向，每帧 60 px（p50 / p95） | 向下，每帧 60 px（p50 / p95） | 向下，每帧 2,000 px（p50 / p95） |
|---|---|---|---|---|
| 未启用 | 2,976 | 6.9 / 7.5 ms | 42.7 / 64.8 ms | 78.5 / 141 ms，178 个长任务 |
| 启用 | 1,056 | 7.0 / 28.8 ms | 24.6 / 41.7 ms | 28.9 / 48.9 ms，7 个长任务 |

向下滚动大约快了一倍，页面中的单元格只有原来的三分之一。横向滚动本来就很流畅；窗口移动的那些帧开销最高达 29 ms。其余的开销在于解析包含所有列的 200 行数据块，列虚拟化并不改变这一点。

## 导出

`exportable()`（以及可选的 `export_permission(...)`、`export_limit(n)`）让工具栏的 `Export::Csv` 按钮下载每一个匹配的行。服务器会逐页遍历网格的 action，并立即流式输出每一页，按用户的布局和语言。公式会被中和。默认最多 100,000 行。

启用 `xlsx` Cargo feature 后，工具栏的 `Export::Xlsx` 按钮会把同样的行下载为 Excel 工作簿（`GET /_zeyn/grid/export?format=xlsx`）。单元格保留其类型：数字、带货币格式的数字形式的金额、按用户时区写成真正 Excel 日期的日期和日期时间，以及布尔值。Choice 写成其标签，被掩码的字段写成其掩码文本，文本始终是文本，因此不会有任何内容变成公式。表头行包含用户语言的标签，加粗并冻结，列保持用户的顺序和宽度。

工作簿使用 `rust_xlsxwriter` 以恒定内存模式写出：每一页到达时就写入临时文件（内存中最多等待两页），最后把压缩后的工作簿流式输出到响应。导出六列、100,000 行时，写入行期间堆内存保持在约 134 KiB（写入 10,000 行和 100,000 行之后都是如此），压缩期间为 460 KiB；文件大小为 3.4 MB。

启用 `pdf` Cargo feature 后，`Export::Pdf` 会把同样的行打印成列表报告（最多 2,000 行）；参见 [PDF 文档](operations.md#pdf-documents)。

## 选择

`selectable()`（`bulk_action` 隐含了它）会添加一个复选框列。Shift 选择一个范围，空格键选择获得焦点的行，表头的复选框选择当前页。选择栏提供选择每一个匹配的行。

## 无障碍

表格具有 `role="grid"`、`aria-rowcount`（虚拟网格）、`aria-colcount`、`aria-colindex` 和 `aria-sort`。行使用漫游 tab 停靠点（roving tab stop）。活动行内的按钮可以用 Tab 到达。分组网格和树形网格是 `role="treegrid"`，行上带有 `aria-level` 和 `aria-expanded`。菜单使用 `role="menu"`，包含 `menuitem`、`menuitemcheckbox` 和 `menuitemradio`，焦点会回到原来的位置。空状态和错误状态会被播报（`role="status"`）。

## 性能

在展示库的归档数据上测量（1,000,000 行、虚拟滚动、debug 服务器、Chromium、144 Hz 显示器），使用 Performance API：

| 滚动 | 帧 p50 | 帧 p95 | 最大值 | 长任务 |
|---|---|---|---|---|
| 滚轮速度（每帧 60 px） | 13.9 ms | 20.9 ms | 34.6 ms | 0 |
| 快速（每帧 2,000 px） | 14.1 ms | 27.8 ms | 54.9 ms | 0 |
| 扫过整个列表 | 13.9 ms | 27.8 ms | 48.6 ms | 0 |

DOM 中有 62 行（约 660 个元素），JS 堆内存保持在 4 MB 左右。从同一份归档数据导出 100,000 行，在 3.4 秒内流式输出了 7 MB。

### 保持屏幕构建函数开销低

请求是无状态的，所以每个网格请求都会重新构建承载该网格的屏幕：虚拟网格获取的每个块（每滚动 200 行一次）、每次翻页、排序、过滤和导出、每次行内编辑和每个行 action，都会在运行网格的 action 之前再次运行 `#[screen]` 函数。服务器需要这个屏幕来按节点路径找到网格并检查访问权限，这就是不为每个用户保存状态所付出的代价。

只排布组件的构建函数只花费几微秒；而查询数据库的构建函数，每个这样的请求都要付出一次查询的代价。因此：

- 在网格的 action（`DataGrid::of(search)`）中加载行。它本来就每个请求运行一次，并且会收到带有页码、排序和过滤条件的 `DataQuery`。
- 选项和查找应放在字段类型（`#[choice]`）、表单查找（`Form::lookup`，用户输入时调用它的 action）或把它们保存在有上限缓存中的服务里，而不是放在构建函数运行的查询里。
- 当屏幕必须加载一条记录（`Form::of(record)`）时，不要把大型或虚拟网格放在它上面：把它们放到各自的屏幕、明细屏幕（`DataGrid::detail`）或窗口中，这样它们的块请求重建的就是一个不加载任何数据的屏幕。
- 屏幕参数是构建函数所需的输入；只保留键，并让网格的 action 据此过滤（`DataGrid::scope`）。
