# Páginas públicas

Las pantallas marcadas como `public` pueden abrirlas todos, también los visitantes que nunca iniciaron sesión: una
página de inicio, una lista de precios, un centro de ayuda. Zeyn las sirve de modo que sean baratas de entregar y fáciles de
manejar para las cachés y los buscadores.

## Sin cookies para los visitantes

Una pantalla pública abierta por una petición sin sesión no establece ninguna cookie. La página lleva un token
CSRF vacío, así que todos los visitantes con el mismo idioma y tema reciben los mismos bytes.

El runtime del navegador pide un token a `GET /_zeyn/csrf` justo antes de la primera llamada que lo necesita
(un formulario, una acción, el selector de idioma). Esa llamada establece la cookie `zeyn_csrf` y nunca se
guarda en caché. Cada `POST` se comprueba exactamente igual que en cualquier otra página: sin un token que coincida se
rechaza con `403`, y no se reintenta nada en silencio.

Los usuarios con sesión iniciada siempre reciben la cookie y el token en la página, como antes.

## Caché HTTP

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N` permite que los navegadores y las cachés compartidas guarden la página durante `N` segundos (de 1 a 86400). Para una
petición sin sesión, la página recibe entonces:

| Cabecera | Valor |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | Una etiqueta fuerte sobre el cuerpo renderizado |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

Una petición con un `If-None-Match` que coincida recibe `304 Not Modified` sin cuerpo. Las pantallas cargadas por
navegación dentro de la aplicación se guardan en caché de la misma manera, con su propia etiqueta.

Una petición con sesión nunca recibe cabeceras de caché públicas: la página se envía con
`Cache-Control: private, no-store`. `cache` en una pantalla que no es `public` no compila.

La pantalla se sigue construyendo y renderizando para cada petición que llega al servidor; la etiqueta ahorra la
transferencia y una caché delante de la aplicación ahorra la petición. Mantén las pantallas en caché libres de cualquier cosa
que dependa del visitante: la hora del día, un contador, una elección al azar.

## Prefetch

`App::prefetch(Prefetch::Moderate)` carga la pantalla siguiente antes del clic; está desactivado por defecto. `nav.js` obtiene la pantalla parcial de un enlace tras 100 ms de hover, con el foco del teclado o al pulsarlo, la conserva 10 segundos, ejecuta como máximo dos de estas obtenciones a la vez y cancela una cuando el puntero se va antes. Solo se precargan los enlaces a pantallas de la aplicación con ruta de path, nunca `/_zeyn/*`, los enlaces de inicio y cierre de sesión, las descargas, los enlaces con `target` ni las pantallas marcadas con `#[screen(no_prefetch)]`, que el servidor rechaza antes de ejecutarlas. No se precarga nada con Save-Data, `prefers-reduced-data`, una conexión 2g o en una pestaña oculta. Donde falta el script, Chromium recibe Speculation Rules (solo prefetch, nunca prerender) mediante una cabecera `Speculation-Rules`, de modo que la política de seguridad de contenido no cambia. El servidor marca estas peticiones: `ctx.is_prefetch()` es true, así que una pantalla puede omitir el recuento de una visita. Las reglas de caché pública se aplican como siempre.

## Buscadores y vistas previas de enlaces

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new(t!("pricing.title"))
        .description(t!("pricing.description"))
        .canonical("/pricing")
        .og_image("https://cdn.example.com/pricing.png")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Team plan",
        }))
}
```

| Método | Etiquetas de head |
|---|---|
| `.description(text)` | `<meta name="description">` y `og:description`, en el idioma del usuario |
| `.canonical(url)` | `<link rel="canonical">` y `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` o `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

Todos los valores se escapan. JSON-LD se escribe con `<`, `>` y `&` como `<`, `>` y `&`,
de modo que un valor que contenga `</script>` no pueda terminar el elemento; el navegador nunca lo ejecuta y la política
de seguridad de contenido sigue siendo tan estricta como en cualquier otra página. Las rutas de la aplicación en propiedades de dirección de JSON-LD (`url`, `@id`, `image`, `logo`, `item` y similares) se escriben absolutas con `App::site_url`, a cualquier profundidad, como esperan los buscadores.

Las URL deben ser URL `https` o rutas de la aplicación que empiecen por `/`. Cualquier otra cosa (`http:`, `//host`,
`javascript:`, espacios, comillas) se omite con una advertencia en el registro. Las rutas de la aplicación se escriben detrás
de la URL del sitio y de la ruta base:

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

Sin `site_url`, las etiquetas de head conservan la ruta de la aplicación, que los navegadores resuelven pero algunos rastreadores no.

La navegación dentro de la aplicación actualiza el título y todas estas etiquetas sin cargar la página.

## Sitemap

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml` pide a la acción 1.000 URL cada vez (`part.offset`, `part.limit`) y envía el
XML en streaming, de modo que un sitio grande nunca se queda en memoria. Hasta 50.000 URL es un único `urlset`. Por encima es un
índice de sitemaps de `/sitemaps/1.xml`, `/sitemaps/2.xml` y así sucesivamente, cada uno con 50.000 URL. La acción
debe ser `public` y devolver las URL en un orden estable. Las rutas de la aplicación necesitan `App::site_url`, porque un
sitemap contiene URL absolutas; las entradas que no sean URL `https` ni rutas de la aplicación se omiten. Lista solo las
páginas públicas: los buscadores no pueden abrir las demás. Una feature por aplicación tiene un sitemap.

## robots.txt y llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

Ambos se sirven como texto plano a todo el mundo.

## La carpeta public

```rust
App::new(t!("app.name"))
    .public(zeyn::public_files!())
```

Los archivos de la carpeta `public/` del crate se incrustan en el binario con `zeyn::public_files!()` y se sirven desde la raíz del sitio: `public/favicon.svg` responde en `/favicon.svg`, y una página enlaza como icono el primero que encuentre de `/favicon.svg`, `/favicon.png` y `/favicon.ico`. Se aceptan imágenes, iconos, fuentes, texto, XML, JSON, manifiestos web y PDF; HTML y scripts detienen la compilación, porque se ejecutarían bajo el origen de la app. Los archivos llevan una etiqueta de entidad y se guardan en caché una hora. Un archivo puede tener 256 KB y la carpeta 2 MB, salvo que `max_public_file_kb` y `max_public_total_kb` en `[package.metadata.zeyn]` digan otra cosa. `App::check` informa de un archivo cuya ruta ya usa una pantalla, `/robots.txt`, `/llms.txt`, `/sitemap.xml` o `/_zeyn/`. `zeyn new` añade un `public/favicon.svg`.
