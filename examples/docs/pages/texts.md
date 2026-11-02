# Texts and t!

Every text a user sees comes from the locale files through `t!`. The compiler checks that every locale file has the key and that the arguments match the placeholders, so a missing translation is a build error, not a blank label.

## Locale files

`locales/<lang>.json` holds nested keys. All files must have the same keys.

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

`t!` returns a `Text`: a key with its arguments, resolved in the user's language when the page renders. Typos fail at compile time:

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

## Labels of fields and choices

Field labels use the key `<Model>.<field>`, such as `Product.code`. Choice labels use `<Choice>.<Variant>`. When a key is missing the label falls back to `#[label("...")]` or a readable form of the name. `zeyn check` reports keys that some languages have and others lack.

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## Registering the languages

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

The first argument of `I18n::new` is the default language. With more than one catalog the shell shows a language picker. The framework's own texts (grid, forms, shell) ship in English and Turkish.

## Texts outside screens

- `ctx.t(&text)` turns a `Text` into a `String` in the user's language, such as for an email.
- `Text::english()` gives the English text for logs and tests.
- `Text::from("literal")` is a text that is never translated, such as a product code.

See [Time and languages](time-and-languages.md) for how the language is chosen and how numbers, money and dates are formatted.
