# Seguridad

La seguridad en Zeyn se declara junto al código que protege y se comprueba en el servidor en cada llamada. Los valores por defecto son estrictos; relajar uno es una línea visible en `app.rs`.

## Permisos

Cada acción y cada pantalla declara `permission = "feature.resource.verb"` o `public`. Sin una de las dos no compila. La identidad de una petición lleva sus permisos:

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

Los elementos de menú, los botones de la barra de herramientas, las acciones de fila y los comandos que el usuario no puede ejecutar se omiten del HTML. Omitirlos es una comodidad; el servidor vuelve a comprobar el permiso en cada llamada.

## Inicio de sesión

La aplicación no guarda contraseñas. Un `Authenticator` averigua quién llama, normalmente a partir de un proxy de inicio de sesión único situado delante de la aplicación:

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

Usa `ForwardAuth` solo cuando todas las peticiones pasen por el proxy; cualquiera que pueda llegar directamente a la aplicación puede enviar estas cabeceras. `require_header` comprueba un secreto compartido que añade el proxy. `DevAuth::superuser(name)` da acceso total a todas las peticiones para el desarrollo local, y el servidor imprime una advertencia cuando está activo. Una compilación de release se niega a arrancar con él (`App::check` lo informa); una demostración que deba ejecutarse en release puede llamar a `App::allow_dev_auth()`. Las aplicaciones creadas con `zeyn new` lo activan solo en compilaciones de depuración.

## Guards

Un guard se ejecuta antes de que se abra cualquier pantalla de una feature y antes de que se llame a cualquiera de sus tablas, formularios o comandos:

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

Guards incluidos: `SignedIn`, `TenantRequired` y `Licensed`.

## Interceptores

Un interceptor se ejecuta alrededor de cada llamada a pantalla, tabla, formulario, API y comando después de las comprobaciones de acceso. `before` puede detener una llamada; `after` ve cómo terminó. `AuditLog(sink)` es el incluido que registra quién hizo qué.

## CSRF, CSP y cabeceras

- Cada página establece una cookie `zeyn_csrf` aleatoria (`HttpOnly`, `SameSite=Strict`) y pone el mismo token en una etiqueta meta. Cada llamada del runtime lo envía en la cabecera `x-zeyn-csrf` y el servidor compara los dos en tiempo constante.
- Las pantallas públicas abiertas sin sesión son la excepción: no establecen cookie ni llevan token, así que se pueden guardar en caché. El runtime obtiene el token de `GET /_zeyn/csrf` antes de su primera llamada, y cada `POST` se sigue comprobando de la misma manera (consulta [Páginas públicas](public-pages.md)).
- La Content Security Policy permite scripts, estilos e imágenes solo del propio origen de la aplicación. No hay scripts en línea; los temas son hojas de estilo, nunca estilos en línea.
- `App::csp` abre la política para hosts con nombre, una directiva cada vez. La cabecera se construye una sola vez al arrancar.

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  Solo se aceptan `https://host`, `https://host:port` y `https://*.host` (un comodín coincide con subdominios, no con el propio host). `.media_blob()` añade `blob:` a `media-src` para las bibliotecas de streaming. `App::check` rechaza `*`, `http:`, esquemas sueltos como `data:` o `https:`, palabras clave entre comillas como `'unsafe-inline'` o `'unsafe-eval'`, rutas, nombres de usuario y valores con espacios, comas o puntos y comas; cada problema dice qué escribir en su lugar. `script-src`, `object-src`, `base-uri` y `frame-ancestors` no se pueden cambiar. `Embed` e `Image` leen las mismas listas, así que un host que el navegador bloquearía nunca se renderiza (consulta [Display](display.md)).
- Las respuestas llevan `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin` y `X-Frame-Options: DENY`.
- `App::secure_cookies(true)` marca las cookies como `Secure`; actívalo detrás de HTTPS.

## Escape

Los datos de usuario en HTML siempre pasan por `render::escape`. Los componentes escapan cada valor que renderizan, así que un producto llamado `<script>` se muestra como texto.

## Límites

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit` se aplica a todas las peticiones excepto los recursos estáticos y las lecturas de fuentes de medios públicas (`Feature::public_media`); `write_limit` añade uno más estricto para las llamadas que cambian datos: envíos de formularios, ediciones de tabla, acciones de fila, masivas y de menú, comandos, importaciones, vistas de tabla guardadas, diseños de dock y temas, llamadas a la API, llamadas MCP y el asistente. Las peticiones rechazadas reciben `429` con `Retry-After`.
- `X-Forwarded-For` solo se cree si viene de `trusted_proxies`. Cada línea de la cabecera cuenta, en orden, y la dirección que se toma es la última que no es un proxy de confianza.
- Las revelaciones y las llamadas de islands y de fuentes de datos también cuentan como escrituras, y lo mismo las exportaciones y los documentos impresos (`GET /_zeyn/grid/export`, `GET /_zeyn/pdf`).
- Los tamaños de página, el número de líneas, la longitud de las consultas y el tamaño de las subidas están limitados en el servidor envíe lo que envíe el cliente.

## Sesiones

`App::idle_timeout(d)` cierra la sesión de los usuarios en la página tras un tiempo sin actividad, con un diálogo de aviso antes. Las acciones marcadas como `reauth` necesitan un inicio de sesión reciente (`App::reauth_within`).

## Protección de datos

El enmascarado de campos, los motivos antes de abrir registros sensibles, la auditoría a nivel de campo y la auditoría de lectura se tratan en [Auditoría, enmascarado y jobs](enterprise.md).

## La API y los agentes

Las llamadas a la API necesitan una cabecera `Authorization`; una cookie de sesión del navegador por sí sola nunca llama a la API ni al endpoint MCP, así que no necesitan token CSRF. Los agentes reciben exactamente los permisos del usuario en cuyo nombre actúan. Consulta [API y OpenAPI](api.md) y [MCP y agentes de IA](mcp.md).
