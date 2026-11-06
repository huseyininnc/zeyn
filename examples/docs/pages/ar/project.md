# المصدر والترخيص والأمان

Zeyn مفتوح المصدر. تبيّن هذه الصفحة مكان الشيفرة، والترخيص الذي تصدر به، وطريقة اختبارها، وكيفية الإبلاغ عن مشكلة أمنية.

## الشيفرة المصدرية

الشيفرة والمشكلات (issues) على GitHub: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn). تُنشر الحزم (crates) على [crates.io](https://crates.io/crates/zeyn)، ومرجع واجهة API على [docs.rs](https://docs.rs/zeyn).

## الترخيص

يصدر Zeyn بترخيص مزدوج: [ترخيص MIT](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT) أو [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE)، بحسب اختيارك (`MIT OR Apache-2.0` في `Cargo.toml`). وتُقبل المساهمات بالترخيصين نفسيهما.

## الجودة

يضم المستودع 767 دالة اختبار (`#[test]` و`#[tokio::test]` في الحزم والأمثلة، أُحصيت في 2026-11-06)، إضافة إلى الأمثلة في تعليقات التوثيق التي تعمل كاختبارات doctest.

يشغّل كل pull request وكل push إلى `dev` و`main` نظام CI:

- `cargo fmt --all --check` للتنسيق؛
- `cargo clippy --workspace --all-targets -- -D warnings`، فأي تحذير lint يوقف البناء؛
- `cargo test --workspace`، اختبارات كل حزمة وكل مثال؛
- `cargo test -p zeyn` مع الميزات `assistant` و`oidc` و`totp` و`islands` و`realtime` و`xlsx` و`email` و`accounts`؛
- `cargo test -p zeyn-cli`، الذي يبني إطار العمل أيضاً دون ميزة الخادم؛
- `cargo deny` للتراخيص والتنبيهات الأمنية والاعتماديات المحظورة.

نتائج كل تشغيل منشورة على [صفحة Actions](https://github.com/huseyininnc/zeyn/actions).

## الأمان

- **الإصدارات المدعومة.** Zeyn في مرحلة alpha. لا يتلقى الإصلاحات الأمنية إلا أحدث إصدار `0.0.x`.
- **الإبلاغ.** أبلغ عن الثغرة بشكل خاص عبر GitHub Security Advisories: افتح تبويب Security في المستودع واختر "Report a vulnerability" ([رابط مباشر](https://github.com/huseyininnc/zeyn/security/advisories/new)). لا تبلغ عنها أبداً في issue أو نقاش أو pull request علني.
- **الاستجابة.** يُؤكَّد استلام البلاغ خلال 5 أيام عمل. بعد إصدار الإصلاح يُنشر تنبيه أمني ويُذكر اسم المبلِّغ ما لم يفضّل عدم ذكره.

السياسة الكاملة، بما فيها ما يجب أن يتضمنه البلاغ ونطاقها، موجودة في [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md).

## المزيد

- [سجل التغييرات](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): تغييرات كل إصدار.
- [خارطة الطريق](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): الإصدار القادم وما هو مخطط له.
- [دليل المساهمة](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): كيفية البناء والاختبار وإرسال تغيير.
- [مدونة السلوك](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): كيف نعمل معاً.
- [الدعم](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): أين تطرح الأسئلة وتبلغ عن الأخطاء.
