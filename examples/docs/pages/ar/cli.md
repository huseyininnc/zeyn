# سطر الأوامر والمشاريع

ينشئ الأمر `zeyn` المشاريع، ويضيف الميزات (features)، ويفحص التطبيق، ويشغّل خادم MCP للوكلاء.

## الأوامر

| الأمر | ماذا يفعل |
|---|---|
| `zeyn new <name>` | ينشئ تطبيقًا ببنية الميزات القياسية |
| `zeyn new <name> --lib` | ينشئ مكتبة من النماذج والميزات والنصوص تشترك فيها عدة تطبيقات |
| `zeyn g feature <name>` | يضيف `src/features/<name>` مع `models` و`services` و`actions` و`screens` وشاشة قائمة تعمل |
| `zeyn g screen <feature> <name>` | يضيف `screens/<name>.rs` إلى ميزة ويسجله ويضيف عنوانه إلى كل ملف لغة |
| `zeyn routes` | يسرد الميزات والشاشات والمسارات وواجهات API والوصول |
| `zeyn check` | يصرّف المشروع ويبلّغ عن المشكلات البنيوية والترجمات الناقصة |
| `zeyn check --fix` | يعيد كتابة واجهات Zeyn المعاد تسميتها في `src` و`tests` و`examples`، ثم يفحص |
| `zeyn check --json` | يطبع التقرير بصيغة JSON مع رموز ثابتة وأسباب وحلول |
| `zeyn mcp` | يشغّل خادم MCP على stdio لوكلاء الذكاء الاصطناعي |

ثبّته من crates.io:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

داخل هذا المستودع، شغّل الأداة عبر Cargo:

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## تطبيق جديد

يكتب `zeyn new` مشروعًا يُبنى وينجح في اختباراته ويتبع القواعد المعطاة للوكلاء:

- `AGENTS.md` و`CLAUDE.md`: قواعد المشروع وفهرس للإطار موجّه للوكلاء.
- `.mcp.json`: يسجل خادم MCP المسمى `zeyn`، ليتمكن الوكيل من استدعاء `zeyn_check` و`zeyn_docs`.
- `config/app.json`: القيم الافتراضية. تتجاوزها متغيرات البيئة `APP_*`، مثل `APP_SERVER__ADDR` للمفتاح `server.addr`.
- `locales/en.json` و`locales/tr.json`: كل نص يراه المستخدم.
- `src/features/`: مجلد واحد لكل ميزة، مع ميزة نموذجية اسمها `notes`.
- `rustfmt.toml` و`clippy.toml` و`deny.toml` و`rust-toolchain.toml` و`[lints]` في `Cargo.toml`: معايير يفرضها CI.
  يضبط `rustfmt.toml` القيمة `chain_width = 40`، ليُقرأ الكود من الأعلى إلى الأسفل: سلسلة الاستدعاءات
  الأطول من ذلك تأخذ استدعاءً واحدًا في كل سطر، بمعدل نقطة أو نقطتين في السطر. يراجع الوكلاء والناس
  قائمة أسرع من سطر طويل واحد، وتُظهر الفروق (diffs) الاستدعاء الذي تغير وحده. أبقِ على هذا الإعداد.

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  وليس `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`.
- `build.rs` و`max_file_lines = 300` تحت `[package.metadata.zeyn]` في `Cargo.toml`: يتوقف `cargo build` عندما يزيد عدد الأسطر غير الفارغة في ملف داخل `src/`؛ ويفعل clippy `too_many_lines` (60، في `clippy.toml`) الشيء نفسه للدوال. انظر [Project structure](structure.md).
- `Dockerfile` و`.dockerignore`: صورة حاوية صغيرة.
- خطوط إنتاج (pipelines) لخوادم CI الشائعة، وكلها تجري الفحوص نفسها (`cargo fmt --check` و`cargo clippy -D warnings` و`cargo test` و`cargo deny check`) وترفض وسمًا `vX.Y.Z` لا يطابق `Cargo.toml`:
  - `.github/workflows/ci.yml` لـ GitHub Actions، مع `.github/dependabot.yml` (تحديثات أسبوعية لـ Cargo والإجراءات) وقائمة مراجعة لطلبات السحب (pull requests)؛
  - `.gitlab-ci.yml` لـ GitLab؛
  - `Jenkinsfile` لـ Jenkins (وكيل Docker، وتغطية بـ `cargo llvm-cov`).
- `sonar-project.properties`: تحليل SonarQube (مشكلات Clippy وتغطية LCOV من `target/lcov.info`). يشغّله سير عمل GitHub عندما يكون متغير المستودع `SONAR_HOST_URL` والسر `SONAR_TOKEN` مضبوطين؛ ويشغّله Jenkins عندما تضبط المهمة `SONAR_ENABLED=true`.
- `SECURITY.md` و`CHANGELOG.md` (Keep a Changelog).
- مستودع git محلي على الفرع `main`، عندما يكون git مثبتًا (Windows أو macOS أو Linux) ولا يكون المجلد داخل مستودع أصلًا. لا يُنفَّذ أي commit، فيحمل أول commit اسمك وبريدك. يتخطاه الخيار `--no-git`.

