# API y OpenAPI

Cualquier acción puede ser también un endpoint JSON. Se aplican el mismo tipo de entrada, las mismas reglas y el mismo permiso, de modo que la API no puede desviarse de las pantallas.

## Publicar una acción

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

Cada acción se convierte en `POST /api/v1/{feature}/{action}`, como `POST /api/v1/stock/save_product`. El cuerpo es la entrada de la acción en JSON y la respuesta es su salida en JSON.

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` cambia el prefijo.

## Reglas para quien llama

- Las llamadas necesitan una cabecera `Authorization`. La lee el autenticador de la aplicación; una cookie de sesión del navegador por sí sola se rechaza, así que la API no necesita token CSRF.
- El acceso, los guards, los interceptores, la auditoría y los límites de frecuencia se aplican exactamente igual que en las pantallas.
- Las entradas que reciben `Valid<M>` ejecutan todas las `#[rule]`. Los errores de campo vuelven como `422` con el mensaje de cada campo.
- Las listas reciben un `DataQuery` y devuelven un `Page`; el servidor limita el tamaño de página.

## Errores

Los errores son problem details de RFC 9457 (`application/problem+json`):

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| Estado | `type` termina en | Cuándo |
|---|---|---|
| 403 | `forbidden` | No hay cabecera `Authorization`, o no hay permiso |
| 404 | `not_found` | Registro desconocido, o una entrada que no se puede interpretar |
| 409 | `conflict` | La versión de la fila cambió |
| 422 | `validation` | Errores de campo |
| 429 | `forbidden` | Demasiadas peticiones; consulta `Retry-After` |
| 504 | `timeout` | La llamada tardó más que el límite del servidor |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

Con `api_docs`, la aplicación publica `{api_prefix}/openapi.json` (OpenAPI 3.1; impórtalo en Postman, un generador de clientes o cualquier visor OpenAPI) y una referencia legible en `{api_prefix}/docs`. Los esquemas salen de los modelos: tipos, campos obligatorios, longitudes, límites y valores de choice.

La documentación lista cada endpoint y el permiso que necesita, por eso está desactivada por defecto. Elige quién puede leerla:

| Acceso | Quién |
|---|---|
| `ApiDocs::open()` | Cualquiera |
| `ApiDocs::basic(credentials)` | Autenticación HTTP básica, como `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | Usuarios con sesión iniciada que tengan el permiso |

## Aplicaciones de navegador en otros orígenes

`App::cors(["https://app.example.com"])` permite que las aplicaciones de navegador de esos orígenes exactos llamen a la API con una cabecera `Authorization`. Las cookies nunca se envían entre orígenes.
