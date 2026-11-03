# 文本与 t!

用户看到的每一段文本都通过 `t!` 来自语言文件。编译器会检查每个语言文件都包含该键，并且参数与占位符一致，因此缺失翻译是构建错误，而不是空白的标签。

## 语言文件

`locales/<lang>.json` 包含嵌套的键。所有文件必须具有相同的键。

```json
{
  "stock": {
    "products": "Products",
    "saved": "Product {code} saved."
  }
}
```

## t!

```rust
Screen::new(t!("stock.products"))
ui.toast(t!("stock.saved", code = p.code));
```

`t!` 返回一个 `Text`：一个带有参数的键，在页面渲染时按用户的语言解析。拼写错误会在编译时失败：

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

## 字段与选项的标签

字段标签使用键 `<Model>.<field>`，例如 `Product.code`。选项标签使用 `<Choice>.<Variant>`。当键缺失时，标签会回退到 `#[label("...")]` 或名称的可读形式。`zeyn check` 会报告某些语言有而其他语言缺少的键。

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## 注册语言

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

`I18n::new` 的第一个参数是默认语言。存在多个语言目录（catalog）时，外壳会显示语言选择器。框架自身的文本（表格、表单、外壳）以英文和土耳其文提供。

## 界面之外的文本

- `ctx.t(&text)` 将 `Text` 转换为用户语言的 `String`，例如用于电子邮件。
- `Text::english()` 提供英文文本，用于日志和测试。
- `Text::from("literal")` 是永远不会被翻译的文本，例如商品编码。

关于如何选择语言，以及数字、金额和日期如何格式化，参见[时间与语言](time-and-languages.md)。
