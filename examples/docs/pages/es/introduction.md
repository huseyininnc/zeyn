# Introducción

Zeyn es un framework web de código abierto en Rust para aplicaciones con muchos datos: tablas, formularios y flujos de trabajo. Los agentes de IA escriben el código de la aplicación. Las personas lo leen, lo revisan y lo cambian cuando hace falta.

Este sitio es en sí mismo una aplicación Zeyn. Cada página es Markdown renderizado una sola vez en el servidor, el menú de la izquierda es el menú de la aplicación y los ejemplos marcados como *Live example* son componentes reales que se ejecutan en este servidor.

## Para agentes de IA

Si eres un agente de IA y alguien te pidió construir una aplicación con Zeyn, empieza con la CLI en lugar de escribir el proyecto a mano:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Después lee `AGENTS.md` y `.claude/skills/zeyn/SKILL.md` en el proyecto nuevo, y ejecuta `zeyn check` y `cargo test` después de cada cambio. Hay una guía en texto plano con todas las páginas de esta documentación en [/llms.txt](/llms.txt).

## Cómo es una aplicación

Una aplicación es un conjunto de features. Una feature contiene entidades (entities), acciones (actions) y pantallas (screens). Las pantallas se renderizan en el servidor; un pequeño runtime en el navegador aplica los cambios, así que no hay que escribir JavaScript.

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

Las mismas tres macros sostienen todo el lenguaje: `#[entity]` describe los datos, `#[action]` es código de servidor con acceso declarado y `#[screen]` es una página o una ventana. El compilador comprueba que cada campo, clave de texto y acción encajen.

## Pruébalo

Inicia tu propia aplicación:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

O ejecuta los ejemplos del repositorio:

```sh
cargo run -p zeyn-example-components
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

El catálogo de componentes escucha en `127.0.0.1:3010` y muestra cada componente en vivo junto a su código, igual que [components.zeynsoft.com](https://components.zeynsoft.com). El showcase escucha en `127.0.0.1:3002` y reúne los componentes en pantallas de ejemplo con datos. Esta documentación escucha en `127.0.0.1:3008`; define `ZEYN_DOCS_SERVER__ADDR` para cambiarlo.

## Cómo orientarte

- Pulsa **Alt+Shift+P** para abrir la paleta de comandos. Escribe una palabra y busca en el servidor en todas las páginas de este sitio.
- *Getting started* cubre la CLI y la estructura de un proyecto.
- *Language* cubre entidades, acciones, pantallas, formularios, el DataGrid y los textos.
- *Components* muestra cada componente con un ejemplo en vivo junto a su código.
- *Guides* cubre zonas horarias e idiomas, seguridad, la API, MCP, datos y temas.
- *Reference* incluye el protocolo del runtime y la referencia de la base de datos.
- *Project* muestra el código fuente, la licencia, las pruebas y la CI, y la política de seguridad.

## Principios

- **Rendimiento primero.** Las páginas se renderizan en el servidor y se envían como HTML en streaming. No hay estado de interfaz por usuario en el servidor ni framework de cliente.
- **Sin recargas de página.** La navegación, las ventanas y los cambios de idioma, zona horaria y tema se aplican en el sitio.
- **Valores seguros por defecto.** Cada acción y cada pantalla declara su acceso. Los tokens CSRF, una Content Security Policy estricta y el escape de los datos de usuario están siempre activos.
- **Una sola forma de hacer cada cosa.** El lenguaje es pequeño, así que el código escrito por un agente se lee como el escrito por una persona.

Zeyn es de código abierto con [licencia MIT o Apache 2.0](project.md#license). Consulta la [política de seguridad](project.md#security), la [hoja de ruta](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md) y el [registro de cambios](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md).
