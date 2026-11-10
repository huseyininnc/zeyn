# تطبيقات سطح المكتب (Desktop apps)

يمكن شحن تطبيق Zeyn كملف تنفيذي واحد لسطح المكتب. تشغّل الحزمة `zeyn-desktop` خادم التطبيق نفسه داخل العملية وتعرضه في نافذة أصلية عبر WebView النظام:

| المنصة | WebView | الحالة |
|----------|---------|--------|
| Windows 10 و 11 | WebView2 (Edge) | مبني ومفحوص يدوياً |
| macOS | WKWebView | يُصرَّف عبر `wry`، غير مفحوص يدوياً |
| Linux | WebKitGTK 4.1 | يُصرَّف عبر `wry` عند تثبيت `libwebkit2gtk-4.1-dev` و `libgtk-3-dev`، غير مفحوص يدوياً |

لا يغادر شيء الجهاز. يعمل التطبيق دون اتصال ما دام لا يستدعي خدمات بعيدة بنفسه.

## الاستخدام

أضف الحزمة خلف ميزة (feature)، كي لا يصرّف البناء العادي للخادم WebView:

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.10", optional = true }
```

شغّل النافذة بدلاً من `App::serve`:

```rust
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    zeyn_desktop::Desktop::new(app()?)
        .id("my-app")
        .title("My App")
        .size(1280, 800)
        .run()
}
```

- يسمّي `id` مجلد البيانات الخاص بكل مستخدم ويُبقي نسخة واحدة لكل معرّف. وقيمته الافتراضية اسم الملف التنفيذي.
- يظهر `title` إلى أن تضبط الصفحة `document.title`؛ وبعدها يتبع عنوان النافذة عنوان الصفحة.
- `size` هو الحجم الأول بالبكسلات المنطقية. وتعيد عمليات التشغيل اللاحقة استخدام آخر حجم وموضع.

يحجب `run` التنفيذ حتى تُغلق النافذة، ثم يوقف الخادم وينهي العملية. ومع `ZEYN_DESCRIBE=1` يطبع وصف التطبيق ويعود، فيستمر عمل `zeyn check` و `zeyn routes`.

لإخفاء نافذة وحدة التحكم (console) في بناء release على Windows، ضع هذا في أعلى `main.rs`:

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` مثال كامل، وهو تطبيق نقطة بيع:

```
cargo run -p zeyn-example-adisyon --features desktop
```

## بناء الملف التنفيذي

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

يشغّل `cargo build --release` مع تحسين وقت الربط (link time optimization)، ووحدة توليد شيفرة واحدة، ورموز مجرّدة (stripped). وتبقى السرعة عند `opt-level = 3`. ثم يطبع مسار كل ملف تنفيذي وحجمه.

### الأيقونة ومعلومات الإصدار والـ manifest (Windows)

يقرأ Windows هذه من موارد داخل ملف exe. أضف سكربت بناء يضمّنها لبناء سطح المكتب فقط:

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`:

```rust
fn main() {
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "My App")
            .set("FileDescription", "My App")
            .set_manifest_file("assets/app.exe.manifest");
        res.compile().unwrap();
    }
}
```

يجب أن يكون معرّف مورد الأيقونة 1 (القيمة الافتراضية لـ `set_icon`)؛ فالنافذة وشريط المهام تستخدمانها. ويأتي الإصدار من `package.version`. راجع `examples/adisyon/build.rs` و `examples/adisyon/assets` لمثال على manifest يدعم DPI لكل شاشة، و UTF-8، و common controls 6. تجد سلسلة أدوات MSVC الملف `rc.exe` في Windows SDK؛ وسلسلة GNU تستخدم `windres`.

## بيئة WebView2 التشغيلية

يأتي Windows 11 و Windows 10 الحديث ومعهما بيئة WebView2 مثبّتة مسبقاً وتحدّث نفسها بنفسها (بيئة evergreen). لا يضمّها ملف exe، وهذا يُبقيه صغيراً.

للأجهزة الأقدم أو المقيّدة:

- أرفق مع مثبّتك أداة Microsoft المسماة evergreen bootstrapper (`MicrosoftEdgeWebview2Setup.exe`، نحو 2 MB) وشغّلها بالمعاملين `/silent /install` عندما تكون البيئة غائبة. تنزّل البيئة مرة واحدة.
- للأجهزة بلا إنترنت، استخدم المثبّت evergreen standalone (كبير، دون اتصال)، أو بيئة الإصدار الثابت (fixed version) الموضوعة بجانب ملف exe. الإصدار الثابت لا يحدّث نفسه، فأنت المسؤول عن تحديثاته الأمنية.

بدون البيئة لا تستطيع النافذة أن تُفتح ويعيد `run` خطأ.

## نموذج الأمان

لا يمكن الوصول إلى الخادم إلا من نافذة التطبيق نفسه.

- **loopback فقط.** يرتبط المستمع (listener) بالعنوان `127.0.0.1` على منفذ يختاره النظام. ولا يرتبط أبداً بـ `0.0.0.0` ولا بعنوان في الشبكة المحلية.
- **رمز التشغيل (Launch token).** يولّد كل تشغيل رمزاً عشوائياً من 256 بت. تفتح النافذة أولاً `/_desktop/start?t=<token>`، الذي يضبط الرمز كملف تعريف ارتباط `HttpOnly` و `SameSite=Strict` ويعيد التوجيه إلى `/`. ويجب أن يحمل كل طلب آخر الرمز في ملف الارتباط هذا أو في الترويس `x-zeyn-desktop`، وإلا حصل على `403`. الأصول الثابتة محمية أيضاً. يعيش الرمز في الذاكرة فقط ويُقارن في زمن ثابت.
- **Host و Origin.** يجب أن يكون ترويس `Host` هو `127.0.0.1:<port>` تماماً، وهذا يمنع DNS rebinding (موقع يُحلّ اسمه إلى 127.0.0.1 يرسل اسمه هو كمضيف). وترويس `Origin`، عند وجوده، يجب أن يكون `http://127.0.0.1:<port>` تماماً. ينطبق الاثنان على كل مسار، بما فيه نقطتا البدء والتركيز (start و focus).
- **البرامج المحلية الأخرى** تستطيع الاتصال بالمنفذ لكنها لا تعرف الرمز. وحسابات المستخدمين الأخرى لا تستطيع قراءة ملف instance في مجلد البيانات الخاص بكل مستخدم.
- **التنقل.** تعرض النافذة origin الخاص بها وروابط `blob:` الخاصة بها و`about:blank` فقط. تُفتح روابط `http` و `https` و `mailto` في المعالج الافتراضي للنظام؛ وكل مخطط آخر (`file:` ومعالجات البروتوكولات المخصصة و`javascript:`) يُسقط. و`window.open` لا ينشئ WebView ثانياً أبداً.
- **أدوات المطور (Devtools)** مفعّلة في بناءات debug ومعطّلة في بناءات release.
- **التنزيلات** مسموحة فقط من origin التطبيق وتذهب إلى مجلد Downloads الخاص بالمستخدم، دون الكتابة فوق ملف موجود أبداً (`orders.csv` ثم `orders (1).csv` وهكذا). يُسمح بعدة تنزيلات متتالية دون سؤال. أما طلبات الأذونات الأخرى (الكاميرا والميكروفون والحافظة) فتحصل على نافذة السؤال المعتادة في WebView.

