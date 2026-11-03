# API ve OpenAPI

Her action aynı zamanda bir JSON endpoint'i olabilir. Aynı girdi türü, aynı kurallar ve aynı izin geçerlidir; böylece API ekranlardan sapamaz.

## Bir action'ı yayımlamak

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

Her action `POST /api/v1/{feature}/{action}` olur, örneğin `POST /api/v1/stock/save_product`. Gövde, action'ın girdisidir (JSON); yanıt ise çıktısıdır (JSON).

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` öneki değiştirir.

## Çağıranlar için kurallar

- Çağrılar bir `Authorization` başlığı gerektirir. Uygulamanın kimlik doğrulayıcısı onu okur; yalnızca tarayıcı oturum çerezi reddedilir, bu yüzden API'nin CSRF token'ına ihtiyacı yoktur.
- Erişim, guard'lar, interceptor'lar, denetim kaydı (audit) ve hız sınırları ekranlardaki gibi aynen uygulanır.
- `Valid<M>` alan girdiler her `#[rule]` kuralını çalıştırır. Alan hataları `422` ile, her alanın mesajıyla birlikte döner.
- Listeler bir `DataQuery` alır ve bir `Page` döndürür; sunucu sayfa boyutunu sınırlar.

## Hatalar

Hatalar RFC 9457 problem ayrıntılarıdır (`application/problem+json`):

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| Durum | `type` şununla biter | Ne zaman |
|---|---|---|
| 403 | `forbidden` | `Authorization` başlığı yok ya da izin yok |
| 404 | `not_found` | Bilinmeyen kayıt veya ayrıştırılamayan bir girdi |
| 409 | `conflict` | Satır sürümü değişti |
| 422 | `validation` | Alan hataları |
| 429 | `forbidden` | Çok fazla istek; bkz. `Retry-After` |
| 504 | `timeout` | Çağrı sunucu sınırından uzun sürdü |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

`api_docs` ile uygulama `{api_prefix}/openapi.json` adresini (OpenAPI 3.1; Postman'a, bir istemci üretecine veya herhangi bir OpenAPI görüntüleyicisine aktarın) ve `{api_prefix}/docs` adresinde okunabilir bir başvuru yayımlar. Şemalar modellerden gelir: türler, zorunlu alanlar, uzunluklar, sınırlar ve seçim değerleri.

Dokümantasyon her endpoint'i ve gerektirdiği izni listeler; bu yüzden varsayılan olarak kapalıdır. Onu kimin okuyabileceğini seçin:

| Erişim | Kim |
|---|---|
| `ApiDocs::open()` | Herkes |
| `ApiDocs::basic(credentials)` | HTTP basic kimlik doğrulaması, örneğin `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | İzne sahip, oturum açmış kullanıcılar |

## Başka origin'lerdeki tarayıcı uygulamaları

`App::cors(["https://app.example.com"])`, tam olarak bu origin'lerdeki tarayıcı uygulamalarının API'yi bir `Authorization` başlığıyla çağırmasına izin verir. Çerezler origin'ler arasında asla gönderilmez.
