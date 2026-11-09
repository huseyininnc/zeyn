# 控件和选择器

表单字段根据其类型选择控件。`.control(M::f, Control::...)` 更改控件，`.lookup(M::f, action)` 将字段变成对服务器的搜索。每个控件底层都是原生表单元素，因此无论有没有运行时，值的提交方式都相同，并且每个控件都遵循其 ARIA 模式，方便键盘和屏幕阅读器用户使用。

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## 默认控件

| 字段类型 | 控件 |
|---|---|
| `String` | 文本输入框；当 `max_len` 超过 255 时为带字符计数器的自动增高文本域 |
| `i32`、`i64` | 带 `min` 和 `max` 规则的数字输入框 |
| `Money` | 按用户数字格式显示的金额，带货币 |
| `Decimal` | 按用户格式显示的精确数字，带字段的 `scale`（参见[Decimal 输入](#decimal-and-percent)） |
| `bool` | 复选框 |
| `#[choice]` 枚举 | 下拉选择：点击后打开可搜索列表的按钮 |
| `#[choice]` 枚举的 `Vec` | 多选：以标签（chip）显示，并用列表选择多个选项 |
| `Date`、`Time`、`DateTime`、`LocalDateTime` | 日期和时间选择器 |
| 模型的 `Vec<M>` | 行编辑器（参见[行编辑器](lines.md)） |

## 其他控件

| 控件 | 用途 |
|---|---|
| `Control::Radio` | 选项较少且全部可见的选择 |
| `Control::Segmented` | 有两到五个简短选项的选择 |
| `Control::Listbox` | 带输入搜索的长选项列表 |
| `Control::Switch` | 作为设置立即生效的 `bool` |
| `Control::Percent` | 带百分号显示的 `Decimal` 或整数 |
| `Control::Password`、`Control::NewPassword` | 按密码类型输入的文本字段，带显示和隐藏按钮 |
| `Control::PasswordMeter` | 带强度指示条的新密码 |
| `Control::Rating` | 1 到 N 的整数，以星星显示；`Decimal` 字段可用半颗星（见 [Rating](#rating)） |
| `Control::Multiline` | 任意长度的文本，使用会自动增高的文本区域 |
| `Control::Color { palette }` | 保存十六进制颜色的 `String`，带取色器（见 [Color and length](#color-and-length)） |
| `Control::Length { units }` | 以像素保存、按用户所选单位显示的 `Decimal` 长度（见 [Color and length](#color-and-length)） |

## Rating

`Control::Rating` 把整数字段显示为 1 到 N 颗星，N 为字段的 `max` 规则（没有规则时为 5，最大为 10）。它是放在 `fieldset` 中的一组原生单选按钮，`fieldset` 的 legend 就是字段标签，因此无需脚本即可遵循 W3C APG 单选组模式：Tab 只进入该组一次，方向键移动并选中，每颗星以用户的语言命名为“3 / 5”。鼠标悬停时，星星会亮到指针所在位置。服务器像校验其他数字一样用字段规则校验该值，所以请为字段设置 `min` 和 `max`；在 `Option<i64>` 上使用 `required` 时，不选任何星星会被拒绝。带 `#[rule(min = 1, max = 5, scale = 1)]` 的 `Decimal` 字段可选半颗星，服务器会拒绝介于两个半星之间的值，例如 4.3（见 [商店组件](commerce.md)）。

```rust
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

Form::of(Feedback::default())
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit(t!("feedback.send"), send_feedback)
```

@live rating


## Color and length

```rust
const BRAND: &[&str] = &["#1E88E5", "#43A047", "#00000080"];

Form::of(band)
    .field(Band::fill)
    .control(Band::fill, Control::Color { palette: BRAND })
    .field(Band::height)
    .control(Band::height, Control::Length { units: &[LengthUnit::Mm, LengthUnit::Pt] })
```

`Control::Color` 把 `String` 字段保存为 `#RRGGBB` 或 `#RRGGBBAA`。字段是带颜色块的文本输入框，因此没有脚本也能使用；它的按钮会打开一个对话框，里面有红、绿、蓝和不透明度滑块、最近使用的颜色以及应用的调色板。按 Escape 或“完成”关闭对话框并把焦点还回去。服务器拒绝不是十六进制颜色的值。

`Control::Length` 把 `Decimal` 字段保存为 CSS 像素（每英寸 96 像素）。用户以毫米、厘米、英寸、磅或像素输入，并在数字旁边选择单位；浏览器会为所有长度字段记住这个单位。上下方向键每次步进 0.1，按住 Shift 时步进 1，范围是以像素表示的 `min` 和 `max` 规则。请给字段设置 `scale = 4`。

## Lookup

Lookup 在用户输入时于服务器上搜索记录。action 接收 `LookupQuery` 并返回 `Page<LookupItem>`；字段保存 id。

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)` 对 `Vec<i64>` 字段做同样的事，并将所选项显示为可移除的标签。

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` 在列表中显示多列。搜索仍然发送到服务器上的 action，每次按键的搜索都会检查其权限；每个条目通过 `LookupItem::cells` 按表头顺序给出各列的值。`LookupQuery::apply` 也会搜索这些单元格。屏幕阅读器会将每个选项读作“Code: C-2, Name: Mert Kaya, City: Ankara”。在窄屏幕上，各列在每个选项内堆叠显示。

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## 多选

`#[choice]` 枚举的 `Vec` 使用多选进行编辑：所选选项显示为带移除按钮的标签（chip），组合框打开一个列表，在其中开启或关闭选项（APG listbox，带 `aria-multiselectable`，位于组合框内）。底层是原生的 `<select multiple>`，因此字段在没有运行时的情况下也能工作。运行时以逗号连接的形式提交值（`Email,Sms`）；服务器逐个读取，拒绝未知值并去除重复项。在 JSON（API、AI 工具、数据库）中，该字段是选项值的数组。

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| 按键 | 作用 |
|---|---|
| Arrow Down、Arrow Up、Enter、Space | 在已关闭的组合框上打开列表 |
| Home、End | 打开列表并定位到第一个或最后一个选项 |
| Arrow Down、Arrow Up | 在已打开的列表中移动 |
| Shift 加 Arrow Down 或 Arrow Up | 移动并开启或关闭下一个选项 |
| Space、Enter | 开启或关闭当前选项；列表保持打开 |
| 字母 | 跳到下一个以这些字母开头的选项 |
| Backspace、Delete | 在已关闭的组合框上：移除最后选择的选项 |
| Escape、Alt 加 Arrow Up | 关闭列表；焦点保持在组合框上 |
| Tab | 关闭列表并继续移动 |

Ctrl+A（APG 模式中的全选）按照快捷键策略的要求交给浏览器处理；标签的移除按钮在 Tab 顺序中。列表、标签和 lookup 各列共需约 1.7 KB gzip 的脚本，没有请求；该字段是 3 个节点加上每个所选选项 1 个。

## Decimal 和百分比

`Decimal` 字段保存精确数字：从输入、服务器、数据库到导出，全程没有浮点数。输入框在键入时按用户的语言格式化数字（土耳其语为 `1.234,5678`，英语为 `1,234.5678`），并允许字段的 `scale` 所规定的小数位数；服务器按同样的语言读取，并再次检查 `scale`、`precision`、`min` 和 `max`。

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent` 以百分点保存值：`12.5` 在英语中显示为 `12.5 %`，在土耳其语中显示为 `%12,5`，存储为 `12.5`。屏幕阅读器通过输入框的描述读出“percent”一词。

## 掩码输入

带有 `mask` 规则的文本字段会在用户输入时显示其模式，并且只提交原始字符。模式即规则：服务器会去掉输入的固定字符（`(532) 123 45 67` 变为 `5321234567`），将 `A` 位置转为大写，并拒绝没有填满每个位置的值。API 调用和 AI 工具发送原始值，同样会得到相同的检查。

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| 模式字符 | 含义 |
|---|---|
| `9` | 一个数字 |
| `a` | 一个字母 |
| `A` | 一个字母，以大写书写 |
| `*` | 一个字母或数字 |
| 反斜杠 | 下一个字符是字面量，例如 `\T` |
| 其他任何字符 | 字面量：显示，但从不存储 |

没有运行时的情况下，该字段是保存原始值的普通文本输入框；用户输入时可以带也可以不带字面量。

## 密码

`Control::Password`（登录）、`Control::NewPassword`（注册、修改）和 `Control::PasswordMeter`（带强度指示条的新密码）是密码输入框，绝不会由服务器填充，也绝不会保存在草稿中。显示和隐藏按钮遵循 APG 切换按钮模式：其名称始终是“Show password”，`aria-pressed` 表明文本是否显示。表单提交时会将密码重新切换为隐藏。

指示条是浏览器根据长度、字符种类和明显模式计算出的提示，以四个条和一段文字（“Strength: good”）显示在礼貌型（polite）live region 中。服务器接受什么由字段的规则（`min_len`、模型的 `validate` 函数）决定，绝不由指示条决定。

@live inputs

### 键盘

| 按键 | 位置 | 作用 |
|---|---|---|
| Arrow Up、Arrow Down | decimal 和百分比输入框 | 在 `min` 和 `max` 范围内按一个整数单位步进 |
| Page Up、Page Down | decimal 和百分比输入框 | 按十步进 |
| Backspace | 掩码输入框 | 删除光标前的字符；在字面量之上时，删除该字面量之前的字符 |
| Tab | 密码输入框 | 移到显示和隐藏按钮 |
| Enter、Space | 显示和隐藏按钮 | 显示或隐藏密码；焦点保持在按钮上 |

### 开销与检查

| | |
|---|---|
| 新增脚本 | `inputs.js` 中约 2 KB gzip（7 KB 原始），用于掩码、切换和指示条；decimal 和百分比输入框使用 number 控件 |
| DOM 节点 | 掩码输入框 3 个，带切换按钮的密码 4 个，带指示条 11 个 |
| 请求 | 输入时没有 |
| 服务器检查 | `scale`、`precision`、`min`、`max` 和 `mask` 在每次保存时运行，页面、API 和 AI 工具一视同仁 |

## 日期和时间选择器

选择器接受用户格式的键入输入，并打开日历或时间列表。`DateOptions` 设置限制：`.no_weekends()`、`.week_numbers()`、`.step_minutes(15)`。`.range(from, to)` 将两个日期字段合并为一个范围选择器，带有诸如 *This month* 的预设。在线示例见[时间和语言](time-and-languages.md#pickers)。