راجع [بنية المشروع](structure.md) لمعرفة كل مجلد.

## المولدات

ينشئ `zeyn g feature` ميزة بملف واحد لكل نموذج وخدمة وإجراء وشاشة:

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

الصيغتان القصيرتان `zeyn g f` و`zeyn g s` تفعلان الشيء نفسه. تعمل الميزة الجديدة فورًا: نموذج وخدمة بيانات في الذاكرة وإجراء بحث وشاشة قائمة مع عنصرها في القائمة. والميزة التي لا تحتاج مجلدًا تحذفه (المجلد وسطر `mod` الخاص به في `mod.rs` للميزة). لا يُنشأ `components/` أبدًا: أضفه يدويًا، بدالة واحدة تعيد `Node` في كل ملف (`components/<name>.rs`)، عندما يُستخدم جزء من شاشة في أكثر من مكان أو يطول ملف شاشة. تحصل الشاشة على الإذن `<feature>.<name>.view` وعلى عنوانها في كل ملف في `locales/`؛ وتحصل اللغات غير الإنجليزية على النص الإنجليزي مع ملاحظة لترجمته. هذه البنية ليست إلزامية: إذا كانت الميزة تستخدم ملفًا واحدًا `screens.rs`، ينقله `zeyn g screen` أولًا بمحتواه إلى `screens/mod.rs`، وإذا حُذف `screens/` ينشئه من جديد. لا تكتب المولدات فوق أي ملف أبدًا، وترفض الأسماء التي ليست بصيغة snake case.

## فحص تطبيق

يشغّل `zeyn check` المصرّف (compiler) ثم فحوص التطبيق نفسه (`App::check`): المسارات المكررة، والشاشات المسجلة مرتين، والخدمات التي تحتاجها شاشة ولم يسجلها أحد، والاختصارات غير القابلة للاستخدام، ورموز السمة (theme tokens) التي يقل تباينها عن WCAG AA، ومفاتيح الترجمة الناقصة في لغة ما. ينتهي بخطأ عند وجود أي خلل، ليتمكن CI من تشغيله.

ويطبع أيضًا تحذيرات لا تُفشل الفحص أبدًا: `#[screen]` أو `#[action]` لا تسجله أي ميزة، وملف واحد `screens.rs` أو `actions.rs` فيه عدة عناصر وتجاوز نحو 150 سطرًا.

عندما يعيد إصدار من Zeyn تسمية واجهة API، يعيد `zeyn check --fix` كتابة الأسماء القديمة، مثل `Grid` إلى `DataGrid` و`GridQuery` إلى `DataQuery`، ثم يفحص مرة أخرى.

```sh
zeyn check --fix
```

لكل مشكلة رمز ثابت: `Z01xx` للتطبيق والميزات والمسارات والخدمات، و`Z02xx` للأمان، و`Z03xx` للصفحات العامة، و`Z04xx` للقوائم والأوامر والاختصارات، و`Z05xx` للبيانات، و`Z06xx` لفحوص الصفحات، و`W01xx` للتحذيرات. يعرض الناتج النصي كل مشكلة مع سببها (`why`) وحلها (`help`) ورابط إلى هذا التوثيق ورمزها. يطبع `zeyn check --json` التقرير نفسه كمستند JSON واحد للوكلاء وCI، ويخرج بخطأ عندما تكون قيمة `ok` هي false.

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

يقرأ `zeyn check` أيضًا مصدر التطبيق وينبّه، مع الملف والسطر، إلى `W0106` نص يحمل `<script>` دون `src` أو معالج أحداث مضمّنًا مثل `onclick=` (تحظر سياسة CSP كليهما)، و`W0107` مكوّن مكتوب يدويًا يكتب HTML دون `escape(..)` أو `r.text(..)` أو `r.value(..)`، و`W0108` action يعيد `Vec` كاملًا دون استعلام يحدّه، و`W0109` `DocumentViewer` مُعلَّم بـ `.trusted()`. ولا يحتاج غياب `permission` أو `public` إلى تنبيه: فهو لا يُترجم.

## Docker

يبني `Dockerfile` المولَّد على مرحلتين ويعمل على صورة distroless بمستخدم بلا صلاحيات الجذر. يفحص الملف التنفيذي صحته بنفسه عبر `<app> health`، وهو ما يستخدمه `HEALTHCHECK`.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

حجم صورة التطبيق المولَّد نحو 43 ميغابايت، ويستخدم التطبيق في وضع الخمول بضعة ميغابايتات من الذاكرة.

## خادم MCP

يتحدث `zeyn mcp` بروتوكول MCP عبر stdio. أدواته هي `zeyn_docs` (مرجع الإطار حسب الموضوع) و`zeyn_describe` (ميزات التطبيق في المجلد الحالي وشاشاته ومساراته ومشكلاته) و`zeyn_inspect` (شجرة عُقد الشاشة مع سطر Rust الذي وضع كل عقدة) و`zeyn_check` (مع `fix: true` يشغّل `check --fix`) و`zeyn_test` و`zeyn_new_feature` و`zeyn_new_screen`. راجع [MCP ووكلاء الذكاء الاصطناعي](mcp.md).
