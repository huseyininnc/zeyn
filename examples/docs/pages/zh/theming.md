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

两种密度，通过令牌切换。舒适是默认值：40 px 表格行、36 px 表头、36 px 控件、20 px 卡片内边距。紧凑（`<html data-density="compact">`）的行、表头和控件均为 32 px，并把间距单位缩放到 0.8。用户在顶栏的账户菜单中选择它（`App::header(|h| h.account())`）；该选择保存在 `zeyn_density` cookie 中，服务器把它渲染到 `<html>` 上，因此首次绘制就是正确的。控件和表格行永远不会小于 24 px，因此紧凑模式仍满足 WCAG 2.2 的目标尺寸要求。`Theme::new().compact()` 让紧凑间距成为组织的默认值。

虚拟表格的行高由服务器固定，因为运行时依据它计算滚动位置。`Paging::virtual_scroll()` 跟随密度（舒适 40 px，紧凑 32 px）；`Paging::virtual_rows(px)` 保持给定的高度。密度变化从下一页起作用于虚拟表格。

## 2. 基础令牌

这些是组织预期要设置的令牌。

| 令牌 | 用途 | 浅色 | 深色 |
|---|---|---|---|
| `--z-brand` | 唯一的品牌色。徽标、主按钮、当前菜单项、焦点环 | `#0f6674` | 相同值；见 `--z-accent` |
| `--z-on-brand` | 实心 `--z-brand` 表面上的文字 | `#ffffff` | `#ffffff` |
| `--z-font` | 界面字体栈 | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | 相同 |
| `--z-font-mono` | 代码、API 示例 | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | 相同 |
| `--z-font-size` | 正文 | `0.875rem`（14 px） | 相同 |
| `--z-font-size-sm` | 表格单元格、标签、筛选器 | `0.8125rem`（13 px） | 相同 |
| `--z-font-size-xs` | 列标题、菜单分组标题、页面信息 | `0.75rem`（12 px） | 相同 |
| `--z-radius` | 基础圆角半径，0 到 12 px。按钮、输入框、菜单项；其他所有圆角都由它派生 | `6px` | 相同 |
| `--z-density` | 间距倍数，`1` 为常规，`0.75` 为紧凑 | `1` | 相同 |
| `--z-bg` | 面板后方的页面背景 | `#f5f6f8` | `#0d0f12` |
| `--z-surface` | 面板、表格、卡片、输入框 | `#ffffff` | `#15181d` |
| `--z-surface-2` | 表格表头和表尾、窗口页脚、只读输入框 | `#f8f9fa` | `#1a1e24` |
| `--z-raised` | 弹出层、菜单、对话框、toast、命令面板。在深色主题中，浮起的层级用更亮的颜色区分，而不依赖阴影 | `#ffffff` | `#1f242b` |
| `--z-hover` | 行、菜单项、按钮的悬停背景 | `#eff1f4` | `#232830` |
| `--z-border` | 分隔线和面板轮廓 | `#e2e5e9` | `#2b3139` |
| `--z-border-strong` | 输入框轮廓、复选框边框、图表基线。相对 `--z-surface` 必须达到 3:1 | `#858d97` | `#6f7884` |
| `--z-text` | 正文和值 | `#15181d` | `#e7eaee` |
| `--z-text-muted` | 标签、表头、次要文字。相对 `--z-surface-2` 必须达到 4.5:1 | `#5a626c` | `#9ba4af` |
| `--z-danger` | 错误、必填标记、危险行 | `#b42318` | `#f28b82` |
| `--z-warning` | 警告行 | `#8a5300` | `#e6b45a` |
| `--z-success` | 成功行 | `#1a6b3c` | `#6fcf8f` |
| `--z-info` | 信息提示（toast） | `#275d93` | `#8ab8e6` |

## 3. 派生令牌

由基础令牌通过 `color-mix()` 和相对颜色语法计算得出。基础令牌变化时它们会自动更新。

