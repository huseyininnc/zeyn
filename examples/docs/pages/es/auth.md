# Autenticación

Quién llama, y qué tan segura está la aplicación de ello. Zeyn tiene cuatro formas de entrada:

| Forma | Úsala para | Feature de Cargo |
| --- | --- | --- |
| `ForwardAuth` | un proxy de inicio de sesión único situado delante de la aplicación establece cabeceras | predeterminada |
| `JwtAuth` | APIs, MCP y llamadas de servicio con `Authorization: Bearer <JWT>` | `oidc` |
| `OidcAuth` | los usuarios del navegador inician sesión en un proveedor OpenID Connect | `oidc` |
| `Totp` | un segundo factor para aplicaciones cuyo inicio de sesión no tiene ninguno | `totp` |
| `App::accounts` | la aplicación guarda sus propios usuarios: contraseñas, doble factor, passkeys (`docs/accounts.md`) | `accounts` |

`RequireMfa` (siempre disponible) pide un segundo factor en una feature, una pantalla o una action.
La compilación predeterminada no incluye ni `oidc` ni `totp`, de modo que las aplicaciones que no los usan no pagan nada.

```toml
zeyn = { version = "0.0.2", features = ["oidc"] }
```

Los proveedores probados son Keycloak y Authentik. Cualquier proveedor que siga el descubrimiento de OpenID Connect,
el flujo de código de autorización con PKCE y JWKS debería funcionar.

## Bearer tokens para APIs y MCP

```rust
use zeyn::{ClaimMap, JwtAuth};

App::new("Stock").auth(
    JwtAuth::new("https://sso.example.com/realms/main", "stock-api")
        .claims(
            ClaimMap::new()
                .roles("realm_access.roles")
                .role("stock-admins", ["stock.product.read", "stock.product.write"]),
        ),
)
```

- El primer argumento es el emisor (issuer) exactamente como aparece en el `iss` de los tokens. Las claves de firma
  provienen del JWKS indicado en `<issuer>/.well-known/openid-configuration`; `jwks_url` establece la
  dirección cuando el emisor no tiene un documento de descubrimiento.
- El segundo argumento es la audiencia que el token debe nombrar en `aud`. En Keycloak, añade un mapper de
  "Audience" al cliente que solicita tokens para la API.
- Solo se acepta `RS256` salvo que `algorithms([...])` indique más (`RS384`, `RS512`, `PS256`,
  `PS384`, `PS512`, `ES256`, `ES384`, `EdDSA`). `none` y todos los algoritmos `HS*` se rechazan
  siempre: no hay forma de permitirlos. Una clave solo se usa para la familia de algoritmos de su tipo,
  de modo que un token "firmado" con HMAC sobre la clave pública se rechaza.
- `exp` es obligatorio. `exp`, `nbf` e `iat` tienen un minuto de margen (`leeway(...)`, como máximo
  cinco minutos).
- Las claves se guardan en caché durante una hora (`key_ttl`). Un token que nombra un `kid` desconocido vuelve a obtener las claves,
  como máximo una vez cada 30 segundos (`min_key_refresh`), de modo que los tokens con ids de clave inventados no pueden
  inundar al proveedor. Se conservan como máximo 16 claves y se rechazan las respuestas de más de 256 KB. Una
  obtención fallida conserva las claves antiguas.
- Un token rechazado simplemente no inicia sesión; el motivo se registra solo en el nivel debug.

### Tenants

Cada autenticador rellena los mismos dos campos de `Identity`: `tenant` (el tenant propio del usuario) y
`tenants` (todos los demás tenants en los que el usuario puede trabajar). `Identity::member_of(id)` es verdadero para cualquiera de los dos;
un superusuario (`DevAuth`) pertenece a todos los tenants.

| Autenticador | De dónde provienen los tenants |
| --- | --- |
| `JwtAuth`, `OidcAuth` | el claim indicado con `ClaimMap::tenant`, una cadena o una lista |
| `accounts` | los claims del usuario de tipo `tenant` (`Claim::new("tenant", "acme")`), en el orden en que se añadieron |
| `ForwardAuth` | la cabecera indicada con `ForwardAuth::tenant`, un tenant |
| tu propio `Authenticator` | establece `identity.tenant` e `identity.tenants` |