أمور لا يغطيها الحارس:

- برنامج يعمل بالمستخدم نفسه يستطيع قراءة ذاكرة العملية أو إرفاق مصحّح (debugger)؛ ولا يستطيع أي تطبيق محلي الدفاع ضد ذلك.
- يحترم WebView2 متغير البيئة `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`، فمن يستطيع ضبط متغيرات البيئة للمستخدم يستطيع تشغيل التصحيح عن بعد. المستخدم نفسه والحد نفسه المذكور أعلاه.
- تظل مصادقة التطبيق نفسه سارية داخل النافذة. و`DevAuth` في بناء سطح المكتب يمنح وصولاً كاملاً لمن يجلس أمام الجهاز.

## السلوك

- **نسخة واحدة (Single instance).** أول تشغيل يكتب `instance` (المنفذ ومفتاح تركيز منفصل) في مجلد البيانات. التشغيل الثاني يرسل المفتاح إلى `/_desktop/focus`؛ فتأتي النافذة الأولى إلى المقدمة وتنتهي العملية الثانية خلال نحو 150 ms. وإذا بدأ تشغيلان في اللحظة نفسها فقد يُفتح كلاهما.
- **حالة النافذة.** يُحفظ الحجم والموضع وعلم التكبير في `window.json` عند الإغلاق. والموضع المحفوظ الذي لم يعد على أي شاشة يُتجاهل.
- **الإيقاف.** إغلاق النافذة يُسقط WebView ويوقف الخادم بسلاسة (ثانيتان على الأكثر) وينهي العملية. وتنتهي عمليات WebView معها.
- **مجلد البيانات.** `%LOCALAPPDATA%\<id>` على Windows، و `~/Library/Application Support/<id>` على macOS، و `~/.local/share/<id>` على Linux. ويقع ملف تعريف WebView (التخزين المؤقت والتخزين المحلي) في `webview` داخله، فيعمل ملف exe أيضاً من مجلد للقراءة فقط مثل Program Files.
- **لا أيقونة في منطقة الإشعارات (tray).** غير مبنية.

## الحجم والذاكرة

مقيسة على Windows 11 مع `examples/adisyon`، بناء release، وبيئة WebView2 رقم 156:

| | |
|---|---|
| ملف exe الناتج عن `zeyn build-desktop` | 6.72 MB (7.25 MB مع ملف release العادي) |
| عملية التطبيق، خاملة | 4.3 MB خاصة، 25.6 MB working set |
| عملية التطبيق وعمليات WebView2، خاملة | 7 عمليات، نحو 180 MB خاصة، 370 MB working set |
| الشيء نفسه مع `--disable-gpu` | نحو 114 MB خاصة، 330 MB working set |
| التشغيل الثاني حتى ينتهي | نحو 130 ms |

يهيمن WebView على الاستهلاك. عملية GPU وحدها تستهلك نحو 88 MB. للشاشات البسيطة على الأجهزة قليلة الذاكرة، شغّل ملف exe مع `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` (مثلاً من اختصار أو سكربت تشغيل) لتوفير نحو 65 MB، على حساب العرض البرمجي (software rendering).
