# العرض (Display)

مكونات تعرض الحالة: الرسائل والشارات والتقدم والأزواج مفتاح وقيمة والسجل والعناصر النائبة. تُعرض على الخادم ولا تحتاج إلى طلبات خاصة بها.

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info` و `success` و `warning` و `danger`، أو `Alert::new(tone, text)`. يضيف `.title(text)` عنواناً، ويضيف `.dismissible()` زر إغلاق. وتُنطق النبرة (tone) أيضاً قبل الرسالة، فلا تُعطى باللون وحده أبداً.

## Badge

`Badge::new(text).tone(Tone::...)`. النبرات: `Neutral` و `Info` و `Success` و `Warning` و `Danger` و `Brand`. تعرض الجداول القيم كشارات بالاستدعاء `.badge(|v| tone)`.

## Progress

`Progress::new(label, value, max)` مع `.tone(...)` اختيارية. وهو `progressbar` حقيقي بتسميته وقيمته.

## DescriptionList

أزواج مفتاح وقيمة، من حقول سجل أو يدوياً:

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

تُنسَّق القيم مثل خلايا الجدول: المبالغ والتواريخ والخيارات بلغة المستخدم.

## Timeline

`Timeline::new().event(moment, title, detail, tone)` يسرد الأحداث. تظهر اللحظات بالمنطقة الزمنية للمستخدم ولغته.

## EmptyState و Skeleton

`EmptyState::new(title).text(text)` يملأ منطقة فارغة بخطوة تالية: يذهب `.link(label, screen, params)` إلى صفحة أخرى عبر `LinkButton`، ويفتح `.opens(label, screen, params)` شاشة من التطبيق كنافذة. و`Skeleton::lines(n)` عنصر نائب أثناء تحميل شيء ما.

## LinkButton

الذهاب إلى مكان ما رابط، والقيام بشيء ما زر. `LinkButton::new(label, screen, params)` هو `<a href>` حقيقي إلى المسار القانوني للشاشة (مثل `/products/42`) يبدو كزر، فيتعامل معه النقر الأوسط و«فتح في علامة تبويب جديدة» ومحركات البحث على أنه الرابط الذي هو عليه. يُفتح عبر `nav.js` دون إعادة تحميل الصفحة، ويتبعه Enter. المظهر ثانوي ما لم يطلب `.primary()` المظهر المملوء للإجراء الرئيسي. يجب أن يكون للشاشة مسار، وإلا لا تُترجم الشيفرة. يستخدمه `EmptyState::link` و`Hero::link`.

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)` زر يزيل كل تفضيلات العميل التي يحتفظ بها هذا المتصفح للتطبيق، كما في صفحة الخصوصية بجانب القائمة من `App::client_prefs_table()`. ويؤكد ذلك إشعار toast. لا تُمَسّ ملفات تعريف الارتباط التي يضبطها الخادم (السمة واللغة والجلسة). ويمكن للسكربتات فعل الشيء نفسه بـ `zeyn.prefs.clear()`.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: الروابط مراجع شاشات مكتوبة الأنواع، فلا تترك شاشة أُعيدت تسميتها رابطاً مكسوراً.

## ActionButton

يشغّل `ActionButton::new(label, action, input)` action. يسأل `.confirm(text)` أولاً، ويعلّم `.tone(Tone::Danger)` الإجراء المدمّر، ويشغّل `.on_success(|result, ui| ...)` التأثيرات (effects). يُثبَّت المدخل (input) على الخادم ولا يأتي من العميل أبداً؛ وتُفحص صلاحية الـ action على الخادم كأي نموذج.

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

