# Runtime Protocol（运行时协议）

状态：草案，由 `zeyn::server` 和 `assets/zeyn/*.js`（以 `/_zeyn/zeyn.js` 提供）实现。

本文是 Zeyn 服务端与浏览器运行时之间的约定。界面（Screen）在服务端渲染。浏览器运行时是一个很小的脚本，它把用户操作发送给服务端，并应用服务端返回的结果。应用代码从不编写 JavaScript。

## 1. Page load（页面加载）

`GET {route}?params={json}` 返回完整的 HTML 文档。`params` 是 Screen 参数经 URL 编码的 JSON；Screen 没有参数时省略。

带路径段的路由，例如 `/orders/{id}`，从路径（`/orders/42`）读取对应的 params 字段，其余字段从 `params` 读取。无法解析的路径段值返回 404。值经过百分号编码，包括点号（`%2E`），因此 Screen 的 URL 在运行时看来永远不像文件。返回 `Screen::moved_to` 的 Screen 会得到 `301 Moved Permanently`，新 URL 位于 `Location` 中；返回 `Screen::redirect` 的 Screen 会得到带有 `Cache-Control: no-store` 的 `303 See Other`。`nav.js` 会无需整页加载地跟随这两种响应，并显示目标地址，即使该加载没有新增历史记录条目（后退和前进）。

失败的页面以对应状态码（404、403、500 等）返回，内容是应用的错误 Screen（`App::not_found`、`App::server_error`）或内置的简短提示，并置于外壳（shell）之内。带有 `x-zeyn-partial: 1` 的请求只会得到该 Screen，响应带有 `x-zeyn-partial: 1` 和 `x-zeyn-title`，运行时据此无需整页加载即可换入。错误详情绝不会到达浏览器。

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

菜单只列出当前用户可以打开的 Screen。当前菜单项带有 `aria-current="page"`。

head 中每个框架脚本和样式表都带有 `?v=` 和构建 id（`meta[name=zeyn-build]`，来自 `ZEYN_BUILD_ID` 或可执行文件的哈希）；`theme.css` 带有主题文本的哈希。带当前值的请求会得到 `Cache-Control: public, max-age=31536000, immutable`，因此热页面加载不会再请求它们；其他任何请求（部署后的旧标签页、没有 `v`）得到 `no-cache` 和 ETag。运行时稍后导入的模块，会从导入它们的脚本的 URL 中沿用同一个 `v`。

### 1.1 Browser modules

页面只加载并运行其屏幕用到的浏览器代码。每个页面都运行核心：`zeyn.js`（请求、效果、窗口、表单、菜单、快捷键、事件委托和模块加载器）、`nav.js` 和 `enterprise.js`（`fetch` 上的响应钩子和会话空闲超时），合计未压缩 90 KB，gzip 后 29 KB。其余都是组件的模块：`/_zeyn/{name}.js`，部分还有 `/_zeyn/{name}.css`，在 `crates/zeyn/src/server/modules.rs` 的 `MODULES` 中声明，并带有其 HTML 中的标记：属性名（`data-zeyn-date`）和类名（`z-select`），按完整名称匹配。

