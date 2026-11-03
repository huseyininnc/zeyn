# 主题

状态：已在 `crates/zeyn/assets/zeyn.css` 中实现。

默认 UI 套件从 `:root` 上的 CSS 自定义属性读取每一种颜色、字体、尺寸和圆角。组织只需覆盖其中少数几个属性即可改变外观，无需编辑 `zeyn.css`，也无需编写选择器。

## 1. 主题如何应用

服务器先提供 `zeyn.css`，然后在应用有额外样式表时，再提供包含令牌（token）覆盖的样式表。覆盖文件只包含 `:root` 块：

```css
:root {
  --z-brand: #7a2448;
  --z-font: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --z-radius: 8px;
  --z-density: 0.75;
}
```

覆盖文件的规则：

- 必须从应用自身的源（origin）提供。内容安全策略（CSP）为 `style-src 'self'`，因此内联 `<style>` 块和第三方样式表都会被阻止。
- 字体必须已安装在用户的机器上，或从应用自身的源提供。套件本身不加载任何网络字体。
- 在 `zeyn.css` 之后加载，这样相同选择器按顺序决定胜负。
- 设置第 2 节中的基础令牌。第 3 节中的派生令牌会自动跟随。只有当计算出的值不适合你的品牌时，才覆盖派生令牌。

### 浅色、深色与应用主题

每个应用的菜单中都有一个主题选择器，位于语言选择器旁边。它提供 System（跟随 `prefers-color-scheme`）、Light、Dark 以及应用定义的任何主题：

```rust
App::new("Billing")
    .theme(Theme::new().brand("#7a2448").radius(6))
    .themes([
        Theme::light(),
        Theme::dark(),
        Theme::named("sand", [("--z-bg", "#f5f1ea"), ("--z-surface", "#fffdf8")]).label(t!("theme.sand")),
    ])
    .default_theme("system")
```

- `App::theme` 是组织的外观，对所有主题都生效。
- `App::themes` 列出用户可选择的主题。没有它时，列表为 Light 和 Dark。当两者都在列表中时，会提供 System。`Theme::named(...)` 以浅色主题为基础构建；`.dark_base()` 则以深色主题为基础。
- `App::default_theme` 是尚未做出选择的用户所使用的主题。未设置时为 `system`。
- `App::theme_picker(false)` 隐藏选择器；所有人都使用默认主题。
- 只有一个主题会锁定应用：`.themes([Theme::named("acme", tokens)])` 或 `.theme(Theme::dark())`。

