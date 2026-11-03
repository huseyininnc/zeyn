# Entidades y modelos

Los datos se describen una sola vez, como un struct de Rust con atributos. La misma descripción gobierna los formularios, las columnas de las tablas, la validación en el servidor y en el navegador, la API JSON y su esquema OpenAPI.

## Entity

`#[entity(table = "...")]` marca una fila de tabla. Necesita un campo `#[key]` y crea un id tipado, como `ProductId`, y una referencia de campo para cada campo, como `Product::code`.

```rust
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}
```

Las referencias de campo están tipadas: `Product::price` es un `Field<Product, Money>`. Un total de tabla sobre una columna de texto o un campo de formulario de otro modelo no compila.

## Model

`#[model]` son datos que no son una fila de tabla: filtros, pasos de un asistente, entrada de formulario.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` convierte un enum de variantes unitarias en un select, un grupo de radio o un control segmentado. Las etiquetas vienen de los archivos de locale.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## Atributos de campo

| Atributo | Significado |
|---|---|
| `#[key]` | La clave primaria de una entidad |
| `#[version]` | Una versión de fila `i64`. Los guardados con una versión antigua fallan con `Error::Conflict` |
| `#[readonly]` | Se muestra, los formularios nunca lo escriben |
| `#[label("...")]` | Etiqueta de reserva cuando los archivos de locale no tienen ninguna |
| `#[rule(...)]` | Reglas de validación, más abajo |

## Reglas

| Regla | En | Significado |
|---|---|---|
| `required` | cualquiera | Debe tener un valor |
| `min_len = n`, `max_len = n` | texto | Longitud en caracteres |
| `email` | texto | Una dirección de correo electrónico |
| `embed` | texto | Una dirección para incrustar con `Embed`: `https` en un host del `frame-src` de la aplicación (`App::csp`). Los formularios también aceptan código `<iframe>` pegado y guardan solo su dirección canónica, sin parámetros de seguimiento. Consulta [Display](display.md#embed) |
| `min = n`, `max = n` | números, dinero, decimales, fechas | Límites. Las fechas se escriben `2026_01_01` |
| `scale = n`, `precision = n` | `Decimal` | Máximo de decimales (2 sin la regla) y máximo de dígitos en total (18 sin la regla). También fijan el tipo de columna, `NUMERIC(precision, scale)` |
| `mask = "..."` | texto | El valor sin formato rellena un patrón: `9` un dígito, `a` una letra, `A` una letra mayúscula, `*` cualquiera de ellas; los demás caracteres se muestran al escribir pero no se guardan. Consulta [Controls](controls.md#masked-input) |
| `unique` | cualquiera | Lo comprueba el store; el formulario muestra el error de la acción |

Una regla que no encaja con el tipo del campo es un error de compilación con un mensaje que dice qué hacer. Las reglas entre campos van en una función:

```rust
#[model(validate = check_stay)]
pub struct Stay {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn check_stay(s: &Stay, errors: &mut ValidationErrors) {
    if let (Some(a), Some(b)) = (s.from, s.to) {
        if b < a {
            errors.add(Stay::to, t!("stay.before_start"));
        }
    }
}
```

## Valid

Una acción que guarda recibe `Valid<M>`. Un valor `Valid` solo se puede crear ejecutando todas las reglas, así que una acción no puede olvidarse de validar:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## Tipos de campo

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, enums `#[choice]`, ids de entidad y `Option<T>` de cualquiera de ellos. Un `Vec<M>` de un modelo se convierte en un editor de líneas en los formularios (consulta [Editor de líneas](lines.md)).

Money se guarda en unidades menores enteras con su moneda. Nunca uses `f64` para dinero. `Decimal` es un número exacto para todo lo demás que tenga fracción: cantidades, precios unitarios con más de dos decimales, tasas. Admite hasta 38 dígitos, nunca pasa por un float y viaja en JSON como cadena (`"12.50"`). Las fechas y las horas se tratan en [Hora e idiomas](time-and-languages.md).
