# Apps y bibliotecas compartidas en repositorios separados

Zeyn funciona en un único repositorio, pero muchos equipos mantienen cada app en su propio repositorio y colocan
el código compartido (entities, servicios, features completas) en bibliotecas que usan varias apps. Esta guía
muestra cómo configurarlo y la única regla que lo mantiene funcionando.

## Qué va dónde

| Repositorio | Creado con | Contiene |
|---|---|---|
| App | `zeyn new billing` | `main.rs`, `app.rs`, features de la app, configuración, catálogos de la app |
| Biblioteca compartida | `zeyn new acme-domain --lib` | entities, servicios, features, sus catálogos, `CATALOGS` |

Una feature de biblioteca es igual que una feature de app: los models, las actions, las pantallas, los permisos y
las traducciones viven juntos en una carpeta. La app decide qué features de biblioteca monta.

```rust
let i18n = I18n::new("en")
    .catalogs(acme_domain::CATALOGS)?
    .catalog("en", include_str!("../locales/en.json"))?
    .catalog("tr", include_str!("../locales/tr.json"))?;

App::new(t!("app.name"))
    .i18n(i18n)
    .service(acme_domain::customers::CustomerStore::default())
    .feature(acme_domain::customers::feature())
```

Los catálogos de la biblioteca se cargan primero, de modo que una app puede sobrescribir un texto de la biblioteca definiendo la misma clave.
Antepón a las claves de la biblioteca el nombre de la feature (`customers.list`) para que dos bibliotecas nunca colisionen.

`examples/shared-domain` es una biblioteca de este tipo. `examples/billing` y `examples/crm` montan ambos su
feature customers; crm renombra "Customers" a "Accounts" en su propio catálogo.

## Qué tiene que poseer la biblioteca

La regla de huérfanos de Rust dice que un trait solo puede implementarse en el crate que define el trait o
el tipo. Una app no puede implementar `zeyn::Model` ni `zeyn::Entity` para un tipo de una biblioteca, por lo que:

- Pon `#[entity]`, `#[model]` y `#[choice]` en el tipo de la biblioteca, junto a sus reglas.
- Mantén las reglas de validación (`#[rule(...)]`) en el tipo compartido. Así todas las apps validan de la misma manera.
- Una app que necesita un comportamiento adicional envuelve el tipo (`struct Invoice { customer: Customer, ... }`)
  o añade su propio trait. No edita el tipo de la biblioteca.

## Cómo obtienen las apps la biblioteca

| Opción | Ventajas | A tener en cuenta |
|---|---|---|
| Dependencia Git con una etiqueta (tag) | Nada que ejecutar, funciona con cualquier servidor Git | Cargo descarga el repositorio completo; cada app lista la URL |
| Registro privado de Cargo (Kellnr, registro de paquetes de GitLab, otros) | Dependencias normales `version = "0.3"`, resolución semver | Un servicio que ejecutar, respaldar y proteger |
| Un único repositorio workspace para todo | Una compilación, un `Cargo.lock`, refactorizaciones en un solo commit | Cada equipo ve todas las apps; el control de acceso es por repositorio, no por carpeta |

Empieza con etiquetas Git. Pasa a un registro cuando tengas más de un puñado de bibliotecas o cuando
la resolución de versiones a mano se vuelva tediosa. El código de la app no cambia; solo cambian las líneas de `Cargo.toml`.

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new` escribe `zeyn = "<version>"` de forma predeterminada. Hasta que Zeyn se publique, cambia esa línea a la
forma Git de arriba en cada app y biblioteca nuevas.

## La regla: una sola versión de Zeyn en todas partes

Cada biblioteca y cada app deben depender exactamente del mismo Zeyn. Con dependencias Git eso significa
la misma URL y la misma etiqueta.

Cargo trata dos fuentes distintas de Zeyn como dos crates distintos. Una biblioteca compilada contra la etiqueta
`v0.0.2` devuelve un `zeyn::Feature` que una app compilada contra `v0.0.3` no puede aceptar, y el compilador
informa de una discrepancia de tipos entre dos cosas con el mismo nombre. Un registro relaja esto a "la misma
versión compatible según semver", pero la regla se mantiene: un Zeyn por compilación.

Para actualizar Zeyn:

1. Actualiza la etiqueta en cada biblioteca, ejecuta sus pruebas, publica una nueva etiqueta de biblioteca.
2. Actualiza Zeyn y las etiquetas de las bibliotecas en cada app a la vez, en un solo commit.

`cargo tree -d -i zeyn` lista los duplicados. Si muestra más de un Zeyn, alguna dependencia sigue apuntando
a una etiqueta antigua.

## Paso a paso

1. Crea la biblioteca y súbela a su propio repositorio.

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. Añádela al `Cargo.toml` de la app con la etiqueta, y monta sus catálogos, servicios y features como se
   muestra arriba.

3. Permite que el CI de la app lea la biblioteca. El `.gitlab-ci.yml` generado reescribe las URL
   `https://<gitlab host>/` y `ssh://git@<gitlab host>/` para usar el job token. En GitLab, abre
   el proyecto de la biblioteca, luego los ajustes de CI/CD, Job token permissions, y añade el proyecto de la app a la
   lista de permitidos.

4. Ejecuta `app.check()` en una prueba de la app (el `tests/app.rs` generado lo hace). Falla cuando una feature
   montada tiene problemas estructurales, por ejemplo una clave de traducción que falta en un idioma.

## Trabajar en una biblioteca y una app a la vez

Apunta la app a tu copia local sin editar `Cargo.toml`. Pon esto en el
`.cargo/config.toml` de la app y mantén ese archivo fuera de Git:

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

Elimínalo, etiqueta la biblioteca y sube la etiqueta en la app cuando el cambio esté terminado.

## Publicar una biblioteca

1. Sube `version` en `Cargo.toml`.
2. Ejecuta `cargo semver-checks` si está instalado. Informa de los cambios de API pública que rompen a quienes la llaman.
   Un cambio incompatible requiere una nueva versión minor mientras la biblioteca esté por debajo de 1.0.
3. Haz commit, etiqueta `v<version>` y sube la etiqueta. El trabajo de CI `tag-matches-version` falla cuando la etiqueta y
   `Cargo.toml` no coinciden.