Con `App::tenancy(resolver)` el tenant de una petición proviene de la petición (host o cabecera).
Un usuario con sesión iniciada debe pertenecer a él; en caso contrario la petición se ejecuta sin sesión, de modo que las pantallas públicas
siguen funcionando y todo lo demás responde como para un usuario anónimo. Un usuario con sesión iniciada sin ningún
tenant no pertenece a ninguno. Cuando el resolver no encuentra ningún tenant, se aplica el tenant propio del usuario.

### Claims

`ClaimMap` convierte los claims en la `Identity`:

| Método | Valor por defecto | Notas |
| --- | --- | --- |
| `user("email")` | `preferred_username`, y si no `sub` | un token sin él se rechaza |
| `tenant("tenant")` | ninguno | el tenant del usuario; una lista (array o separada por espacios) nombra todos los tenants a los que pertenece el usuario, el primero es el propio. Véase "Tenants" más arriba |
| `roles("realm_access.roles")` | ninguno | path con puntos; roles de realm de Keycloak, o `groups` en Authentik |
| `role("clerks", [...])` | ninguno | permisos para los titulares de un rol o miembros de un grupo |
| `permissions("scope")` | ninguno | claim cuyos valores son permisos; una lista o una cadena separada por espacios |
| `time_zone("zoneinfo")` | `zoneinfo` | la zona del usuario |
| `mfa_acr(["2"])` | ninguno | valores de `acr` que significan un segundo factor; véase más abajo |

`OidcAuth` lee los claims del ID token. Keycloak coloca los roles de realm solo en el access token
salvo que el mapper de "realm roles" del cliente tenga activado "Add to ID token"; actívalo (o añade un
`oidc-usermodel-realm-role-mapper` con `id.token.claim`). Authentik coloca `groups` en el ID
token con el scope `profile`.

`auth_time` (o `iat` en su defecto) se convierte en `Identity::authenticated_at`, que leen las actions con `reauth`.

## Inicio de sesión en el navegador

```rust
use zeyn::{ClaimMap, OidcAuth};

let oidc = OidcAuth::new(
    "https://sso.example.com/realms/main",
    "stock-web",
    "https://stock.example.com",
    config.secret("auth.session_key")?,
)
.client_secret(config.secret("auth.client_secret")?)
.claims(ClaimMap::new().roles("realm_access.roles").role("stock-admins", ["stock.product.write"]));

App::new("Stock").oidc(oidc)
```

`App::oidc` lo convierte en el autenticador, el `login_url`, la dirección de cierre de sesión por inactividad y la
dirección de reautenticación. Añade tres rutas:

| Ruta | Qué hace |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | inicia un inicio de sesión y redirige al proveedor |
| `GET /_zeyn/auth/callback` | la redirección de vuelta del proveedor |
| `GET /_zeyn/auth/logout` | termina la sesión aquí y en el proveedor |

En el proveedor, registra `<public url>/_zeyn/auth/callback` como URI de redirección y
`<public url>/` como URI de redirección tras el cierre de sesión. La URL pública se configura, nunca se toma
de la cabecera `Host` de la petición. Con un base path o un path público las rutas pasan a colgar de él.

### El flujo

1. Un usuario sin sesión abre una página que necesita un permiso y va a `/_zeyn/auth/login`.
2. El servidor crea un nuevo `state`, `nonce` y verificador PKCE (32 bytes aleatorios cada uno), los sella
   junto con el path `return_to` en una cookie que vive diez minutos, y redirige al
   endpoint de autorización del proveedor con `code_challenge_method=S256`.
3. El proveedor devuelve al usuario con un código y el `state`. El servidor abre la cookie,
   compara `state` en tiempo constante, elimina la cookie e intercambia el código con el verificador
   PKCE (el secreto del cliente mediante autenticación HTTP básica cuando está definido).
