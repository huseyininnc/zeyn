# API وOpenAPI

يمكن لأي إجراء أن يكون أيضًا نقطة نهاية JSON. ينطبق نوع المدخل نفسه والقواعد نفسها والصلاحية نفسها، ولذلك لا يمكن أن تنحرف الواجهة البرمجية عن الشاشات.

## نشر إجراء

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

يصبح كل إجراء `POST /api/v1/{feature}/{action}`، مثل `POST /api/v1/stock/save_product`. جسم الطلب هو مدخل الإجراء بصيغة JSON، والجواب هو مخرجه بصيغة JSON.

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

يغيّر `App::api_prefix("/api/v2")` البادئة.

## قواعد للمستدعين

- تحتاج الاستدعاءات إلى ترويسة `Authorization`. يقرؤها مصادِق التطبيق؛ وكوكي جلسة المتصفح وحده مرفوض، لذلك لا تحتاج الواجهة إلى رمز CSRF.
- الوصول والحراس (guards) والمعترضات (interceptors) والتدقيق وحدود المعدل تنطبق تمامًا كما في الشاشات.
- المدخلات التي تأخذ `Valid<M>` تشغّل كل `#[rule]`. وتعود أخطاء الحقول بالرمز `422` مع رسالة كل حقل.
- تأخذ القوائم `DataQuery` وتعيد `Page`؛ ويضع الخادم حدًا أعلى لحجم الصفحة.

## الأخطاء

الأخطاء هي تفاصيل مشكلة وفق RFC 9457 (`application/problem+json`):

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| الحالة | ينتهي `type` بـ | متى |
|---|---|---|
| 403 | `forbidden` | لا توجد ترويسة `Authorization`، أو لا توجد صلاحية |
| 404 | `not_found` | سجل غير معروف، أو مدخل لا يمكن تحليله |
| 409 | `conflict` | تغير إصدار الصف |
| 422 | `validation` | أخطاء الحقول |
| 429 | `forbidden` | طلبات كثيرة جدًا؛ راجع `Retry-After` |
| 504 | `timeout` | استغرق الاستدعاء أكثر من حد الخادم |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

مع `api_docs` ينشر التطبيق `{api_prefix}/openapi.json` (OpenAPI 3.1؛ استورده في Postman أو مولّد عملاء أو أي عارض OpenAPI) ومرجعًا مقروءًا في `{api_prefix}/docs`. تأتي المخططات (schemas) من النماذج: الأنواع والحقول المطلوبة والأطوال والحدود وقيم الاختيارات.

يسرد التوثيق كل نقطة نهاية والصلاحية التي تحتاجها، ولذلك هو معطّل افتراضيًا. اختر من يحق له قراءته:

| الوصول | من |
|---|---|
| `ApiDocs::open()` | أي شخص |
| `ApiDocs::basic(credentials)` | مصادقة HTTP الأساسية، مثل `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | المستخدمون الذين سجلوا الدخول ولديهم الصلاحية |

## تطبيقات المتصفح على أصول أخرى

يتيح `App::cors(["https://app.example.com"])` لتطبيقات المتصفح على هذه الأصول (origins) بالضبط أن تستدعي الواجهة بترويسة `Authorization`. لا تُرسل الكوكيز عبر الأصول أبدًا.