يعرض `Markdown::new(text)` نصاً منسقاً، مثل تعليق أو وصف منتج أو مساعدة. النص سلسلة نصية أو مفتاح لغة (locale key). ويحتاج إلى الميزة `markdown` (`zeyn = { features = ["markdown"] }`) التي لا تضيف أي اعتمادات.

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- العناوين والفقرات والقوائم والجداول وكتل الشيفرة والاقتباسات والخطوط الفاصلة والشيفرة المضمّنة والروابط والصور والخط الغامق والمائل.
- لا يُمرَّر HTML الخام أبداً: يظهر `<script>` كنص، وكتل الشيفرة تُهرَّب (escaped) أيضاً.
- لا يجوز أن تشير الروابط إلا إلى `http` أو `https` أو `mailto` أو مرساة (anchor) أو مسار نسبي. أما الأخرى، مثل `javascript:` أو `data:`، فلا يبقى منها إلا نص الرابط. وتحصل روابط المستخدمين على `rel="noopener noreferrer nofollow ugc"`.
- لا تُحمَّل الصور إلا من التطبيق نفسه (مسار نسبي)؛ وتعرض الصور الأخرى نصها البديل.
- `.trusted()` للنص الذي كتبه التطبيق: تحصل العناوين على معرّفات (ids) للروابط إليها، وتفقد الروابط `nofollow ugc`. أما التهريب وقواعد الروابط فتبقى كما هي.
- النص الذي يتجاوز 64 KB (`MAX_MARKDOWN`) يُقطع، ويُحدّ التداخل، فيبقى عرض المدخلات الخبيثة رخيصاً.

## Embed

يعرض `Embed` صفحة من موقع آخر، مثل مشغّل فيديو أو خريطة، داخل إطار مقفل. العنوان (title) مطلوب (قارئات الشاشة تعلن الإطارات به)؛ وبدون `.title(...)` لا يمكن وضع الـ embed ويقول المصرّف ذلك.

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- يجب أن يكون المضيف في `frame-src` الخاص بالتطبيق: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` (انظر [الأمان](security.md#csrf-csp-and-headers)). وعندما لا يكون العنوان `https` أو لا يكون مضيفه مدرجاً، لا يُعرض أي إطار؛ ويقول تحذير إن المحتوى لا يمكن عرضه.
- يحصل كل إطار على `sandbox="allow-scripts allow-same-origin allow-presentation"` و `referrerpolicy="no-referrer"` و `loading="lazy"`. لا يمكن السماح بالنوافذ المنبثقة ولا بالتنقل في النافذة العليا ولا بالنماذج ولا بالتنزيلات أبداً. ويضيف `.allow(...)` ميزات المتصفح: `Fullscreen` و `PictureInPicture` و `Autoplay` و `EncryptedMedia`.
- يعرض `.click_to_load(poster)` زراً أولاً، مع صورة الملصق (مسار على التطبيق أو مضيف من `img-src`؛ و`""` لعدم وجود صورة) واسم الموقع الآخر. لا يُتصل بالموقع الآخر حتى يُضغط الزر؛ عندها يحل الإطار مكانه ويتلقى التركيز. السكربت جزء من `media.js`، فلا يكلّف طلباً إضافياً.
- يضع `.notice(t!(...))` نصاً من التطبيق على ذلك الزر، مثل تنبيه حول المحتوى. يُهرَّب، ويُترجم كأي نص، ويكون جزءاً من الاسم المتاح (accessible name) للزر، فتقرؤه قارئات الشاشة قبل أن يقرر المستخدم. وبدون `.click_to_load` فإنه يشغّل الزر بلا ملصق.
- يسأل `.remember(EXTERNAL_VIDEO)` مرة واحدة في عمر المفتاح بدلاً من كل صفحة. المفتاح هو مفتاح موافقة (consent) معلن بـ `ClientPref::consent` ومسجَّل بـ `App::client_prefs` (انظر [تفضيلات العميل](security.md#client-preferences))؛ والـ embeds المرتبطة بالمفتاح نفسه تتشارك الإجابة. الصفحات اللاحقة والتنقلات داخل التطبيق والتبويبات الأخرى تحمّل تلك الإطارات من تلقاء نفسها، دون نقل التركيز (WCAG 2.2 3.2.1 On Focus و 2.4.3 Focus Order)؛ ولا ينتقل التركيز إلى الإطار إلا بعد ضغطة حقيقية. يعود الزر عندما ينتهي السجل أو يفشل في فحوصه، وعندما لا يكون التطبيق قد سجّل المفتاح، وعندما يكون التخزين محجوباً والصفحة جديدة، وبدون JavaScript. والنص العادي أو مفتاح الاختيار (choice) لا يُترجم.
- الحقل الذي يحمل عنواناً للتضمين يأخذ `#[rule(embed)]`. تقبل النماذج عنوان URL أو شيفرة تضمين ملصقة، وتحتفظ بـ `src` فقط، وتشترط `https` ومضيفاً مدرجاً، وتسقط معاملات التتبع (`utm_*` و `fbclid` و `gclid` وغيرها) وتخزن العنوان القانوني (canonical)، لا HTML أبداً. ويجب أن ترسل مدخلات API العنوان القانوني.

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