| 模块 | 组件 |
|---|---|
| `components.js` | 每个控件模块都会导入的共享部分：辅助函数、弹出层和列表框核心、下拉选择（应用外壳中的选择器）、确认对话框、提示和面包屑 |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js` | 面板、标签页和分割面板；日期和时间选择器（也用于 grid 的日期筛选）；列表框、查找和多选；数字、自动增高的文本区、掩码和密码；行编辑器；分步表单；树；工具提示和弹出框；菜单按钮和分割按钮；忙碌遮罩 |
| `grid.js` | grid，以 `<link rel="modulepreload">` 链接，因为 `zeyn.js` 会自己导入它 |
| `palette.js`, `pro.js` | 命令面板和命令按钮；菜单栏、工具箱、状态栏、停靠面板和属性检查器 |
| `forms.js`, `reveal.js`, `import.js` | 表单草稿和流程转换；显示被遮蔽的值；导入 |
| `catalog.js`, `media.js`, `files.js`, `charts.js` 及其样式表 | 卡片网格、横向列表和搜索框；播放器、嵌入内容和图片；上传；图表、迷你图、KPI 卡片和子弹图 |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | 分享按钮、筛选标签、反应、hero 幻灯片、偏好存储、卡片预览（首次悬停）、助手（聊天面板，或首次按下启动按钮） |
| `topics.js`, `vlist.js` | 监听主题的节点以及驱动它们的控件（`data-zeyn-listens`、`data-zeyn-drives`）；虚拟列表（`data-zeyn-vlist`） |
| `inbox.js`, `kanban.js`, `calendar.js` and their stylesheets, `diff.css`, `code.css`, `barcode.css` | 收件箱和通知铃铛；看板；日历；不需要脚本的差异视图、代码视图和条码（代码视图的复制按钮在 `share.js` 中） |
| `range.js`、`quantity.js`、`gallery.js`、`countdown.js` 和 `commerce.css` | 范围滑块、数量步进器、商品图库和倒计时；所有商店组件的样式表（价格、规格选择器、分面面板、评价汇总） |
| `thread.js` 和 `thread.css`, `stream.js`, `composer.js` 和 `composer.css`, `typing.js` 和 `typing.css` | 消息线程；线程和助手共用的流式 Markdown 和 JSON 行；聊天 composer；正在输入指示器 |
| `stories.css` | 故事环及其行，无需脚本 |

完整页面在 head 中链接所有页面都用的样式表（`zeyn.css`、`components.css`、`content.css`、`pro.css`、`enterprise.css`）、其 HTML 中出现标记的模块的样式表，以及这些模块的脚本（放在它们导入的模块之后）。`zeyn.js` 带着同一张表（`[name, script selector, style selector]`）提供，并在之后到来的内容中加载缺少的部分：`nav.js` 在替换屏幕前发出带 `{ root, waits }` 的 `zeyn:need` 并等待新的样式表，窗口在打开前同样等待，`zeyn:content`、grid 页和 grid 筛选会加载其内容所需的脚本。浏览器对每个 URL 只保留一个实例，因此模块只运行一次。模块运行时设置它在文档中找到的内容，之后监听 `zeyn:content`；它从不扫描或监听页面上不存在的组件。模块用自身 URL 的 `?v=` 导入共享部分。当页面链接了它不使用的模块，或某个模块的样式表为其标记之外的类设置样式时，`tests/ui_modules.rs` 会失败。

在 showcase 上测得（未压缩，网络上 gzip 后）：只有文字的页面加载 132 KB 脚本（46 KB），原来是 269 KB（80 KB）；grid 页面 223 KB（74 KB），原来是 357 KB（106 KB）；表单页面 186 KB（65 KB），原来是 269 KB（80 KB）；每个页面的样式为 151 KB（35 KB），原来是 177 KB（44 KB）。包含所有组件的页面（例如组件目录）加载的文件比以前多（68 个请求而不是 41 个，未压缩 836 KB 而不是 774 KB），因为每个模块都是单独的文件。

## 2. Screen markup（Screen 标记）

每个 Screen 根元素都带有其名称和参数：

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

Window 使用 `class="z-window" role="dialog" aria-modal="true"` 和 `--z-width` 样式。

每个主体项都包裹在一个节点元素中。其路径是下面各端点所使用的地址。容器（`Row`、`Stack`、`Panel`、`Card`、`Tabs`、`Split`）以同样方式包裹每个子项，子项的路径追加其索引：主体项 1 的第一个子项的第三个子项为 `"1.0.2"`。

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

端点的 `node` 参数可以是点分隔的字符串，对于主体项也可以是纯数字（`0`）。路径最多 16 层数字；其他情况一律以 `422` 拒绝。

## 3. Attributes the runtime reads（运行时读取的属性）

| Attribute | On | Meaning |
|---|---|---|
| `data-zeyn-effect` | button, `tr` | 点击时执行的 JSON effect（行：双击或按 Enter 时） |
| `aria-keyshortcuts` | button | 键盘快捷键，例如 `Control+N` |
| `data-zeyn-export` | button | 同一 Screen 上 grid 的导出格式（`csv`、`xlsx`） |
| `data-zeyn-grid` | grid root | 标记一个 grid；同时带有 `data-zeyn-mode`、`data-zeyn-paging`、`data-zeyn-page-size`、`data-zeyn-total`、`data-zeyn-next`、`data-zeyn-offset` |
| `data-field`, `aria-sort`, `aria-colindex` | `th` | 列的字段名、当前排序、位置 |
| `data-filter`, `data-options` | `th` | 筛选输入类型（`text`、`number`、`money`、`date`、`datetime`、`time`、`choice`、`bool`）以及 choice 选项 `[{value,label}]` |
| `data-pin` | `th` | 固定列的 `start` 或 `end`（选择列和行操作列同样适用） |
| `data-edit`, `data-optional` | `th` | 可编辑列的输入类型，面向有保存权限的用户 |
| `data-zeyn-texts`, `data-zeyn-columns`, `data-zeyn-view` | grid root | 本地化的控件文本、全部列 `[{field,label,hidden,pin,width}]`、已保存的视图 |
| `data-zeyn-views` | grid root | 视图保存在服务端时为 `server` |
| `data-zeyn-version-field`, `data-zeyn-exportable` | grid root | 行内编辑所用的行版本字段名；grid 可导出时为 `csv` |
| `data-zeyn-key`, `data-zeyn-version`, `data-zeyn-menu` | `tr` | 行键、行版本、上下文菜单项 |
| `data-v` | `td` | 可编辑单元格的输入值 |
| `data-zeyn-detail`, `aria-expanded` | `tr` | 该行的详情 Screen，形式为 `{screen, params}`；是否已展开 |
| `data-zeyn-expand` | button in a row | 打开或关闭该行的详情、分组或子项 |
| `data-zeyn-level`, `aria-level`, `data-zeyn-path` | `tr` in a `treegrid` | 分组 grid 或树形 grid 中的深度（顶层为 0）；该行展开时要发送的分组值或祖先键的路径 |
| `z-group` | `tr` | 分组标题行；`z-branch-more` 是分支的“加载更多”行 |
| `data-zeyn-run` | button in a row | 要执行的行项目（`a0` 是第一个行操作） |
| `data-agg` | `tfoot td` | 该列显示的合计（`sum avg min max count`） |
| `data-zeyn-bulk` | div before the grid | 带有 `data-zeyn-bulk-run` 按钮的选择栏 |
| `data-zeyn-menu` | screen root | Screen 的上下文菜单 |
| `data-zeyn-native-menu="off"` | `html` | 除文本输入框外，关闭浏览器自带的上下文菜单 |
| `data-zeyn-rows` | `tbody` | 行的放置位置 |
| `data-zeyn-page` | pager button | `prev` 或 `next` |
| `data-zeyn-page-info` | span | 形如 `1-50 of 230` 的文本，由 grid 根元素上的本地化模板生成 |
| `data-zeyn-text-range`, `data-zeyn-text-range-total`, `data-zeyn-text-loaded` | grid root | 本地化模板：`{from}-{to}`、`{from}-{to} of {total}`、`{count} rows loaded` |
| `data-zeyn-more` | div | 无限滚动哨兵，包含“加载更多”按钮 |
| `data-zeyn-submit` | form | 提交 action 的名称 |
| `data-zeyn-editor` | input | 自定义字段类型的客户端编辑器 |

Effects：

| Effect | JSON | Runtime does |
|---|---|---|
| Open window | `{"effect":"open_window","screen":"product_form","params":{...}}` | 将该 Screen（第 4.1 节）加载到模态 `<dialog>` 中 |
| Navigate | `{"effect":"navigate","screen":"...","params":{...}}` | 加载 `/_zeyn/route/{screen}?params=`，它会重定向到该页面；其路由带路径段时使用路径形式 |
| Toast | `{"effect":"toast","message":"Saved"}` | 显示 4 秒的 toast |
| Close and refresh | `{"effect":"close_and_refresh"}` | 关闭最上层的 dialog，并重新加载其下方 Screen 上的每个 grid |

## 4. Endpoints（端点）

所有 `POST` 请求都发送 JSON，必须携带请求头 `x-zeyn-csrf`，其值取自 `zeyn-csrf` meta 标签，否则以 `403` 拒绝。

未登录状态下打开的公开 Screen，其 `zeyn-csrf` meta 标签为空，也不带 `Set-Cookie`，以便可以被缓存。在第一次 `POST` 之前，运行时会调用 `GET /_zeyn/csrf`，它返回 `{ "token": "..." }` 并带有 `Cache-Control: no-store`，在浏览器还没有 cookie 时设置 `zeyn_csrf` cookie，并拒绝 `Sec-Fetch-Site` 不是 `same-origin` 的请求。令牌被放入 meta 标签；调用失败时，`POST` 不带令牌发出，并像其他请求一样被拒绝。不会有任何静默重试。

应用内导航（`nav.js`）以 `x-zeyn-partial: 1` 请求 Screen，只得到其 HTML，页面标题位于 `x-zeyn-title`。当 Screen 设置了 head 标签（description、canonical URL、预览图、robots、JSON-LD）时，正文以 `<template data-zeyn-head>` 开头，其中包含这些标签；`nav.js` 会移除文档 head 中所有 `[data-zeyn-head]` 元素，把该 template 的标签移到那里，然后丢弃该 template。没有 head 标签的 Screen 不发送 template，因此旧标签只会被移除。

### 4.1 Screen fragment（Screen 片段）

`GET /_zeyn/screen/{screen}?params={json}` 只返回 Screen 的 HTML，不含页面外壳。用于 Window 和行详情：打开带有 `data-zeyn-detail` 的行时，会把它的 Screen 加载到紧随其后的新 `tr.z-detail` 中（一个横跨整行的单元格，内含 `role="region"`），关闭时该行被移除。该 Screen 的权限与任何 Screen 一样被检查；详情中的 grid 使用详情 Screen 自己的名称和参数与服务端通信。

### 4.2 Grid rows（Grid 行）

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

筛选条件可以设置 `contains`、`starts_with`、`equals`、`from`、`to`、`any_of` 和 `is`（参见 `docs/grid.md`）。`view.columns` 是渲染行时使用的列布局；`head: true` 时，响应还会包含 `head`，即表头行。

无限滚动时，`page` 为 `{ "type": "after", "cursor": "...", "limit": 100 }`，第一批的 `cursor` 为 `null`。

响应 `200`：

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

请求了合计时，`foot` 是合计行，否则为 `null`。

`total` 和 `next` 可以为 `null`。运行时对分页 grid 替换行，对无限滚动则追加行。

分组 grid 和树形 grid（`<table role="treegrid">`）通过 `POST /_zeyn/grid` 发送 `path` 来加载一个分支：即正在展开的那一行的分组值（或祖先键），取自它的 `data-zeyn-path`。响应结构相同；`rows` 包含分组行或更深一层的行，`total` 统计它们的数量。运行时把它们放在被展开的行之后，并在该行关闭时移除其后所有更深层的行。路径长度超过 grid 的分组层数（树形则超过 32）时返回 `404`。

### 4.2.1 More grid endpoints（更多 grid 端点）

| Endpoint | Body | Does |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | 保存用户的视图（`null` 表示删除）。`204`；没有视图存储时为 `404` |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | 执行行项目（`a0` 为行操作，`m2.1` 为菜单路径）。服务端按键加载该行并构造输入。`{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | 对选中的键执行批量操作；`all` 时对匹配 `query.filters` 的每一行执行。`{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | 保存行内编辑；`values` 包含被编辑的列和行版本，格式与表单发送的一致。`{row}`；不可编辑的列为 `403`，另有 `409`、`422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | 以 CSV 流式输出所有匹配的行 |
| `POST /_zeyn/menu` | `{screen, params, item}` | 执行 Screen 上下文菜单中的某个 action 项 |

### 4.2.2 Virtual scrolling（虚拟滚动）

带有 `data-zeyn-paging="virtual"` 的 grid 在所有行之上显示一个可滚动的列表，无论共有多少行。服务端渲染第一个块（`data-zeyn-page-size` 行，偏移 0），并带有 `data-zeyn-total`：前 64 行在表格中，其余的在它之后的 `<template data-zeyn-rows-later>` 中；如果 grid 显示 40 列或更多列，则所有行都在 template 中。grid 根元素还带有 `data-zeyn-row-height`（像素）和 CSS 变量 `--z-row-height`。

运行时的行为：

- 把表格主体放进一个滚动容器，其内部占位元素高度为 `total * rowHeight` 像素，上限为 8,000,000 px。超过上限时，滚动位置按比例映射到行索引（`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`），因此 1,000,000 行仍然可以滚到末尾。
- DOM 中只保留可见行以及上下各 20 行，并用 `transform: translateY(...)` 定位。行高固定；过长的文本以省略号截断。
- 以 `POST /_zeyn/grid` 加载每块 `data-zeyn-page-size` 行的数据块，使用 `page: { "type": "offset", "offset": blockStart, "limit": blockSize }`，首次调用之后 `with_total: false`。缓存最多保留 20 个块且最多 100,000 个单元格（最近最少使用者先淘汰；63 列的 grid 为 7 个块，但不少于 3 个），为 `reload` 保留的块计入同一上限并最先被丢弃，沿滚动方向预取下一个块，并取消已离开视野的块的请求（`AbortController`）。
- 块加载期间从不清空可见行。缺失的行显示带有 `aria-busy="true"` 的占位行，直到其块到达，因此界面不会闪烁。
- 排序和筛选变化会丢弃缓存并滚动到顶部。
- 键盘：方向键、Page Up、Page Down、Home 和 End 移动活动行并将其滚动到可见位置。表格带有 `aria-rowcount` = total，每一行带有 `aria-rowindex`。
- 位置文本（`data-zeyn-page-info`）使用 `data-zeyn-text-range-total`。

卡片网格（`zeyn::ui::CardGrid`，不包括 rail 和行列表）在包含 200 张卡片后，会对页面本身的滚动而不是滚动容器进行窗口化：`catalog.js` 给列表加上 `data-zeyn-window`，在其中保留距视口 800 px 以内的行，把上方和下方的行变成内边距，让每一行的高度等于前几张卡片中最高的那张（`grid-auto-rows`），并在每张卡片上设置 `aria-posinset` 和 `aria-setsize`（在可能还有更多页面时为 `-1`）。窗口之外获得焦点的卡片会带着 `z-cards-kept` 留在 DOM 中，并放在它自己的单元格上方。来自“显示更多”或其他页面的卡片会加入同一个窗口。当页面滚动速度超过每 ms 3 px 时，卡片进入时会暂缓加载图片（`data-zeyn-park`，`src` 和 `srcset` 先另存起来），在 120 ms 内没有滚动后再取回图片。

### 4.3 Form submit（表单提交）

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

值按用户输入的原样发送：输入框和下拉框为字符串，复选框为布尔值。服务端按字段类型解析它们。

服务端以 Screen 加载的 model 为起点，只覆盖位于表单上且非只读的字段。客户端发来的其他值一律忽略。键绝不来自客户端。

响应：

- `200 { "effects": [ ... ] }`：已保存；按顺序执行这些 effects。
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`：在各字段下方显示对应消息（`#{scope}-{field}-error`），在输入框上设置 `aria-invalid="true"`，并聚焦第一个无效的输入框。`field` 为空的错误显示在表单顶部。

### 4.4 Errors（错误）

任何端点都可能返回：

| Status | Body | Meaning |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | 缺少权限、guard 拒绝或 CSRF 令牌无效 |
| `404` | `{ "error": "not_found", "message": "..." }` | 未知的 Screen 或节点 |
| `409` | `{ "error": "conflict", "message": "..." }` | 他人修改了该记录 |
| `422` | 见 4.3 | 校验 |
| `504` | `{ "error": "timeout", "message": "..." }` | 调用耗时超过服务端限制 |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | 详情只在服务端日志中 |

运行时将 `message` 作为错误 toast 显示。

### 4.5 Language（语言）

`POST /_zeyn/locale`，带 `{ "locale": "tr" }`，把选择存入 `zeyn_locale` cookie 并返回 `204`。运行时随后重新加载页面。没有 cookie 时，服务端先根据 `Accept-Language` 选择，再使用应用默认值。`<html lang>` 始终与所选 locale 一致。

应用有多个 locale 时，外壳会渲染一个语言选择器（`select[data-zeyn-locale]`）。

### 4.6 Theme（主题）

除非应用隐藏它或只提供单一主题，外壳会渲染一个主题选择器（`select[data-zeyn-theme-picker]`）。每个选项带有 `data-scheme`（`light`、`dark`，System 为空），应用自定义主题带有 `data-custom`。变更时，运行时立即在 `<html>` 上设置 `data-theme` 和 `data-zeyn-theme`，并设置 `color-scheme` meta，然后发送 `POST /_zeyn/theme`，带 `{ "theme": "dark" }`。服务端返回带 `zeyn_theme` cookie 的 `204`，对应用未提供的主题则返回 `404`。页面使用该 cookie 中的主题渲染，因此没有重新加载，也没有闪烁。参见 `docs/theming.md`。

### 4.7 Enterprise endpoints（企业端点）

`enterprise.js` 和 `enterprise.css` 与运行时一同加载。详情见 `docs/enterprise.md`。

| Endpoint | Purpose |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | 被掩码字段的原始值，`{value}`；需要取消掩码权限，始终记入审计 |
| `POST /_zeyn/reason {screen, reason}` | 标记为 `reason` 的 Screen 所需的理由；返回带 `HttpOnly` cookie 的 `204` |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV 预览（`commit: false`）或导入 |
| `POST /_zeyn/keepalive` | 空闲警告之后的“保持登录”；`204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | 存活与就绪探测；health 不受限流，readiness 仅限 `probe_ips` |
| `GET /_zeyn/jobs` | 对拥有 jobs 权限的用户返回任务状态 |

每个响应都带有 `x-zeyn-build`。运行时会处理的拒绝响应：`428 reason_required`（询问理由并重复请求）和 `401 reauth_required`（要求用户重新登录并重复请求）。

## 5. Security（安全）

- 每个响应都设置 `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`、`X-Content-Type-Options: nosniff` 和 `Referrer-Policy: same-origin`。`App::csp` 会把指定名称的 `https` 主机加入 `img-src`、`media-src`（以及 `blob:`）、`frame-src`、`connect-src` 和 `font-src`；其他指令绝不改变。该响应头文本在启动时一次性构建。
- 没有内联脚本，也没有内联事件处理器。内联 `style` 属性只用于 CSS 自定义属性（`--z-width`、`--z-flex`）和固定的列宽，因此策略中加入了 `style-src-attr 'unsafe-inline'`。脚本仍保持严格限制。
- CSRF 令牌存放在 `HttpOnly`、`SameSite=Strict` 的 cookie 中，并以常量时间与 `x-zeyn-csrf` 请求头比较。
- 页面、`totp` 表单和 `GET /_zeyn/csrf` 从不原样携带 cookie 中的令牌：每个响应都带有一个新的随机填充（pad），后面跟着令牌与该填充的异或结果（128 个十六进制数字），因此 gzip 无法泄露它（BREACH）。服务器在比较前先还原，并仍接受原来的 64 位明文令牌。
- 1 KB 或更大的动态响应（页面、局部 Screen、grid 块、导出）在浏览器接受时以级别 2 进行 gzip 压缩，并带有 `Vary: Accept-Encoding`。从不压缩的有：已经带有 `Content-Encoding` 的响应（框架资源，压缩一次后缓存）、范围请求（`Content-Range` 以及任何带 `Range` 的请求）、视频、音频、SVG 以外的图片、`application/octet-stream`、压缩包和 PDF、`text/event-stream`、`application/x-ndjson`（助手的流）以及 WebSocket 升级。
- 每次调用依次检查：身份认证、Screen 的访问权限、Feature 的 guard、节点 action 的访问权限，然后运行拦截器（interceptor）和 action。

## 6. Runtime behavior（运行时行为）

- 没有 JavaScript 时，页面仍会渲染每个 grid 的第一页。表单和分页需要运行时。
- 键盘：方向键在 grid 行之间移动，Enter 打开一行，Escape 关闭最上层的 dialog，来自 `aria-keyshortcuts` 的快捷键在除文本输入框之外的任何位置都有效。在 grid 中：Space 选择该行，F2 编辑，Ctrl+Z 撤销上一次编辑，Shift+F10 或上下文菜单键打开该行或表头的菜单，Shift+点击按多列排序，Right 和 Left（在从右到左的页面中镜像）打开和关闭行的详情、分组或子项，在子行上按 Left 回到其父行。在某一行上按 Right 进入其单元格：方向键、Home、End、Ctrl+Home、Ctrl+End、Tab 和 Shift+Tab 移动单元格光标，Shift 扩展范围（单元格上的 `aria-selected`），Ctrl+C 把该范围复制为以制表符分隔的文本，Ctrl+V 粘贴到可编辑单元格，每个变更的行对应一次 `POST /_zeyn/grid/edit`。有 40 列或更多列的 grid 只在页面中保留视野内的列（带固定宽度的 `colgroup`，带 `colspan` 的 `td.z-col-gap` 占位单元格，单元格上的 `aria-colindex`）。
- 在定义了 Zeyn 菜单的地方，右键会打开该菜单（表头、带 `data-zeyn-menu` 的行、带 `data-zeyn-menu` 的 Screen）；其他任何地方保留浏览器自带的菜单。
- dialog 打开时焦点移入其中，关闭时焦点回到打开它的元素。
- 点击的同一帧内即显示等待状态：屏幕加载时 `nav.js` 将目标链接标记为当前并设置 `html[data-zeyn-loading]`，窗口和表单提交也会设置它；grid 在按下排序、翻页或筛选按键后立即淡化行（`aria-busy`）（筛选会再等待 150 ms 的按键，新请求会取消旧请求）。窗口顶部的细条只在响应超过 150 ms 时出现；它是固定定位的，不会让任何内容移动，在 `prefers-reduced-motion` 下不做动画。
- 客户端检查（`required`、`minlength`、`maxlength`、`min`、`max`、`type="email"`）在提交前通过浏览器的约束验证 API 运行。服务端始终会再次校验。

## 7. Component library（组件库）

`zeyn::ui` 组件由第 1.1 节的模块和样式表 `/_zeyn/components.css` 提供。和运行时一样，它们不需要登录。

### 7.1 Endpoints（端点）

全部为带 JSON 和 CSRF 请求头的 `POST`，并且会先检查 Screen 的访问权限和 Feature 的 guard。

| Endpoint | Body | Answer |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | 一个节点的 HTML，包含其 grid 的第一页。用于懒加载标签页。 |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`：界面运行一次渲染出的多个节点，用于被刷新或被驱动主题的监听者。最多 32 个不同路径；界面中不存在的路径返回 `404`。界面的访问权限、guard 和 interceptor 只运行一次，params 按界面的类型化 params 读取。 |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | 来自该节点数据源 `key` 的 JSON：查找（lookup）选项（`key` 为字段名）、树的子节点（`key` 为空）、步骤检查（`key` 为 `_step`）。会检查数据源 action 的权限并运行拦截器，与 grid 相同。不符合 action 的输入以 `404` 拒绝。 |
| `/_zeyn/data` with `screen` = `_site.{feature}` | 同上 | 对站点头部插槽中的组件执行相同操作：`node` 为插槽（0 start、1 center、2 end、3 bottom）、节点在其中的索引，随后是子节点索引。会检查该 feature 的 guard 和 action 的权限。 |
| `/_zeyn/parse` | `{ kind, text, label }` | 服务端如何按用户的语言、时区和时钟解读键入的日期或时间：`{ ok, text, iso, utc, zone }` 或 `{ ok: false, message }`。`kind` 为 `date`、`time`、`datetime`、`local` 或 `presets`（今天和快捷范围）。 |

### 7.2 Values the forms submit（表单提交的值）

- Lookup 以文本提交 id：`"7"`，多个时为 `"3,7,9"`。
- Lines 提交 `"{field}": "rows"`，每个单元格一个值，键为 `"{field}.{line}.{column}"`，另有 `"{field}.{line}._i"`，即该行来源的已加载 line 的索引（新增 line 为空）。服务端根据已加载的 line 重建每一行，因此只有显示出来且可编辑的列会改变。
- line 的校验错误使用 `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }`。

### 7.3 Events（事件）

| Event | On | Detail |
|---|---|---|
| `zeyn:content` | `document` | 带有新内容的元素（导航、懒加载面板）。运行时加载所需的模块，每个已加载的模块初始化其中的内容。 |
| `zeyn:errors` | the form | `422` 错误，在运行时显示之后触发。line 编辑器和分步表单使用它。 |
| `zeyn:effect` | `document` | 运行时不认识的 effect：`{ effect, source, run }`。`run(next)` 执行另一个 effect。`confirm` 使用它。 |
| `zeyn:topics` | `document` | `{ topics }`：来自其他脚本（收到通知的 `live.js`）的待刷新主题。 |
| `zeyn:reload` | 某个元素 | 就地重新加载其中的网格：当前页，或在 `{ reset: true }` 时加载第一页。 |
| `zeyn:density` | `document` | 无。用户在账户菜单中选择了另一种密度（`<html>` 上的 `data-density` 已经改变）；窗口化的卡片网格会重新测量其行。 |

Effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` 会在模态警告对话框中询问，只有在用户确认后才执行 `then`。

Effect `{"effect":"refresh","topics":["sales"]}` 刷新这些主题的监听者。`[data-zeyn-listens="a,b"]` 标记一个监听者（其父级 `data-zeyn-node` 的节点）；`[data-zeyn-drives="a"]` 标记驱动主题的控件：筛选表单（`data-zeyn-kinds` 说明哪些 params 是数字或布尔值）、搜索框（其字段名为 `q`）或筛选标签（每个标签在 `data-zeyn-params` 中带有自己的 params）。驱动时，控件的值合并到界面根的 `data-zeyn-params`，若该界面是主界面则用它们替换地址，然后刷新监听者。`topics.js` 一次处理同一动画帧内点名的主题：包含网格的监听者收到 `zeyn:reload`，界面上的其他监听者放进一次 `POST /_zeyn/nodes`，同一界面较新的请求会中止旧请求并接管其节点。焦点和展开的 `<details>` 按 id、test id、`data-zeyn-key` 行和位置保留。

虚拟列表是带有 `data-zeyn-row-height`、`data-zeyn-block` 和 `data-zeyn-total` 的 `section[data-zeyn-vlist]`；`vlist.js` 用 `{ page: { type: "offset", offset }, with_total: true }` 向 `POST /_zeyn/grid` 请求数据块，并把每个 `<li>` 放在 `(aria-posinset - 1) * height` 处。依赖另一个字段的 lookup 带有 `data-depends-on="field"`，并在其 `/_zeyn/data` 输入中把该字段的值作为 `parent` 发送。

客户端偏好（`zeyn::ClientPref`）位于浏览器模块 `/_zeyn/state.js` 中，只有包含带 `data-zeyn-remember` 或 `data-zeyn-prefs-clear` 的元素的页面才会导入它。`GET /_zeyn/prefs` 返回 `{ preferences }`，表示应用的同意钩子是否允许在会话之外保留偏好值，并带有 `Cache-Control: no-store`。服务器不读取任何已保存的值。

### 7.4 Media（媒体）

`Video` 和 `Audio` 由 `/_zeyn/media.js` 和 `/_zeyn/media.css` 提供，链接在组件库之后。参见 `docs/media.md`。

播放器在第一次进入视野时，通过页面共用的一个 `IntersectionObserver` 设置其来源和海报（先 `data-zeyn-poster-low`，再 `data-zeyn-poster`）；带有 `data-zeyn-preview` 的根元素在四分之一可见时静音并循环播放，除非 `prefers-reduced-motion`、`prefers-reduced-data` 或 Save-Data 另有要求，直到访客使用它的控件为止。带加载选项的图片位于一个无需脚本即可显示占位符的框（`.z-img-host`）中，并带有 `data-zeyn-img`，这是加载 `media.js` 的标记。文档上的一个捕获阶段监听器处理这些图片的 `load` 和 `error`：加载成功时把框标记为 `z-img-done`（在低分辨率文件上淡入时还有 `z-img-fade`）；失败时尝试 `data-zeyn-fallback` 一次，然后显示框中的 `<template data-zeyn-img-error>`，或替代文本加图标。在模块运行之前就已结束的图片，会在模块启动时检查一次。

| Endpoint | Does |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | 已注册媒体源的文件，支持 range 请求：`200`、`206`、`304`、`416`。 |
| `GET /_zeyn/media/position?source=&id=` | `{ at, kept }`，即已登录用户上次停止的位置；媒体源不保存位置时 `kept` 为 `false`（此时播放器改用 local storage）。 |
| `POST /_zeyn/media/position` | 带 CSRF 请求头的 `{ source, id, at }` 用于保存位置：`204`，位置无效时为 `422`。 |

### 7.5 Islands（岛）

启用 `islands` feature 后，包含 `div[data-zeyn-island]` 的 Screen 会附带 `<meta name="zeyn-islands">`（岛名称到 `{ js, wasm }` URL 的映射）、`/_zeyn/islands.js` 以及带有 `'wasm-unsafe-eval'` 的策略；其他页面不变。包含岛的局部响应带有 `x-zeyn-islands: 1`，当文档缺少岛策略时，`nav.js` 会整页加载它。

| Attribute or endpoint | Meaning |
|---|---|
| `data-zeyn-island`, `data-zeyn-load`, `data-zeyn-props` | 岛名称、`visible`、`interaction` 或 `idle`、props JSON |
| `data-zeyn-island-fallback` | 回退内容，岛挂载后隐藏 |
| `data-zeyn-island-state` | 由加载器设置：`loading`、`ready`、`failed` |
| `GET /_zeyn/islands/{file}` | manifest 中列出的构建产物，缓存一年 |
| `POST /_zeyn/data` with `key` = remote name | 来自岛的类型化调用，包含 7.1 的所有检查 |

参见 `docs/islands.md`。

### 7.6 Realtime（实时）

启用 `realtime` feature 后，`div[data-zeyn-live="topic"]` 标记在服务端通知该 topic 时刷新的节点。包含此类节点的页面会链接 `/_zeyn/live.js`；包含此类节点的局部响应带有 `x-zeyn-live: 1`，`nav.js` 会导入该脚本。元素上的 `zeyn:reload` 会就地重新加载其内部的 grid。

| Endpoint | Does |
|---|---|
| `GET /_zeyn/live/topics?t=a,b&csrf=` | WebSocket；文本帧 `{ "topic": "a" }` |
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=` | WebSocket；双向的二进制帧 `0x01` + Y update (v1) |

握手在升级之前会检查 origin、与 cookie 比对的 CSRF 令牌、已登录用户以及权限。参见 `docs/realtime.md`。

播放器在回退到渐进式文件之前，会在其根元素上派发可取消的 `zeyn:media-adaptive` 事件（`{ media, kind, url, hls, dash }`）；流媒体库调用 `preventDefault()` 即可接管。

## 8. Desktop class shell（桌面级外壳）

`/_zeyn/pro.js` 和 `/_zeyn/pro.css` 在组件库之后加载。与运行时一样，它们无需登录。

### 8.1 Commands and the palette（命令与命令面板）

命令在 Rust 中声明（`Command::open`、`Command::navigate`、`Command::effect`、`Command::action`），并通过 `Feature::command` 注册。外壳为用户可以运行的每个命令（Feature 的 guard 通过且命令的访问权限允许）在 `div[data-zeyn-commands]` 内渲染一个隐藏按钮：

| Attribute | Meaning |
|---|---|
| `data-zeyn-command` | 命令 id |
| `data-zeyn-effect` | effect 命令的 effect，点击时由运行时执行 |
| `data-zeyn-command-run` | action 命令的 id，通过 `POST /_zeyn/command` 执行 |
| `aria-keyshortcuts` | 规范化后的快捷键（`Control+Alt+N`）；运行时的快捷键处理会点击该按钮 |
| `data-hint`, `data-icon` | 向用户显示的快捷键、图标名称 |
| `data-text-*` on the container | 本地化的命令面板文本 |

`App::check` 会拒绝：无法解析的快捷键、单字符快捷键（WCAG 2.1.4）、命令面板自身的快捷键、被两个命令使用的快捷键或 id、浏览器从不传给页面的快捷键（`BROWSER_RESERVED`：`Ctrl+N`、`Ctrl+T`、`Ctrl+W`、`Ctrl+Tab`、`Alt+F4` 及其 macOS 形式），并且除非命令调用 `.override_browser()`，还会拒绝浏览器自己使用的快捷键（`BROWSER_DEFAULTS`：打印、保存、查找、地址栏、重新加载、缩放、开发者工具、复制和粘贴，`F1` 到 `F12`）。工具栏按钮在运行时遵循相同规则：此类快捷键不会被绑定，并记录原因。可自由使用的组合是 `Alt+Shift+<letter>` 和 `Ctrl+Alt+<letter>`。

`Alt+Shift+P`（macOS 上为 Option+Shift+P）默认打开命令面板；`App::palette_shortcut("Ctrl+Shift+P")` 可以更改它，也可以有意改成浏览器使用的按键。快捷键位于 `data-palette-keys` 中，字母按物理按键匹配，因此 Option 和 AltGr 不会破坏它。命令面板也可以通过菜单中的 `button[data-zeyn-palette-open]` 打开。它是一个带有 ARIA combobox 和 listbox 的模态 `<dialog>`。它列出应用的命令、当前 Screen 的工具栏按钮、菜单栏命令和上下文菜单项（有 Window 打开时为最上层的 Window），以及菜单中的 Screen。匹配是模糊的，忽略大小写和重音符号；最近运行的八个命令排在最前（`localStorage` 键 `zeyn.palette.recent`）。Escape 将其关闭，焦点回到原来的位置。

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | 以服务端固定的输入运行一个 action 命令：CSRF、Feature 的 guard、action 的权限和拦截器。`{ effects }`；未知 id 和 effect 命令为 `404` |
| `POST /_zeyn/search` | `{ query }` | 运行用户可以运行的每个 `Feature::search`（CSRF、Feature 的 guard、action 的权限、拦截器），查询截断为 100 个字符。`{ hits: [{ label, group, url }] }`，最多 20 条，且只含应用内路径 |

容器带有 `data-search` 时，命令面板在最后一次按键后 150 ms 发送查询（两个字符或更多），并把命中项列在命令下方。只绘制最新一次查询的响应。选择某个命中项会发送 `zeyn:navigate`，并在 `url` 前加上基础路径；其中的 `#anchor` 会在 Screen 换入之后滚动到该元素。

运行时在 `document` 上监听 `zeyn:run`，以便其他脚本通过它执行工作：`{ effects, source }` 执行 effects，`{ screen, item }` 执行 Screen 的上下文菜单项，`{ error }` 显示错误 toast。

### 8.2 Menu bar, toolbox, property inspector, status bar（菜单栏、工具箱、属性检查器、状态栏）

| Markup | Behavior |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG 菜单栏。顶层项是带有 roving tab stop 的 `button[role=menuitem][aria-haspopup=menu]`；每个控制一个关闭时为 `hidden` 的 `ul[role=menu]`。各项带有 8.1 的命令属性。Left 和 Right 在菜单之间移动，Down、Enter 和 Space 打开，Up 在最后一项处打开，Home 和 End 跳转，字母键移到下一个以它开头的项，Escape 关闭并回到菜单栏，Tab 和点击外部会关闭。用户不能运行的菜单和菜单项不会被渲染。 |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | `button.z-tool[aria-pressed]` 的切换组。方向键、Home 和 End 移动 roving tab stop；点击、Enter 或 Space 开启一个工具，发送 `zeyn:tool`（`detail.tool`），并把 `data-text-selected` 写入状态栏。 |
| `section.z-inspector` | 一个带有 `Form::section` 分组（`details.z-form-section`，其主体为 `role=group`）的表单，通过检查器节点路径上的 `POST /_zeyn/form` 保存。包含失败字段的分组会在浏览器检查之前以及 `422` 之后展开。 |
| `div[data-zeyn-statusbar]` | 位于起始端和末尾端的项，以及 `[data-zeyn-status-live]`（`role=status`、`aria-live=polite`）。Screen 中任意元素发出的 `zeyn:status`（`detail.text`）会替换实时文本。 |

### 8.3 Docking panels（停靠面板）

`div.z-dock[data-zeyn-dock]` 是由四个 `section.z-dock-area[data-area]`（`left`、`center`、`bottom`、`right`）和三个 `div.z-dock-bar[role=separator][data-edge]` 分隔条组成的网格。尺寸为自定义属性 `--z-dock-left`、`--z-dock-right`（宽度的百分比）和 `--z-dock-bottom`（高度的百分比），取值 10 到 60。每个区域有一个 `role=tablist`，包含可拖动的 `button.z-dock-tab[data-panel]`，一个选项按钮，以及容纳子节点的 `role=tabpanel` 面板（路径与任何容器相同；面板移动时路径不变）。没有打开面板的侧边区域会被隐藏。

键盘：Left 和 Right（在从右到左的页面中镜像）、Home 和 End 在区域内已打开的标签页之间移动；Delete 关闭获得焦点的面板；Shift+F10 或上下文菜单键打开该区域的选项（移动到另一个区域、关闭、显示已关闭的面板）；分隔条上的方向键以 5% 调整大小，Home 和 End 跳到极限值。用 `MenuList::panels` 创建的菜单栏条目把每个面板列为 `menuitemcheckbox`。变化会在 `[data-zeyn-dock-live]` 中播报。

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`：已登录用户保存的布局，或 `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`；`layout: null` 表示删除 |

`layout` 为 `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }`，按标签页顺序排列。服务端检查 CSRF、Screen 的访问权限和 Feature 的 guard，只保留该 dock 自己的面板（每个一次）并限制尺寸范围，然后将其存入应用的视图存储（`App::datagrid_views`），键为租户、Screen 和节点。没有存储时端点返回 `404`，没有用户时返回 `403`；此时浏览器把布局保存在 `localStorage` 中。

## 9. AI agents: MCP and WebMCP（AI 代理：MCP 与 WebMCP）

外部 AI 代理通过两个入口访问应用，两者默认关闭，用 `App::mcp` 开启：

| Setting | Endpoint MCP | WebMCP tools | Writes |
|---|---|---|---|
| `Mcp::off()` (default) | `404` | `404` | no |
| `Mcp::read_only()` | on | on | no |
| `Mcp::on()` | on | on | 表单保存和执行写入的 API action |

`.server(false)` 和 `.webmcp(false)` 关闭其中一个入口。由 `zeyn new` 创建的新项目以 `Mcp::read_only()` 起步。

为什么默认关闭：该端点允许任何持有有效 `Authorization` 请求头的人通过模型驱动应用，而模型可能被它所读取的数据所操纵。框架让每个应用都为代理做好准备（工具由代码生成，无需编写任何内容），但应用是否打开这个入口，以及代理是否可以写入，应由运行它的人决定。只读模式只需一行即可启用。

### 9.1 Tools（工具）

工具是为已登录用户根据应用构建的：

- 每个 grid 一个搜索工具（`{screen}_{node}_search`，输入：一个 `DataQuery`），启用写入时，每个表单一个保存工具（`{screen}_{node}_save`，输入：按键入原样的表单值），范围是每个无需 params 即可打开的路由 Screen；
- 每个已发布的 API action 一个工具（`api_{feature}_{action}`）。输入为 `DataQuery` 的 action 属于读取；其他所有 API action 都算作写入。

描述由 Screen 标题、Screen 和 action 的 `///` 文档注释的第一段，以及它们的 `ai = "..."` 指引构建。用户无权使用的 grid 和表单，以及标记了 `ai = false` 的 Screen 和 action，绝不会被列出，被调用时也会被拒绝。