4. La firma del ID token se comprueba contra el JWKS del proveedor, y luego `iss`, `aud`, `azp`
   cuando hay varias audiencias, `exp`, `nbf`, `iat` y el `nonce`.
5. Se establece la cookie de sesión y el usuario vuelve a `return_to`. Solo se acepta un path local;
   cualquier otra cosa va a la página de inicio de la aplicación.

### La cookie de sesión

No hay almacén de sesiones en el servidor. La sesión vive en una sola cookie, `zeyn_session`:

- Cifrada y autenticada con AES-256-GCM. La clave se deriva de la clave de sesión
  (`Config::secret`, de al menos 32 bytes; una más corta detiene la aplicación al arrancar). El nombre de la cookie
  queda ligado, de modo que un valor sellado para una cookie nunca se abre como otra. Un byte cambiado, o una cookie
  sellada con otra clave, simplemente no inicia sesión.
- Contiene el usuario, el tenant, la zona horaria, los roles que `ClaimMap` asigna, los permisos directos,
  la hora de inicio de sesión, si se usó un segundo factor, el fin de la sesión y, con refresh tokens,
  el refresh token. No el ID token ni el access token.
- `HttpOnly`, `SameSite=Lax` (la redirección de vuelta del proveedor es una navegación entre sitios) y
  `Secure` cuando la URL pública es `https://` (o `App::secure_cookies(true)`).
- Una sesión que superaría los 3800 bytes se rechaza al iniciar sesión con un mensaje en el log, nunca
  se recorta. Asigna menos roles o desactiva los refresh tokens.
- Dura `session_lifetime` (ocho horas salvo que se defina otro valor). Cambiar la clave de sesión cierra la sesión de todos.

La protección CSRF no cambia: cada `POST` del runtime sigue necesitando el token `zeyn_csrf` en
la cabecera `x-zeyn-csrf`, sea cual sea la cookie que inicia la sesión del usuario.

### Refresh tokens

`refresh_tokens(true)` conserva el refresh token en la cookie. Cuando los tokens caducan, la siguiente
petición los refresca antes de ejecutarse y devuelve la cookie renovada. Un refresco rechazado (el
usuario fue deshabilitado o cerró sesión en el proveedor) termina la sesión. Sin refresh tokens una
sesión dura `session_lifetime`, pase lo que pase en el proveedor. El refresco se ejecuta como una capa
sobre toda la aplicación, de modo que cada ruta queda cubierta, también las de las features opcionales.

Si el proveedor rota los refresh tokens y revoca el antiguo de inmediato, dos peticiones que refrescan
en el mismo momento pueden cerrar la sesión del usuario. Mantén la rotación desactivada o permite la reutilización durante un breve tiempo.

### Cierre de sesión

`/_zeyn/auth/logout` elimina la cookie de sesión y envía al usuario al
`end_session_endpoint` del proveedor con `client_id` y `post_logout_redirect_uri`. Sin ese endpoint
el usuario va a la página de inicio de la aplicación.

### Clientes de API y MCP de la misma aplicación

`OidcAuth::accept_bearer("stock-api")` también acepta access tokens bearer del mismo emisor para
esa audiencia, con la misma asignación de claims. Una petición con una cabecera `Authorization: Bearer` se
juzga solo por su token.

## Segundo factor y step up

`Identity::mfa` es verdadero cuando el usuario demostró un segundo factor:

- con `OidcAuth` y `JwtAuth`, cuando `amr` nombra `mfa`, `otp`, `hwk`, `swk`, `sc`, `sms`, un
  dato biométrico u otro segundo factor (RFC 8176), o cuando `acr` es uno de `ClaimMap::mfa_acr`;
- con `Totp`, tras un código verificado.

`RequireMfa` lo exige:

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

Como guard de feature cubre toda la feature. Como interceptor cubre las pantallas y actions indicadas,
o todas las llamadas cuando ambas listas están vacías.

Cuando falta el factor, una carga de página se redirige a la dirección de step up con `return_to`.
Las demás llamadas responden `401` con el código `mfa_required` (tipo de problema `urn:zeyn:problem:mfa_required`
en la API); el runtime abre la página de step up en una pestaña nueva y repite la petición cuando el
usuario vuelve, como hace con `reauth_required`.

