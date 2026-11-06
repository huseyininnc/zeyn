# 管理视图

面向后台的组件：记录中改了什么、只读的代码和数据、条码、通知、看板和日历。每个组件都在服务器上渲染，只有用到它的页面才加载它的样式表和脚本。

## DiffView

`DiffView` 显示一条记录两个值之间改变的字段：旧值加删除线，新值标为新增。可以用 `DiffView::changes(entry.changes)` 从审计记录构建，也可以用 `DiffView::of(&before, &after)` 从模型的两个值构建，后者的比较方式与审计跟踪相同（不含行版本）。

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` 让字段显示用户语言的标签，让选项字段显示选项标签。并排布局是一个表格，一列放旧值，一列放新值；`DiffLayout::Inline` 把新值放在旧值后面，适合窗口这样的窄处。最多显示 `MAX_DIFF_FIELDS`（200）个字段，其余的用一条说明提示。

掩码字段保持掩码。值是审计跟踪保存的掩码文本，模型用 `#[mask]` 标记的字段在显示前会再次掩码，因此即使是手工构建的变更列表也不会把原始值放进页面。该组件不需要脚本；`diff.css`（1.9 KB）只随它加载。
