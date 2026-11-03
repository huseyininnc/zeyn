# سطر الأوامر والمشاريع

ينشئ الأمر `zeyn` المشاريع، ويضيف الميزات (features)، ويفحص التطبيق، ويشغّل خادم MCP للوكلاء.

## الأوامر

| الأمر | ماذا يفعل |
|---|---|
| `zeyn new <name>` | ينشئ تطبيقًا ببنية الميزات القياسية |
| `zeyn new <name> --lib` | ينشئ مكتبة من النماذج والميزات والنصوص تشترك فيها عدة تطبيقات |
| `zeyn g feature <name>` | يضيف `src/features/<name>` مع النماذج والمخزن والإجراءات والشاشات |
| `zeyn routes` | يسرد الميزات والشاشات والمسارات وواجهات API والوصول |
| `zeyn check` | يصرّف المشروع ويبلّغ عن المشكلات البنيوية والترجمات الناقصة |
| `zeyn check --fix` | يعيد كتابة واجهات Zeyn المعاد تسميتها في `src` و`tests` و`examples`، ثم يفحص |
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
- `Dockerfile` و`.dockerignore`: صورة حاوية صغيرة.
- خطوط إنتاج (pipelines) لخوادم CI الشائعة، وكلها تجري الفحوص نفسها (`cargo fmt --check` و`cargo clippy -D warnings` و`cargo test` و`cargo deny check`) وترفض وسمًا `vX.Y.Z` لا يطابق `Cargo.toml`:
  - `.github/workflows/ci.yml` لـ GitHub Actions، مع `.github/dependabot.yml` (تحديثات أسبوعية لـ Cargo والإجراءات) وقائمة مراجعة لطلبات السحب (pull requests)؛
  - `.gitlab-ci.yml` لـ GitLab؛
  - `Jenkinsfile` لـ Jenkins (وكيل Docker، وتغطية بـ `cargo llvm-cov`).
- `sonar-project.properties`: تحليل SonarQube (مشكلات Clippy وتغطية LCOV من `target/lcov.info`). يشغّله سير عمل GitHub عندما يكون متغير المستودع `SONAR_HOST_URL` والسر `SONAR_TOKEN` مضبوطين؛ ويشغّله Jenkins عندما تضبط المهمة `SONAR_ENABLED=true`.
- `SECURITY.md` و`CHANGELOG.md` (Keep a Changelog).
- مستودع git محلي على الفرع `main`، عندما يكون git مثبتًا (Windows أو macOS أو Linux) ولا يكون المجلد داخل مستودع أصلًا. لا يُنفَّذ أي commit، فيحمل أول commit اسمك وبريدك. يتخطاه الخيار `--no-git`.

راجع [بنية المشروع](structure.md) لمعرفة كل مجلد.

## فحص تطبيق

يشغّل `zeyn check` المصرّف (compiler) ثم فحوص التطبيق نفسه (`App::check`): المسارات المكررة، والشاشات المسجلة مرتين، والخدمات التي تحتاجها شاشة ولم يسجلها أحد، والاختصارات غير القابلة للاستخدام، ورموز السمة (theme tokens) التي يقل تباينها عن WCAG AA، ومفاتيح الترجمة الناقصة في لغة ما. ينتهي بخطأ عند وجود أي خلل، ليتمكن CI من تشغيله.

عندما يعيد إصدار من Zeyn تسمية واجهة API، يبقى الاسم القديم إصدارًا واحدًا كاسم بديل مهجور (deprecated alias). يعيد `zeyn check --fix` كتابة الأسماء القديمة، مثل `Grid` إلى `DataGrid` و`GridQuery` إلى `DataQuery`، ثم يفحص مرة أخرى.

```sh
zeyn check --fix
```

## Docker

يبني `Dockerfile` المولَّد على مرحلتين ويعمل على صورة distroless بمستخدم بلا صلاحيات الجذر. يفحص الملف التنفيذي صحته بنفسه عبر `<app> health`، وهو ما يستخدمه `HEALTHCHECK`.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

حجم صورة التطبيق المولَّد نحو 43 ميغابايت، ويستخدم التطبيق في وضع الخمول بضعة ميغابايتات من الذاكرة.

## خادم MCP

يتحدث `zeyn mcp` بروتوكول MCP عبر stdio. أدواته هي `zeyn_docs` (مرجع الإطار حسب الموضوع) و`zeyn_describe` (ميزات التطبيق في المجلد الحالي وشاشاته ومساراته ومشكلاته) و`zeyn_check` (مع `fix: true` يشغّل `check --fix`) و`zeyn_test` و`zeyn_new_feature`. راجع [MCP ووكلاء الذكاء الاصطناعي](mcp.md).