يحتاج `Image` إلى نصه البديل وحجمه؛ وبدون `.alt(...)` و `.size(width, height)` لا يمكن وضعه ويقول المصرّف ذلك. يحفظ الحجم المساحة أثناء تحميل الملف، فلا تتحرك الصفحة.

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` مسار على التطبيق (`/static/logo.png`) أو عنوان `https` على مضيف من `img-src` الخاص بالتطبيق (`App::csp(|c| c.img_src([...]))`). أي صورة أخرى لا تُعرض ويظهر نصها البديل بدلاً منها، فلا يسجل المتصفح أي طلب محجوب.
- تحصل كل صورة على `decoding="async"`؛ ويضيف `.lazy()` الخاصية `loading="lazy"` للصور التي تقع أسفل الشاشة. يقبل `srcset` واصفات الكثافة (`2x`) أو العرض (`800w`)؛ والمدخلات على مضيفات أخرى أو بواصفات أخرى تُترك. وتضيف واصفات العرض `sizes` انطلاقاً من عرض الصورة.
- الصورة الزخرفية البحتة تأخذ نصاً بديلاً فارغاً: `.alt(Text::from(""))`.

## ExternalLink

يربط `ExternalLink::new(url, text)` بموقع آخر مع `rel="noopener noreferrer"`، فلا يحصل الموقع الآخر على عنوان الصفحة ولا على مقبض للنافذة. تسمع قارئات الشاشة "external site" بعد النص؛ ويدل سهم عليه بصرياً.

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

يضيف `.user_content()` الخاصية `nofollow ugc` للعناوين التي كتبها المستخدمون. ويفتح `.new_tab()` تبويباً جديداً ويخبر بذلك قارئات الشاشة؛ وتبقى الروابط في التبويب نفسه ما لم يُضبط ذلك. العناوين `https` و `http` فقط تصبح روابط؛ وأي شيء آخر، مثل `javascript:`، يُعرض كنص عادي.

## Tooltip و popover و busy overlay

التلميح (tooltip) وصف قصير لعنصر تحكم واحد: `ActionButton::tip(text)` و `ToolButton::tip(text)` في شريط أدوات الشاشة و `Form::tip(field, text)` الذي يضيف زر معلومات بعد تسمية الحقل. النص هو `aria-describedby` لعنصر التحكم، فتسمعه قارئات الشاشة معه. يظهر بعد تمرير قصير للمؤشر وفوراً عند التركيز، ويبقى ما دام المؤشر فوقه (WCAG 1.4.13)، ويختفي بـ Escape دون نقل التركيز، ويظهر على اللمس بعد ضغطة طويلة (نقرة على زر المعلومات). لا يحمل التلميح أبداً معلومة يجب أن يحصل عليها المستخدم: ضعها في التسمية أو في قواعد الحقل أو في الصفحة.

`Popover::new(label).title(title).add(node)` زر يفتح حواراً غير مشروط (non-modal) بجانبه، لتفاصيل صغيرة. ينتقل التركيز إليه؛ ويغلقه Escape أو زر الإغلاق أو نقرة خارجه ويعود التركيز إلى الزر. وعلى الشاشات الضيقة يُفتح كورقة سفلية (bottom sheet). أبناؤه عقد (nodes) عادية، فالنموذج داخله يرسل إلى مسار عقدته الخاص مع كل فحوص الخادم.

يغطي `Busy::new().add(node)` محتواه أثناء تنفيذ طلب بداخله: نموذج يحفظ، أو زر action، أو جدول أو شجرة تُحمَّل. تحصل الحاوية على `aria-busy` فوراً؛ ولا يظهر المؤشر الدوار والنص "Working" (رسالة حالة) إلا بعد 400 ms، أو `.delay_ms(n)`، فلا تومض الإجابات السريعة، ويبقيان 300 ms على الأقل متى ظهرا. تحجب الطبقة المؤشر لا لوحة المفاتيح، ويبقى التركيز حيث كان.

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| المفتاح | أين | ماذا يفعل |
|---|---|---|
| Tab | عنصر تحكم له تلميح | يُظهر التركيز التلميح؛ ومغادرته تخفيه |
| Escape | في أي مكان أثناء ظهور تلميح | يخفي التلميح؛ ويبقى التركيز |
| Enter, Space | زر popover | يفتح الـ popover؛ وينتقل التركيز إلى أول عنصر تحكم فيه |
| Escape | داخل الـ popover | يغلقه؛ ويعود التركيز إلى الزر |
| Tab | داخل الـ popover | ينتقل بين عناصر التحكم فيه؛ ومغادرته تغلقه |

تحتاج الثلاثة إلى نحو 2.2 KB من السكربت بعد ضغط gzip ولا تحتاج إلى طلب خاص بها؛ التلميح عقدة واحدة، والـ popover ست عقد زائد محتواه، والـ busy overlay ثلاث.

## الأزرار المنسدلة والمنقسمة

يفتح `MenuButton::new(label)` قائمة أوامر؛ ويشغّل `MenuButton::split(label, action, input)` action واحداً من جزئه الرئيسي ويفتح القائمة من سهمه. الأوامر هي `.action(label, action, input)` (أو `.action_then(...)` مع تأثيرات بعد النجاح)، و`.item(label, effect)` للتأثيرات مثل فتح نافذة، و`.separator()`. ويسأل `.confirm(text)` قبل تشغيل آخر أمر action.

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

أوامر الـ action عقد لها مسارها الخاص وترسل إلى نقطة نهاية النموذج مثل `ActionButton`: يُفحص في كل استدعاء وصول الشاشة وحراس الميزة وصلاحية الـ action والمعترضات و CSRF وحد معدل الكتابة، ويبقى المدخل على الخادم. الأوامر التي لا يحق للمستخدم تشغيلها تُحذف من القائمة (أما الـ action الرئيسي للزر المنقسم فيُعطَّل بدلاً من ذلك)، ويظل الخادم يرفضها إذا استُدعيت.

@live menus

| المفتاح | أين | ماذا يفعل |
|---|---|---|
| Enter, Space, Arrow Down | الزر | يفتح القائمة على أول أمر |
| Arrow Up | الزر | يفتح القائمة على آخر أمر |
| Arrow Down, Arrow Up | القائمة | الأمر التالي والسابق، بالدوران |
| Home, End | القائمة | أول أمر وآخر أمر |
| الحروف | القائمة | الأمر التالي الذي يبدأ بها |
| Enter, Space | أمر | يشغّله ويغلق القائمة |
| Escape | القائمة | يغلقها؛ ويعود التركيز إلى الزر |
| Tab | القائمة | يغلقها وينتقل إلى ما بعدها |

تحتاج القائمة إلى نحو 0.8 KB من السكربت بعد ضغط gzip ولا تحتاج إلى طلب حتى يُشغَّل أمر؛ وهي 4 عقد زائد عقدتين لكل أمر.