`#[action(confirm)]` 标记一个只有在用户于应用内确认之后，模型才可以运行的 action。`/_zeyn/mcp` 既不列出也不运行此类工具（该通道上没有人能够确认）。WebMCP 工具会在模态对话框中询问（`zeyn:confirm`），然后随表单发送 `confirmed: true`；内置的助手则在其聊天中询问（参见 `docs/assistant.md`）。

### 9.2 Endpoints（端点）

| Endpoint | Does |
|---|---|
| `GET /_zeyn/tools?screen&params` | 当前所见 Screen 的 `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }`。`zeyn.js` 用 `document.modelContext` 注册它们 |
| `POST /_zeyn/mcp` | JSON-RPC：`initialize`、`tools/list`、`tools/call`。需要 `Authorization` 请求头和已登录用户 |
| `POST /_zeyn/form` with `via_agent: true` | WebMCP 表单保存；需要启用 WebMCP 的 `Mcp::on()`。只有在用户确认之后才可使用 `confirmed: true` |

每次代理调用都以该用户的身份运行：Screen 的访问权限、guard、action 的权限、拦截器、限流和时间限制。代理调用始终会被审计（`via_agent: true`），被掩码的字段对它们始终保持遮蔽，即使用户本人可以取消掩码。OpenAPI 用 `x-zeyn-ai` 标记每个操作：`ai = false` 时为 `false`，否则为 `{ hint, confirm, writes }`。
