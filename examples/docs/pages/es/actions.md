# Acciones

Una acción es código de servidor al que puede llamar una pantalla, una tabla, un formulario, un comando, la API JSON o un agente. `#[action]` declara quién puede llamarla; el framework lo comprueba antes de que se ejecute el código.

## Declarar una acción

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- El primer argumento es la entrada. Viene del cliente, así que el framework la lee como JSON no confiable.
- Todos los demás argumentos se inyectan: `Ctx` o un `Service` registrado.
- El tipo de retorno es `Result<T>`.

## Acceso

Cada acción declara exactamente una de estas opciones:

| Opción | Significado |
|---|---|
| `permission = "feature.resource.verb"` | La identidad necesita este permiso |
| `public` | Cualquiera, incluidos los usuarios que no han iniciado sesión |

Una acción sin acceso no compila. Los nombres de permiso van en minúsculas y separados por puntos, como `sales.order.write`.

Otras opciones:

| Opción | Significado |
|---|---|
| `audit` | Escribe un registro de auditoría en cada llamada |
| `reauth` | El usuario debe haber iniciado sesión hace poco (`App::reauth_within`) |

## Errores

| Error | Estado | Uso |
|---|---|---|
| `Error::NotFound` | 404 | El registro no existe o el usuario no debe saber que existe |
| `Error::Forbidden` | 403 | Acceso denegado |
| `Error::Denied(t!("..."))` | 403 | Denegado con un motivo que el usuario puede leer |
| `Error::Conflict` | 409 | La versión de la fila cambió desde que se cargó el formulario |
| `Error::Validation(errors)` | 422 | Errores de campo, mostrados junto a los campos |
| `Error::Internal("detail")` | 500 | El detalle va solo al registro del servidor |

Un error de campo desde código:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Listas

Las listas reciben un `DataQuery` y devuelven `Page<M>`. Una acción de lista nunca devuelve todas las filas; la tabla pide páginas y el servidor limita el tamaño de página sea lo que sea que envíe el cliente.

Para una tabla que la aplicación calcula o guarda en su propio índice (millones de filas, eventos construidos a partir de sus ids), `q.span()` da las filas pedidas como un desplazamiento y un límite, tanto si vienen de páginas numeradas como de un cursor, y `Page::window(rows, total, span)` las devuelve con el total y el cursor a la fila siguiente. Solo se construyen las filas de la página; el orden y los filtros los aplica la aplicación leyendo `q.sort` y `q.filters`.

```rust
#[action(permission = "audit.read")]
pub async fn search_events(q: DataQuery, log: EventLog) -> Result<Page<Event>> {
    let span = q.span();
    let rows = log.range(span.offset, span.limit)?;
    Ok(Page::window(rows, log.len()?, span))
}
```

## El contexto

`Ctx` es la petición: `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()` y `ctx.t(&text)`. Usa `ctx.now()` y `ctx.today()` en lugar del reloj del sistema, para que las pruebas puedan fijar la hora con `App::fixed_now`.

## Dónde se usan las acciones

- `DataGrid::of(action)` carga filas; `.edit_with(action)`, `.row_action(...)` y `.bulk_action(...)` las cambian.
- `Form::submit(label, action)` guarda un formulario.
- `ActionButton::new(label, action, input)` ejecuta una acción desde un botón.
- `Command::action(...)` ejecuta una desde la paleta de comandos o un atajo.
- `Feature::api(action)` la publica como JSON (consulta [API y OpenAPI](api.md)).
- `Feature::search(action)` añade una búsqueda de servidor a la paleta de comandos.

Todas las rutas ejecutan las mismas comprobaciones: CSRF para las llamadas del navegador, los guards de la feature, el acceso de la acción y después los interceptores.