Con `OidcAuth` la dirección de step up es `/_zeyn/auth/login?mfa=1`. Envía `acr_values` (los
valores de `mfa_acr`) con `prompt=login` y `max_age=0`. Si el nuevo ID token sigue sin mostrar un segundo
factor, el callback lo rechaza en lugar de entrar en un bucle.

- **Keycloak**: da al flujo del navegador niveles de autenticación (un paso condicional de OTP o WebAuthn
  en el nivel 2) y usa `mfa_acr(["2"])`. Keycloak responde `acr` con el nivel alcanzado, o
  con su nombre cuando el cliente asigna nombres a niveles (`acr.loa.map`); en ese caso enumera esos nombres.
- **Authentik**: Authentik informa de cómo inició sesión el usuario mediante `amr` cuando el flujo usó
  esas etapas. Usa un flujo con una etapa de validación de autenticador para la aplicación.

Las actions con `reauth` bajo `OidcAuth` van a `/_zeyn/auth/login?reauth=1`, que envía
`prompt=login` y `max_age=0`, de modo que el nuevo `auth_time` sea reciente.

## TOTP integrado

Para aplicaciones cuyo inicio de sesión no tiene segundo factor (un proxy sin MFA, un autenticador
específico de la aplicación):

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp` muestra el registro a un usuario sin segundo factor: un código QR dibujado como SVG
  en el servidor (sin script, sin petición de imagen), la clave para escribir a mano y un campo de código. El
  secreto espera en una cookie sellada durante diez minutos hasta que un código demuestra que la aplicación lo tiene.
- Tras el registro, el usuario ve diez códigos de recuperación una sola vez. El almacén guarda solo sus hashes
  con clave (HMAC-SHA256 con una clave derivada de la clave TOTP), de modo que una copia del almacén por sí sola no puede
  adivinarse sin conexión. Cada uno funciona una vez.
- El secreto almacenado se sella con la clave TOTP y se liga al usuario, de modo que el almacén por sí solo no puede
  generar códigos y los registros no pueden intercambiarse entre usuarios.
- Los usuarios registrados verifican allí con un código o un código de recuperación. Un código se acepta una vez (se
  almacena el último intervalo de tiempo usado) con un intervalo de desfase de reloj en cada sentido.
- Los intentos se limitan por usuario (cinco cada cinco minutos salvo que `attempts` indique otra cosa) mediante
  el almacén de límites de la aplicación, de modo que varios servidores comparten el recuento con `RedisStore`.
- Un código verificado establece una cookie sellada `zeyn_mfa` durante `lifetime` (doce horas salvo que se defina otro valor) que
  marca la identidad con `mfa` y cuenta como un inicio de sesión reciente para las actions con `reauth`.
- Un factor registrado no puede reemplazarse solo desde una sesión; restablecerlo es una
  tarea de administrador en el almacén.
- Los formularios se envían al mismo origen con el token `zeyn_csrf` como campo del formulario.

`TotpStore` son dos métodos asíncronos, `load(user)` y `save(user, record)`; `MemoryTotpStore` es para
pruebas. Mantén estable la clave TOTP: los registros sellados con otra clave dejan de abrirse.

## Pruebas

`crates/zeyn/tests/oidc.rs` ejecuta cada flujo contra un pequeño emisor dentro de la prueba que firma con una
clave creada para la ejecución, y comprueba que se rechazan los tokens caducados, las audiencias y emisores incorrectos, `alg=none`, un
token HMAC con la clave pública como clave, los claims modificados, las cookies manipuladas, un `state` reutilizado o que no coincide
y un `nonce` incorrecto. `crates/zeyn/tests/totp.rs` cubre el registro,
la verificación, la repetición, los códigos de recuperación y el límite de frecuencia.

Con un Keycloak en modo de desarrollo, los mismos flujos también se ejecutan contra él:

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

La prueba crea un realm `zeyn-test` con su propio cliente y usuario y lo elimina al final.
Sin `ZEYN_TEST_OIDC` se omite.