| 令牌 | 用途 | 浅色 | 深色 |
|---|---|---|---|
| `--z-accent` | 在表面上用作文字或细线的品牌色：链接、当前菜单项、聚焦行的竖条 | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`，即品牌色相配固定的浅色调。不支持相对颜色语法的浏览器使用 `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | 主按钮填充 | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | 主按钮文字 | `var(--z-on-brand)` | `#0d0f12` |
| `--z-primary-hover` | 主按钮悬停 | 品牌色 86% 混合黑色 | 强调色 84% 混合白色 |
| `--z-primary-active` | 主按钮按下 | 品牌色 74% 混合黑色 | 强调色 70% 混合白色 |
| `--z-brand-subtle` | 当前菜单项和聚焦表格行背后的淡色 | 强调色以 `--z-tint` 叠加在 `--z-surface` 上 | 相同公式 |
| `--z-focus` | 焦点环 | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`、`--z-warning-subtle`、`--z-success-subtle`、`--z-info-subtle` | 行和消息的背景 | 状态色以 `--z-tint` 叠加在 `--z-surface` 上 | 相同公式 |
| `--z-tint` | 淡色的强度 | `9%` | `16%` |
| `--z-text-2xs` 到 `--z-text-num-lg`，以及 `--z-leading-*` | 字号阶梯，字号/行高：`2xs` 11/16（按键、手机上的图表刻度）、`xs` 12/16（表格表头、说明文字、徽章）、`sm` 13/20（单元格、导航、标签、按钮）、`md` 14/20（正文、输入框）、`lg` 16/24（卡片、图表和对话框的分节标题）、`xl` 20/28（窗口标题、手机上的页面标题）、`2xl` 24/32（页面标题）、`num` 30/36（KPI 数值）、`num-lg` 40/44（主指标）。字重：文字 400，标签、按钮和当前项 500，仅标题和数字用 600 | | 相同 |
| `--z-space` | 间距单位 | `calc(4px * var(--z-density))` | 相同 |
| `--z-control` | 按钮和输入框高度 | `max(24px, 9 * --z-space)`（36 px；紧凑 32 px） | 相同 |
| `--z-row`、`--z-row-head` | 分页和无限滚动表格的行高和表头高度 | 40 px 和 36 px（紧凑 32 px） | 相同 |
| `--z-gap` | 卡片之间以及屏幕各部分之间的间隙 | `4 * --z-space`（16 px） | 相同 |
| `--z-s-0_5`、`--z-s-1`、`--z-s-1_5`、`--z-s-2`、`--z-s-3`、`--z-s-4`、`--z-s-5`、`--z-s-6`、`--z-s-8`、`--z-s-10`、`--z-s-12`、`--z-s-16` | 间距阶梯：数字乘以 `--z-space`（密度为 1 时为 2、4、6、8、12、16、20、24、32、40、48 和 64 px） | | 相同 |
| `--z-radius-sm` | 徽章、标签片、复选框、键盘按键 | `max(2px, --z-radius - 2px)`（4 px） | 相同 |
| `--z-radius-lg` | 卡片、表格、弹出层、toast | `--z-radius + 2px`（8 px） | 相同 |
| `--z-radius-xl` | 对话框、登录卡片、命令面板 | `--z-radius + 6px`（12 px） | 相同 |
| `--z-radius-full` | 头像、开关、状态圆点、计数胶囊 | `999px` | 相同 |
| `--z-shadow-1` | 默认按钮、分段控件的滑块 | `0 1px 2px`，不透明度 6% | 无 |
| `--z-shadow-2` | 弹出层、菜单、工具提示、toast。表格行和卡片永远没有阴影 | 两层柔和阴影 | 1 px `--z-border` 描边环和一层深阴影 |
| `--z-shadow-3` | 对话框和命令面板 | 两层更深的阴影 | 1 px 描边环和一层更深的阴影 |
| `--z-dur-1`、`--z-dur-2`、`--z-dur-3`、`--z-dur-out` | 动效：悬停颜色（80 ms）、弹出层和 toast 进入（140 ms）、对话框、抽屉和命令面板进入（200 ms）、所有退出（100 ms）。在 `prefers-reduced-motion: reduce` 下它们变为 0，淡入淡出最多 80 ms | | 相同 |
| `--z-ease-out`、`--z-ease-in`、`--z-ease-linear` | 进入和移动、退出以及颜色变化的缓动曲线 | `cubic-bezier(0.2, 0, 0, 1)`、`cubic-bezier(0.4, 0, 1, 1)`、`linear` | 相同 |
| `--z-backdrop` | 模态窗口后方 | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | 阅读方向的符号，用于没有逻辑属性形式的偏移（标记起始或结束侧的内阴影、滑动变换）：`calc(2px * var(--z-dir))` | `1`，在 `dir="rtl"` 下为 `-1` | 相同 |

由服务器标记设置的元素级令牌：`--z-width`（窗口宽度）、`--z-flex`（列占比）、`--z-row-height`（虚拟表格行高），以及组件库使用的 `--z-stack-gap`、`--z-split`、`--z-progress`、`--z-columns`、`--z-level` 和 `--z-pop-x` / `--z-pop-y`（弹出层位置）。

图表系列颜色是 `components.css` 中的 `--z-chart-1` 到 `--z-chart-8`：蓝 `#2563d9`、橙 `#d67400`、品红 `#a3369b`、绿 `#14866d`、橄榄 `#7a6a00`、紫 `#6e56cf`、红 `#cc3b33` 和石板灰 `#56636f`，在深色主题中取值更浅（`#5b9bff`、`#f08a3c`、`#e27bd6`、`#38c29f`、`#ece27a`、`#9d8cff`、`#ff7b70`、`#9aa7b5`）。这一顺序让前五种颜色在绿色盲、红色盲和蓝色盲下仍可区分，而且系列 1 不是品牌色，因此红色或绿色的品牌永远不会被读作好或坏；应用仍可设置 `--z-chart-1: var(--z-brand)`。启动检查要求每一种相对 `--z-surface` 达到 3:1。可像覆盖任何基础令牌一样覆盖它们。

