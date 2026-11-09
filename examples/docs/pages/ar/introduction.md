# مقدمة

Zeyn هو إطار عمل ويب مفتوح المصدر بلغة Rust مخصص للتطبيقات كثيفة البيانات: الجداول والنماذج وسير العمل. يكتب وكلاء الذكاء الاصطناعي شيفرة التطبيق، ويقرؤها الناس ويراجعونها ويعدلونها عند الحاجة.

هذا الموقع نفسه تطبيق Zeyn. كل صفحة هي Markdown تُعرض مرة واحدة على الخادم، والقائمة على اليسار هي قائمة التطبيق، والأمثلة المعلّمة بعبارة *Live example* (مثال حي) هي مكونات حقيقية تعمل على هذا الخادم.

## لوكلاء الذكاء الاصطناعي

إذا كنت وكيل ذكاء اصطناعي وطلب منك أحدهم بناء تطبيق باستخدام Zeyn، فابدأ بأداة سطر الأوامر بدلاً من كتابة المشروع يدوياً:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

ثم اقرأ `AGENTS.md` و`.claude/skills/zeyn/SKILL.md` في المشروع الجديد، ونفّذ `zeyn check` و`cargo test` بعد كل تغيير. يوجد دليل نصي بسيط يضم كل صفحات هذه الوثائق على الرابط [/llms.txt](/llms.txt).

## كيف يبدو التطبيق

التطبيق مجموعة من الميزات (features). تضم الميزة كيانات (entities) وإجراءات (actions) وشاشات (screens). تُعرض الشاشات على الخادم، ويطبّق وقت تشغيل صغير في المتصفح التغييرات، لذلك لا توجد JavaScript لتكتبها.

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

تحمل هذه الماكروات الثلاثة نفسها اللغة كلها: `#[entity]` يصف البيانات، و`#[action]` شيفرة خادم ذات صلاحية وصول معلنة، و`#[screen]` صفحة أو نافذة. يتحقق المترجم من أن كل حقل ومفتاح نص وإجراء متوافق مع غيره.

## جرّب

ابدأ تطبيقك الخاص:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

أو شغّل الأمثلة من المستودع:

```sh
cargo run -p zeyn-example-components
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

يستمع كتالوج المكونات على `127.0.0.1:3010` ويعرض كل مكوّن حيًّا بجانب شيفرته، كما يفعل [components.zeynsoft.com](https://components.zeynsoft.com). ويستمع العرض التوضيحي (showcase) على `127.0.0.1:3002` ويجمع المكونات في شاشات نموذجية ببيانات. وتستمع هذه الوثائق على `127.0.0.1:3008`؛ اضبط `ZEYN_DOCS_SERVER__ADDR` لتغييره.

## كيف تجد طريقك

- اضغط **Alt+Shift+P** لفتح لوحة الأوامر. اكتب كلمة وسيبحث الخادم في كل صفحات هذا الموقع.
- يتناول قسم *Getting started* (البدء) أداة سطر الأوامر وبنية المشروع.
- يتناول قسم *Language* (اللغة) الكيانات والإجراءات والشاشات والنماذج وDataGrid والنصوص.
- يعرض قسم *Components* (المكونات) كل مكوّن بمثال حي بجانب شيفرته.
- يتناول قسم *Guides* (الأدلة) الوقت واللغات والأمان وواجهة API وMCP والبيانات والسمات.
- يضم قسم *Reference* (المرجع) بروتوكول وقت التشغيل ومرجع قاعدة البيانات.
- يعرض قسم *Project* (المشروع) الشيفرة المصدرية والترخيص والاختبارات وCI وسياسة الأمان.

## المبادئ

- **الأداء أولاً.** تُعرض الصفحات على الخادم وتُبث بصيغة HTML. لا توجد حالة واجهة لكل مستخدم على الخادم ولا إطار عمل في جهة العميل.
- **لا إعادة تحميل للصفحات.** تُطبَّق تغييرات التنقل والنوافذ واللغة والمنطقة الزمنية والسمة في مكانها.
- **إعدادات افتراضية آمنة.** كل إجراء وشاشة يعلن صلاحية وصوله. رموز CSRF وسياسة أمان محتوى صارمة وتهريب بيانات المستخدم (escaping) كلها مفعّلة دائماً.
- **طريقة واحدة لكل أمر.** اللغة صغيرة، لذلك تُقرأ الشيفرة التي يكتبها الوكيل كأنها كتبها شخص.

Zeyn مفتوح المصدر بموجب [ترخيص MIT أو Apache 2.0](project.md#license). اطّلع على [سياسة الأمان](project.md#security) و[خارطة الطريق](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md) و[سجل التغييرات](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md).
