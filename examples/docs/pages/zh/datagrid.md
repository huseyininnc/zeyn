# DataGrid

`DataGrid` 显示来自 action 的行，该 action 接受 `DataQuery` 并返回 `Page<E>`。分页、排序、筛选和合计都在服务器上运行，因此百万行的表格对浏览器的开销与百行的表格相同。

## 一个表格

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

点击表头排序，在筛选框中输入内容进行筛选，拖动表头可移动列，拖动其边缘可调整列宽。合计涵盖所有经过筛选的行，而不仅是当前页。

## action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply` 在内存中对 `Vec` 进行筛选、排序、合计和分页，适用于小型列表和测试。以数据库为后端的存储会读取同一个 `DataQuery`（页、排序、筛选、合计），并用 SQL 作答；参见 [数据与数据库](database.md)。

## 分页

| 分页 | 用途 |
|---|---|
| `Paging::pages(n)` | 带页码的分页，除非另行设置，每页 50 行 |
| `Paging::infinite(n)` | 滚动时加载更多行 |
| `Paging::virtual_scroll()` | 超大列表；页中只包含可见的行 |
| `Paging::none()` | 一页即可容纳的短列表 |

无论客户端请求多少，服务器都会限制页大小。

## 列

| 调用 | 结果 |
|---|---|
| `.width(px)`、`.flex(n)` | 固定宽度，或占用剩余宽度的一定份额 |
| `.filter()` | 适合该类型的筛选框：文本、数字范围、日期范围、选项列表 |
| `.sum()`、`.avg()`、`.min()`、`.max()` | 页脚中的合计。在文本列上使用 `sum` 无法通过编译 |
| `.badge(\|v\| tone)` | 将值显示为彩色徽章 |
| `.progress(max)` | 从 0 到 `max` 的进度条 |
| `.check_icon()` | 对 `true` 显示一个对勾 |
| `.link(\|row\| effect)` | 使单元格成为运行某个 effect 的链接 |
| `.pin_start()`、`.hidden()` | 固定在起始侧，或隐藏直到用户将其显示 |
| `.editable()` | 可就地编辑，通过 `.edit_with(action)` 保存 |
| `.plain()` | 整数不带分组分隔符，用于编码和年份 |

数字和金额右对齐，并使用等宽数字。整数按用户的区域设置分组（英语为 `1,234,567`，土耳其语为 `1.234.567`），适用于表格单元格以及 `DescriptionList` 等显示值；`#[key]` 标识从不分组，`.plain()` 可为单个列关闭分组。

## 行

- `.on_open(|row| effect)`：双击行或在行上按 Enter。
- `.row_action(label, action, |row| input)`：每行上的一个按钮。
- `.bulk_action(label, action, M::id)`：对所选行运行。
- 紧跟在 `.row_action` 或 `.bulk_action` 之后的 `.confirm(message)` 和 `.tone(Tone::Danger)`：在该操作运行前，用与 `ActionButton::confirm` 相同的对话框询问（焦点先在“取消”上），并把按钮画成红色。
- `.row_menu(|row| ContextMenu)`：行的右键菜单。
- `.row_style(|row| Option<RowStyle>)`：危险、警告或成功样式的行。
- `.exportable()`：将筛选后的行导出为 CSV，由服务器以流方式输出。

## 就地刷新

最后调用的 `.listens(topic)` 让网格在主题被点名时就地重新加载（见 [主题](effects.md#topics)）：`ui.refresh(topic)` 之后，它保留排序和筛选重新加载当前页；带 `.drives(topic)` 的筛选控件变化之后，它以新的界面 params 从第一页开始。网格经由带有全部权限检查的 `POST /_zeyn/grid` 重新加载，筛选改变的界面 param 通过界面函数传给网格，例如作为 scope：

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

[DataGrid 参考](grid.md)涵盖了已保存的视图、主从明细、分组、树形数据、就地编辑、单元格导航、复制和粘贴、列虚拟化以及无障碍。