## 4. 选择品牌色

- `--z-brand` 在浅色主题中承载白色文字（主按钮、徽标字母标记、登录 shell 的品牌面板）。请选择相对 `--z-on-brand` 至少达到 4.5:1 的颜色。大多数中等到深色的品牌色都能通过。对于黄色这类浅色品牌色，还要把 `--z-on-brand` 设为深色。
- 在深色主题中，套件会把品牌色调亮用于文字、焦点环和主按钮，而徽标字母标记保留实心的品牌色。不同的品牌在两个主题中都会显得经过设计，无需第二种颜色。
- 让状态色与品牌色保持区分。红色品牌搭配红色错误行会让错误更难被发现；这种情况下，请把 `--z-danger` 调向另一种红色，或通过覆盖 `--z-brand-subtle` 让品牌色不出现在表格里。
- 以第 5 节的表格为目标检查你的颜色。任何低于阈值的颜色对都不符合 WCAG 2.2 AA。

## 5. 已验证的对比度

下表中的每一对颜色都会在应用启动时（`App::check`）针对内置主题和每个应用主题进行检查，也由 `src/server/theme.rs` 中的单元测试 `built_in_themes_reach_wcag_aa_on_every_pair` 检查（用 `--nocapture` 运行它即可打印此表）。派生令牌（`--z-accent`、主色和淡色）按照 `zeyn.css` 的计算方式在 Oklab 中计算。阈值：文字为 4.5:1，UI 组件轮廓和焦点指示器为 3:1。

