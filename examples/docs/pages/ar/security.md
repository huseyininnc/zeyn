# الأمان

يُعلَن الأمان في Zeyn بجوار الشيفرة التي يحميها ويُفحص على الخادم عند كل استدعاء. الإعدادات الافتراضية صارمة؛ وتخفيف أحدها سطر ظاهر في `app.rs`.

## الصلاحيات

يعلن كل إجراء وشاشة `permission = "feature.resource.verb"` أو `public`. وبدون أحدهما لا تُصرَّف الشيفرة. تحمل هوية الطلب (identity) صلاحياته:

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

عناصر القائمة وأزرار شريط الأدوات وإجراءات الصفوف والأوامر التي لا يحق للمستخدم تشغيلها تُستبعد من HTML. الاستبعاد وسيلة راحة؛ أما الخادم فيفحص الصلاحية مرة أخرى عند كل استدعاء.

## تسجيل الدخول

لا يحتفظ التطبيق بكلمات المرور. يعرف `Authenticator` من الذي يستدعي، وعادةً من وكيل تسجيل دخول موحد (single sign on) أمام التطبيق:

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

لا تستخدم `ForwardAuth` إلا عندما يمر كل طلب عبر الوكيل؛ فكل من يستطيع الوصول إلى التطبيق مباشرة يستطيع إرسال هذه الترويسات. يفحص `require_header` سراً مشتركاً يضيفه الوكيل. يمنح `DevAuth::superuser(name)` كل طلب وصولاً كاملاً للتطوير المحلي، ويطبع الخادم تحذيراً عند تفعيله. ويرفض إصدار release البدء به (ويبلّغ `App::check` عن ذلك)؛ أما العرض التوضيحي الذي يجب أن يعمل في release فيمكنه استدعاء `App::allow_dev_auth()`. التطبيقات المنشأة بـ `zeyn new` تفعّله في إصدارات debug فقط.

## الحراس

يعمل الحارس (guard) قبل فتح أي شاشة من ميزة وقبل استدعاء أي من جداولها أو نماذجها أو أوامرها:

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

الحراس المضمنة: `SignedIn` و`TenantRequired` و`Licensed`.

## المعترضات

يعمل المعترض (interceptor) حول كل استدعاء لشاشة أو جدول أو نموذج أو API أو أمر، بعد فحوص الوصول. يستطيع `before` إيقاف الاستدعاء؛ ويرى `after` كيف انتهى. و`AuditLog(sink)` هو المعترض المضمن الذي يسجّل من فعل ماذا.

## CSRF وCSP والترويسات

- كل صفحة تضبط ملف تعريف ارتباط عشوائياً `zeyn_csrf` (`HttpOnly` و`SameSite=Strict`) وتضع الرمز نفسه في وسم meta. كل استدعاء من وقت التشغيل يرسله في الترويسة `x-zeyn-csrf` ويقارن الخادم بين القيمتين في زمن ثابت.
- الشاشات العامة المفتوحة بلا جلسة استثناء: لا تضبط ملف تعريف ارتباط ولا تحمل رمزاً، لذلك يمكن تخزينها مؤقتاً. يجلب وقت التشغيل الرمز من `GET /_zeyn/csrf` قبل أول استدعاء له، ويظل كل `POST` يُفحص بالطريقة نفسها (راجع [Public pages](public-pages.md)).
- تسمح سياسة أمان المحتوى (Content Security Policy) بالسكربتات والأنماط والصور من أصل التطبيق نفسه فقط. لا توجد سكربتات مضمنة؛ والسمات (themes) أوراق أنماط، وليست أنماطاً مضمنة أبداً.
- يفتح `App::csp` السياسة لمضيفين مسمّين، توجيهاً واحداً في كل مرة. تُبنى الترويسة مرة واحدة عند بدء التشغيل.

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

  لا يُقبل إلا `https://host` و`https://host:port` و`https://*.host` (يطابق حرف البدل النطاقات الفرعية لا المضيف نفسه). يضيف `.media_blob()` القيمة `blob:` إلى `media-src` لمكتبات البث. يرفض `App::check` القيمة `*` و`http:` والمخططات المجردة مثل `data:` أو `https:` والكلمات المفتاحية المقتبسة مثل `'unsafe-inline'` أو `'unsafe-eval'` والمسارات وأسماء المستخدمين والقيم التي تحوي مسافات أو فواصل أو فواصل منقوطة؛ وتذكر كل مشكلة ما ينبغي كتابته بدلاً منها. لا يمكن تغيير `script-src` و`object-src` و`base-uri` و`frame-ancestors`. يقرأ `Embed` و`Image` القوائم نفسها، لذلك لا يُعرض أبداً مضيف كان المتصفح سيحجبه (راجع [Display](display.md)).
- تحمل الاستجابات `X-Content-Type-Options: nosniff` و`Referrer-Policy: same-origin` و`X-Frame-Options: DENY`.
- يعلّم `App::secure_cookies(true)` ملفات تعريف الارتباط بـ `Secure`؛ فعّله خلف HTTPS.

## تهريب القيم (Escaping)

بيانات المستخدم في HTML تمر دائماً عبر `render::escape`. تهرّب المكونات كل قيمة تعرضها، فيظهر منتج اسمه `<script>` كنص.

## الحدود

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- ينطبق `rate_limit` على كل طلب عدا الأصول الثابتة وقراءات مصادر الوسائط العامة (`Feature::public_media`)؛ ويضيف `write_limit` حداً أشد للاستدعاءات التي تغيّر البيانات: إرسال النماذج وتعديلات الجداول وإجراءات الصفوف والإجراءات الجماعية وإجراءات القائمة والأوامر والاستيراد وعروض الجداول المحفوظة وتخطيطات dock والسمات واستدعاءات API واستدعاءات MCP والمساعد. تحصل الطلبات المرفوضة على `429` مع `Retry-After`.
- لا يُصدَّق `X-Forwarded-For` إلا من `trusted_proxies`. يُحتسب كل سطر من الترويسة بالترتيب، والعنوان المأخوذ هو الأخير الذي ليس وكيلاً موثوقاً.
- تُحتسب استدعاءات الكشف (reveals) والجزر (islands) ومصادر البيانات كعمليات كتابة أيضاً، وكذلك عمليات التصدير والمستندات المطبوعة (`GET /_zeyn/grid/export` و`GET /_zeyn/pdf`).
- أحجام الصفحات وأعداد البنود وأطوال الاستعلامات وأحجام الرفع تُقيَّد على الخادم مهما أرسل العميل.

## الجلسات

يسجّل `App::idle_timeout(d)` خروج المستخدمين من الصفحة بعد مدة بلا إدخال، مع مربع حوار تحذيري أولاً. الإجراءات المعلّمة بـ `reauth` تحتاج إلى تسجيل دخول حديث (`App::reauth_within`).

## حماية البيانات

إخفاء الحقول (masking) وذكر السبب قبل فتح السجلات الحساسة والتدقيق على مستوى الحقل وتدقيق القراءة موضحة في [Audit, masking and jobs](enterprise.md).

## واجهة API والوكلاء

تحتاج استدعاءات API إلى ترويسة `Authorization`؛ ولا يستدعي ملف تعريف ارتباط جلسة المتصفح وحده واجهة API أو نقطة MCP أبداً، لذلك لا تحتاج إلى رمز CSRF. يحصل الوكلاء على صلاحيات المستخدم الذي يعملون نيابةً عنه بالضبط. راجع [API and OpenAPI](api.md) و[MCP and AI agents](mcp.md).
