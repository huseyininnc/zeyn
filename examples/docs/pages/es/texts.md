# Textos y t!

Cada texto que ve un usuario procede de los archivos de locale a través de `t!`. El compilador comprueba que cada archivo de locale tenga la clave y que los argumentos coincidan con los marcadores, así que una traducción que falta es un error de compilación, no una etiqueta vacía.

## Archivos de locale

`locales/<lang>.json` contiene claves anidadas. Todos los archivos deben tener las mismas claves.

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

`t!` devuelve un `Text`: una clave con sus argumentos, resuelta en el idioma del usuario cuando se renderiza la página. Los errores tipográficos fallan en tiempo de compilación:

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

## Etiquetas de campos y de choices

Las etiquetas de campo usan la clave `<Model>.<field>`, como `Product.code`. Las etiquetas de choice usan `<Choice>.<Variant>`. Cuando falta una clave, la etiqueta recurre a `#[label("...")]` o a una forma legible del nombre. `zeyn check` informa de las claves que algunos idiomas tienen y otros no.

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## Registrar los idiomas

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

El primer argumento de `I18n::new` es el idioma por defecto. Con más de un catálogo, el shell muestra un selector de idioma. Los textos propios del framework (tabla, formularios, shell) se distribuyen en inglés y turco.

## Textos del framework en más idiomas

Los textos propios del framework también existen en chino, ruso, español, coreano, japonés, hindi y árabe. Cada idioma es una feature de Cargo, así que una app solo paga por los que activa (cada uno añade entre 35 y 60 KB de texto al binario):

```toml
zeyn = { version = "0.0", features = ["lang-ru", "lang-ar"] }
```

`lang-all` activa todos los idiomas. La app solo ofrece un idioma cuando registra su propio catálogo para él, como arriba. Sin la feature, los textos del framework usan el idioma predeterminado.

## Textos fuera de las pantallas

- `ctx.t(&text)` convierte un `Text` en un `String` en el idioma del usuario, por ejemplo para un correo electrónico.
- `Text::english()` da el texto en inglés para registros y pruebas.
- `Text::from("literal")` es un texto que nunca se traduce, como un código de producto.

Consulta [Hora e idiomas](time-and-languages.md) para saber cómo se elige el idioma y cómo se formatean los números, el dinero y las fechas.