| 颜色对 | 浅色 | 深色 | 要求 |
|---|---|---|---|
| `--z-text` 在 `--z-bg` 上 | 16.45 | 15.90 | 4.5 |
| `--z-text` 在 `--z-surface` 上 | 17.79 | 14.74 | 4.5 |
| `--z-text` 在 `--z-surface-2` 上 | 16.88 | 13.86 | 4.5 |
| `--z-text` 在 `--z-raised` 上 | 17.79 | 12.93 | 4.5 |
| `--z-text` 在 `--z-hover` 上 | 15.72 | 12.27 | 4.5 |
| `--z-text` 在 `--z-brand-subtle` 上 | 15.56 | 11.18 | 4.5 |
| `--z-text` 在 `--z-danger-subtle` 上 | 15.49 | 11.60 | 4.5 |
| `--z-text` 在 `--z-warning-subtle` 上 | 15.59 | 11.20 | 4.5 |
| `--z-text` 在 `--z-success-subtle` 上 | 15.53 | 11.23 | 4.5 |
| `--z-text` 在 `--z-info-subtle` 上 | 15.52 | 11.40 | 4.5 |
| `--z-text-muted` 在 `--z-bg` 上 | 5.71 | 7.61 | 4.5 |
| `--z-text-muted` 在 `--z-surface` 上 | 6.18 | 7.05 | 4.5 |
| `--z-text-muted` 在 `--z-surface-2` 上 | 5.86 | 6.63 | 4.5 |
| `--z-text-muted` 在 `--z-raised` 上 | 6.18 | 6.19 | 4.5 |
| `--z-text-muted` 在 `--z-hover` 上 | 5.46 | 5.87 | 4.5 |
| `--z-text-muted` 在 `--z-brand-subtle` 上 | 5.40 | 5.35 | 4.5 |
| `--z-border-strong` 在 `--z-surface` 上 | 3.36 | 3.98 | 3 |
| `--z-border-strong` 在 `--z-surface-2` 上 | 3.19 | 3.74 | 3 |
| `--z-border-strong` 在 `--z-raised` 上 | 3.36 | 3.49 | 3 |
| `--z-accent` 在 `--z-surface` 上 | 6.62 | 9.81 | 4.5 |
| `--z-accent` 在 `--z-bg` 上 | 6.12 | 10.58 | 4.5 |
| `--z-accent` 在 `--z-brand-subtle` 上 | 5.79 | 7.44 | 4.5 |
| `--z-focus` 在 `--z-surface` 上 | 6.62 | 9.81 | 3 |
| `--z-focus` 在 `--z-bg` 上 | 6.12 | 10.58 | 3 |
| `--z-on-primary` 在 `--z-primary` 上 | 6.62 | 10.58 | 4.5 |
| `--z-on-primary` 在 `--z-primary-hover` 上 | 8.82 | 11.70 | 4.5 |
| `--z-on-primary` 在 `--z-primary-active` 上 | 11.12 | 12.74 | 4.5 |
| `--z-danger` 在 `--z-surface` 上 | 6.57 | 7.45 | 4.5 |
| `--z-danger` 在 `--z-danger-subtle` 上 | 5.73 | 5.86 | 4.5 |
| `--z-warning` 在 `--z-surface` 上 | 6.33 | 9.35 | 4.5 |
| `--z-warning` 在 `--z-warning-subtle` 上 | 5.55 | 7.11 | 4.5 |
| `--z-success` 在 `--z-surface` 上 | 6.54 | 9.32 | 4.5 |
| `--z-success` 在 `--z-success-subtle` 上 | 5.70 | 7.09 | 4.5 |
| `--z-info` 在 `--z-surface` 上 | 6.83 | 8.54 | 4.5 |
| `--z-info` 在 `--z-info-subtle` 上 | 5.96 | 6.60 | 4.5 |
| `--z-on-brand` 在 `--z-brand` 上 | 6.62 | 6.62 | 4.5 |
| `--z-brand` 在 `--z-surface` 上 | 6.62 | 不适用 | 4.5 |

`--z-border` 仅作装饰（用于已带有文字的条目之间的分隔线），不要求达到 3:1。

## 6. 其他无障碍行为

- 每个可交互元素都显示 2 px 的 `:focus-visible` 焦点环，颜色为 `--z-focus`。表格行把焦点环画在行内部，因此不会被滚动容器裁切。
- 按钮、输入框、排序表头、筛选框、分页按钮和窗口关闭按钮在任何密度下都至少为 24 x 24 px。
- `prefers-reduced-motion: reduce` 会移除 toast 的进入动画和所有过渡。
- 在 `forced-colors` 模式下，图标使用 `CanvasText`，主按钮和聚焦行使用 `Highlight`，依赖背景淡色的状态（当前菜单项、行状态）则改用轮廓或边框。