令牌在应用启动时检查。`App::check`（因此 `router` 和 `serve` 也会）会报告未知的令牌名称、可能跳出其声明的值（`;`、花括号、`\`、`<`、`>`、`url(`）、无效的主题名称、未知的默认主题，以及某个主题使第 5 节中任何颜色对低于 WCAG 2.2 AA 的情况。只有 `#rgb` 和 `#rrggbb` 值可以检查对比度；其他颜色语法则被信任。

所有主题都放在同一个样式表 `/_zeyn/theme.css` 中，它在启动时构建一次，并在套件的样式表之后加载。所选主题保存在 `zeyn_theme` cookie 中；对于已登录且应用配有视图存储（`App::datagrid_views`）的用户，还会保存在该存储中，因此它会跟随用户到新设备。服务器在 `<html>` 上渲染主题，所以首次绘制就已正确，无需内联脚本：

| 标记 | 结果 |
|---|---|
| `<html>` | System：跟随 `prefers-color-scheme` |
| `<html data-theme="light">` | 始终为浅色 |
| `<html data-theme="dark">` | 始终为深色 |
| `<html data-theme="light" data-zeyn-theme="sand">` | 应用主题 `sand`，基于浅色主题 |

在选择器中切换主题只会改变这些属性和 `color-scheme` meta，然后调用 `POST /_zeyn/theme`。不会重新加载页面。

若要在手写的覆盖文件中为深色主题提供它自己的值，请重复套件使用的选择器：

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### 密度

`--z-density` 缩放间距单位。`1` 为常规，`0.75` 为紧凑。`<html data-density="compact">` 无需样式表即可设置为 `0.75`。控件和表格行永远不会小于 24 px，因此紧凑模式仍满足 WCAG 2.2 的目标尺寸要求。

虚拟表格的行高由服务器固定（`Paging::virtual_rows(px)`，默认 32 px），因为运行时依据它计算滚动位置。密度不会改变它。

## 2. 基础令牌

这些是组织预期要设置的令牌。

| 令牌 | 用途 | 浅色 | 深色 |
|---|---|---|---|
| `--z-brand` | 唯一的品牌色。主按钮、应用名称条、当前菜单项、聚焦行、焦点环 | `#0f6674` | 相同值；见 `--z-accent` |
| `--z-on-brand` | 实心 `--z-brand` 表面上的文字 | `#ffffff` | `#ffffff` |
| `--z-font` | 界面字体栈 | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | 相同 |
| `--z-font-mono` | 代码、API 示例 | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | 相同 |
| `--z-font-size` | 正文 | `0.875rem`（14 px） | 相同 |
| `--z-font-size-sm` | 表格单元格、标签、筛选器 | `0.8125rem`（13 px） | 相同 |
| `--z-font-size-xs` | 列标题、菜单分组标题、页面信息 | `0.75rem`（12 px） | 相同 |
| `--z-radius` | 基础圆角半径，0 到 12 px。所有组件都以它为基准缩放 | `4px` | 相同 |
| `--z-density` | 间距倍数，`1` 为常规，`0.75` 为紧凑 | `1` | 相同 |
| `--z-bg` | 面板后方的页面背景 | `#f4f5f6` | `#121416` |
| `--z-surface` | 面板、表格、窗口、输入框 | `#ffffff` | `#1a1d20` |
| `--z-surface-2` | 表格表头、斑马纹行、窗口页脚、只读输入框 | `#f7f8f9` | `#202326` |
| `--z-hover` | 行、菜单项、按钮的悬停背景 | `#eceef1` | `#2a2e33` |
| `--z-border` | 分隔线和面板轮廓 | `#dcdfe3` | `#33383e` |
| `--z-border-strong` | 输入框和按钮的轮廓、表头分隔线。相对 `--z-surface` 必须达到 3:1 | `#7d848d` | `#7a818a` |
| `--z-text` | 正文和值 | `#1a1d21` | `#e4e6e9` |
| `--z-text-muted` | 标签、表头、次要文字。相对 `--z-surface-2` 必须达到 4.5:1 | `#5b626b` | `#a2a9b1` |
| `--z-danger` | 错误、必填标记、危险行 | `#b3261e` | `#f28b82` |
| `--z-warning` | 警告行 | `#8a5300` | `#e6b45a` |
| `--z-success` | 成功行 | `#1e6b3a` | `#6fcf8f` |
| `--z-info` | 信息提示（toast） | `#2b5d8a` | `#8ab8e6` |

## 3. 派生令牌

由基础令牌通过 `color-mix()` 和相对颜色语法计算得出。基础令牌变化时它们会自动更新。

| 令牌 | 用途 | 浅色 | 深色 |
|---|---|---|---|
| `--z-accent` | 在表面上用作文字或细线的品牌色：链接、当前菜单项、聚焦行的竖条 | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`，即品牌色相配固定的浅色调。不支持相对颜色语法的浏览器使用 `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | 主按钮填充 | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | 主按钮文字 | `var(--z-on-brand)` | `#121416` |
| `--z-primary-hover` | 主按钮悬停 | 品牌色 86% 混合黑色 | 强调色 84% 混合白色 |
| `--z-primary-active` | 主按钮按下 | 品牌色 74% 混合黑色 | 强调色 70% 混合白色 |
| `--z-brand-subtle` | 当前菜单项和聚焦表格行背后的淡色 | 强调色以 `--z-tint` 叠加在 `--z-surface` 上 | 相同公式 |
| `--z-focus` | 焦点环 | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`、`--z-warning-subtle`、`--z-success-subtle`、`--z-info-subtle` | 行和消息的背景 | 状态色以 `--z-tint` 叠加在 `--z-surface` 上 | 相同公式 |
| `--z-tint` | 淡色的强度 | `9%` | `20%` |
| `--z-space` | 间距单位 | `calc(4px * var(--z-density))` | 相同 |
| `--z-control` | 按钮和输入框高度 | `max(24px, 8 * --z-space)`（32 px） | 相同 |
| `--z-row` | 分页和无限滚动表格的行高 | `max(24px, 8 * --z-space)`（32 px） | 相同 |
| `--z-gap` | 屏幕各部分之间的间隙 | `3 * --z-space`（12 px） | 相同 |
| `--z-radius-sm`、`--z-radius-lg` | 小型部件（筛选器、代码）、窗口 | `--z-radius` 的 `0.75x` 和 `1.5x` | 相同 |
| `--z-shadow-2` | 仅用于窗口和 toast。表格行永远没有阴影 | 柔和的双层阴影 | 更深 |
| `--z-backdrop` | 模态窗口后方 | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |

由服务器标记设置的元素级令牌：`--z-width`（窗口宽度）、`--z-flex`（列占比）、`--z-row-height`（虚拟表格行高），以及组件库使用的 `--z-stack-gap`、`--z-split`、`--z-progress`、`--z-columns`、`--z-level` 和 `--z-pop-x` / `--z-pop-y`（弹出层位置）。

图表系列颜色是 `components.css` 中的 `--z-chart-1` 到 `--z-chart-6`，在深色主题中取值更浅。每一个相对 `--z-surface` 都达到 3:1。可像覆盖任何基础令牌一样覆盖它们。

## 4. 选择品牌色

- `--z-brand` 在浅色主题中承载白色文字（按钮、应用名称条）。请选择相对 `--z-on-brand` 至少达到 4.5:1 的颜色。大多数中等到深色的品牌色都能通过。对于黄色这类浅色品牌色，还要把 `--z-on-brand` 设为深色。
- 在深色主题中，套件会把品牌色调亮用于文字、焦点环和主按钮，而应用名称条保留实心的品牌色。不同的品牌在两个主题中都会显得经过设计，无需第二种颜色。
- 让状态色与品牌色保持区分。红色品牌搭配红色错误行会让错误更难被发现；这种情况下，请把 `--z-danger` 调向另一种红色，或通过覆盖 `--z-brand-subtle` 让品牌色不出现在表格里。
- 以第 5 节的表格为目标检查你的颜色。任何低于阈值的颜色对都不符合 WCAG 2.2 AA。

## 5. 已验证的对比度

在运行的 showcase 中于 Chromium 里测量：每个令牌都被解析为其渲染后的 sRGB 值（包括 `color-mix()` 和相对颜色），然后计算 WCAG 2.x 对比度。阈值：文字为 4.5:1，UI 组件轮廓和焦点指示器为 3:1。

| 颜色对 | 浅色 | 深色 | 要求 |
|---|---|---|---|
| `--z-text` 在 `--z-bg` 上 | 15.49 | 14.77 | 4.5 |
| `--z-text` 在 `--z-surface` 上 | 16.91 | 13.54 | 4.5 |
| `--z-text` 在 `--z-surface-2` 上 | 15.91 | 12.63 | 4.5 |
| `--z-text` 在 `--z-hover` 上 | 14.55 | 10.93 | 4.5 |
| `--z-text` 在 `--z-brand-subtle` 上（聚焦行） | 14.79 | 9.27 | 4.5 |
| `--z-text` 在 `--z-danger-subtle` 上 | 14.73 | 9.76 | 4.5 |
| `--z-text` 在 `--z-warning-subtle` 上 | 14.82 | 9.31 | 4.5 |
| `--z-text` 在 `--z-success-subtle` 上 | 14.76 | 9.33 | 4.5 |
| `--z-text-muted` 在 `--z-bg` 上 | 5.65 | 7.78 | 4.5 |
| `--z-text-muted` 在 `--z-surface` 上 | 6.17 | 7.13 | 4.5 |
| `--z-text-muted` 在 `--z-surface-2` 上（表头） | 5.80 | 6.65 | 4.5 |
| `--z-text-muted` 在 `--z-hover` 上 | 5.31 | 5.76 | 4.5 |
| `--z-text-muted` 在 `--z-danger-subtle` 上（弱化行） | 5.37 | 5.14 | 4.5 |
| `--z-text-muted` 在 `--z-warning-subtle` 上 | 5.41 | 4.90 | 4.5 |
| `--z-accent` 在 `--z-surface` 上（链接） | 6.62 | 9.33 | 4.5 |
| `--z-accent` 在 `--z-bg` 上 | 6.06 | 10.18 | 4.5 |
| `--z-accent` 在 `--z-brand-subtle` 上（当前菜单项） | 5.79 | 6.39 | 4.5 |
| `--z-on-primary` 在 `--z-primary` 上 | 6.62 | 10.18 | 4.5 |
| `--z-on-primary` 在 `--z-primary-hover` 上 | 8.82 | 11.26 | 4.5 |
| `--z-on-primary` 在 `--z-primary-active` 上 | 11.12 | 12.26 | 4.5 |
| `--z-on-brand` 在 `--z-brand` 上（应用名称条） | 6.62 | 6.62 | 4.5 |
| `--z-danger` 在 `--z-surface` 上（字段错误） | 6.54 | 7.09 | 4.5 |
| `--z-danger` 在 `--z-danger-subtle` 上 | 5.69 | 5.11 | 4.5 |
| `--z-warning` 在 `--z-surface` 上 | 6.33 | 8.90 | 4.5 |
| `--z-warning` 在 `--z-warning-subtle` 上 | 5.55 | 6.12 | 4.5 |
| `--z-success` 在 `--z-surface` 上 | 6.52 | 8.87 | 4.5 |
| `--z-info` 在 `--z-surface` 上 | 6.92 | 8.13 | 4.5 |
| `--z-focus` 在 `--z-surface` 上 | 6.62 | 9.33 | 3 |
| `--z-focus` 在 `--z-bg` 上 | 6.06 | 10.18 | 3 |
| `--z-focus` 在 `--z-brand-subtle` 上 | 5.79 | 6.39 | 3 |
| `--z-border-strong` 在 `--z-surface` 上（输入框轮廓） | 3.78 | 4.30 | 3 |
| `--z-border-strong` 在 `--z-surface-2` 上 | 3.55 | 4.01 | 3 |

`--z-border` 仅作装饰（用于已带有文字的条目之间的分隔线），不要求达到 3:1。

## 6. 其他无障碍行为

- 每个可交互元素都显示 2 px 的 `:focus-visible` 焦点环，颜色为 `--z-focus`。表格行把焦点环画在行内部，因此不会被滚动容器裁切。
- 按钮、输入框、排序表头、筛选框、分页按钮和窗口关闭按钮在任何密度下都至少为 24 x 24 px。
- `prefers-reduced-motion: reduce` 会移除 toast 的进入动画和所有过渡。
- 在 `forced-colors` 模式下，图标使用 `CanvasText`，主按钮和聚焦行使用 `Highlight`，依赖背景淡色的状态（当前菜单项、行状态）则改用轮廓或边框。
