# النص والمحتوى

تُبنى صفحات القراءة، مثل المقالات والملفات الشخصية والقوائم، من بضعة مكوّنات صغيرة. وهي HTML عادي يُعرض على الخادم، بلا سكربت، وكل نص فيها يأتي من ملفات locale أو يُهرَّب (escaped).

@live content

## Heading

من `Heading::h2(text)` إلى `Heading::h6(text)`. عنوان الـ screen هو الـ `h1` الوحيد في الصفحة، لذلك لا يوجد `h1` هنا: اختر المستوى الذي يلي في بنية المستند، لا الحجم الذي تريده. يتيح `.id("anchor")` للروابط أن تشير إليه.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

فقرة من نص مُهرَّب. يستخدم `.muted()` لون النص الثانوي، ويستخدم `.small()` حجم الخط الصغير. تُلَفّ الكلمات الطويلة إلى السطر التالي.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

معلومات قصيرة في سطر واحد، مثل التاريخ ومدة القراءة والفئة. يُخفى الفاصل (نقطة وسطية ما لم يحدد `.separator("/")` غير ذلك) عن قارئات الشاشة، وتُحذف الأجزاء الفارغة.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

روابط إلى العنصر السابق والعنصر التالي، مع `rel="prev"` و`rel="next"`. تُفتح دون إعادة تحميل الصفحة. تأتي كلمتا «السابق» و«التالي» من نصوص الإطار؛ ويضيف `.prev_title` و`.next_title` عنوان العنصر تحتهما. تنعكس الأسهم في اللغات التي تُكتب من اليمين إلى اليسار، وعلى الهواتف يتراص الرابطان.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
    .shortcut("Alt+Shift+N")
```

### اختصارات الروابط

يمنح `.shortcut(keys)` اختصار لوحة مفاتيح للجانب الذي أُضيف أخيرًا، ويمنح `Chips::links(...).link(...).shortcut(keys)` اختصارًا للـ chip الرابط الذي أُضيف أخيرًا. الضغط على المفاتيح يتبع الرابط عبر `nav.js` دون إعادة تحميل الصفحة، في صفحات غلاف التطبيق وغلاف الموقع على السواء. لا تفعل المفاتيح شيئًا بينما يكون التركيز في حقل إدخال أو منطقة نص أو قائمة اختيار أو منطقة قابلة للتحرير، فلا يُستولى على الكتابة أبدًا. يحمل الرابط `aria-keyshortcuts` لقارئات الشاشة ويعرض المفاتيح في تلميح `<kbd>` تخفيه شاشات اللمس. تُقرأ الاختصارات كما يُقرأ `Command::shortcut`: الاختصارات التي يحتفظ بها المتصفح (`Ctrl+T` و`Ctrl+W`) أو يستخدمها (`Ctrl+P` و`F5`) لا تُربط. يبلّغ `App::check` عن اختصارات الروابط في خانات رأس الموقع التي لا يمكن قراءتها أو التي يملكها المتصفح أو التي هي مفتاح حرف واحد (WCAG 2.2، 2.1.4: يُعدّ `Shift+N` كذلك ولا يُعدّ `Alt+Shift+N`) أو التي تتعارض مع أمر أو مع اللوحة (palette) أو مع رابط آخر؛ ويفعل `App::check_page(&screen)` الأمر نفسه لشاشة بُنيت في اختبار مع معاملاتها، لأن `App::check` لا يستطيع بناء الشاشات بنفسه.

المفاتيح المضغوطة بينما التركيز داخل إطار من أصل آخر، مثل مشغّل فيديو مضمَّن، تذهب إلى مستند ذلك الإطار ولا تصل إلى الصفحة أبدًا؛ ولا يستطيع أي سكربت تغيير ذلك. أخبر المستخدمين بأن ينقروا خارج الإطار، أو يضغطوا Tab للخروج منه، قبل استخدام اختصار.

## AutoGrid

عناصر فرعية في شبكة يتبع عدد أعمدتها العرض المتاح: عدد الأعمدة التي تتسع بعرض `min` بكسل على الأقل (240 ما لم يُحدَّد)، وعمود واحد على الهاتف. يرتّبها المتصفح بالتعبير `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`، فلا توجد نقاط توقف ولا سكربت. وهي تحتوي أي عُقد: بطاقات، لوحات، شبكات. يميّزها الاسم عن `DataGrid` الذي يعرض صفوف البيانات.

```rust
AutoGrid::new()
    .min(220)
    .gap(4)
    .add(Card::new().title(t!("home.orders")))
    .add(Card::new().title(t!("home.stock")))
    .add(Card::new().title(t!("home.people")))
```

@live autogrid

## List

صفوف بعضها تحت بعض داخل `<ul>`. لكل `ListItem` عنوان، ويمكن أن تكون له صورة صغيرة في البداية (العناوين الآمنة نفسها لصور البطاقات: مسار أو `https:` أو `data:image/`) وعنوان فرعي ونص وصفي قصير في النهاية: نص، أو مكوّن مضمَّن مثل `RelativeTime`. مع `.link(screen, params)` يصبح الصف كله رابطًا واحدًا، فيكون محطة واحدة في ترتيب Tab مع حلقة تركيز مرئية، ويُفتح دون إعادة تحميل الصفحة. يلتف النص الطويل؛ وعلى الهواتف ينتقل النص الوصفي تحت العنوان، وفي الصفحات من اليمين إلى اليسار ينعكس الصف.

تحتفظ `List::new()` بالعناصر التي تُعطى لها. وتحمّل `List::of(action)` الصفوف عبر النوع نفسه من الـ action الذي يستخدمه DataGrid، وترقّم الصفحات مثل [شبكة البطاقات](catalog.md#card-grid): 20 صفًا مع زر "Show more" ما لم يحدد `paging` غير ذلك، وخيارات الفرز، والبحث على الخادم عبر `.search(text)`، والتحقق من صلاحية الوصول إلى الـ action لكل صفحة.

```rust
List::new()
    .title(t!("team.title"))
    .item(ListItem::new("Ada Lovelace").subtitle(t!("team.math")))
    .item(ListItem::new("Alan Turing").subtitle(t!("team.computing")).meta(t!("team.lead")))

List::of(search_people)
    .title(t!("people.title"))
    .item(|p: &Person| {
        ListItem::new(p.name.clone())
            .image(p.photo.clone())
            .subtitle(p.city.clone())
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name)
    .paging(Paging::pages(20))
```

@live list

## Avatar

صورة دائرية لشخص أو فريق، بحجم 32 أو 40 أو 64 بكسل (`AvatarSize::Small` و`Medium` و`Large`). الاسم مطلوب: فهو النص البديل ومصدر الأحرف الأولى التي تظهر عند عدم وجود صورة، أو عندما لا تكون الصورة على مضيف يسمح به `img-src` الخاص بالتطبيق (القاعدة نفسها المطبقة على `Image`). تتبع الأحرف الأولى الأحرف الكبيرة في اللغة، فيتحول `i` التركي إلى `İ`. وحين يُكتب الاسم بجوار الصورة الرمزية مباشرة، يخفيها `.decorative()` عن قارئات الشاشة حتى لا تقرأ الاسم مرتين.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

محتوى مخفي إلى أن يفتحه القارئ، مثل إجابة سؤال. وهو عنصر إفصاح أصلي (`<details>` و`<summary>`)، فيعمل مع لوحة المفاتيح وقارئات الشاشة دون أي سكربت، ويبقى المحتوى خارج ترتيب القراءة إلى أن يُفتح. التسمية يحددها التطبيق.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## Chips

صف من روابط صغيرة مستديرة تلتف على أكثر من سطر. يضع `Chips::links(label)` روابط عادية إلى الشاشات، مثل وسوم مقال، داخل `nav` سُمّي بالتسمية label. ويصنع `Chips::filter(label, screen, params)` أزرار تبديل على معامل شاشة: كل chip رابط إلى الشاشة نفسها تُضاف قيمته إلى المعامل أو تُزال منه، فيعرض الخادم `aria-pressed` من المعاملات التي استلمها ولا يُحفظ شيء في المتصفح. يفتح الضغط على chip العنوان الجديد عبر `nav.js` دون إعادة تحميل الصفحة، ويبقى التركيز وموضع التمرير على الـ chip. تُضغط قيمة واحدة في كل مرة ما لم يُستدعَ `.multiple()`؛ وتُتجاهل القيم في المعاملات التي ليست خيارات. لوحة المفاتيح: ينتقل Tab بين الـ chips، ويضغط Enter أو Space واحدًا منها، ويمكن أن يكون لـ chip رابط اختصاره الخاص (انظر [اختصارات الروابط](#link-shortcuts)). ويُظهر الـ chip المضغوط علامة صح أيضًا، فلا تعتمد الحالة على اللون وحده. يُحمَّل السكربت (`chips.js`، أقل من 1 KB) فقط في الصفحات التي فيها chips للتصفية.

```rust
#[screen(route = "/articles", public)]
pub fn articles(p: ArticleParams) -> Screen {
    Screen::new(t!("blog.articles")).body(
        Chips::filter(t!("blog.tags"), articles, |tags| ArticleParams { tags: tags.join(",") })
            .current(p.tags.split(','))
            .option("guide", t!("blog.guides"))
            .option("news", t!("blog.news"))
            .multiple(),
    )
}
```

@live chips


## Hero

قسم كبير في أعلى الصفحة: صورة خلفية مع عنوان وسطر نص وإجراءات مثل الأزرار. يضيف `.link(label, screen, params)` زر `LinkButton` إلى صفحة أخرى: الأول في الشريحة بمظهر أساسي، والتالية بمظهر ثانوي. وهو ثابت افتراضيًا. يضيف `.slide(title)` شريحة أخرى، ويضبط `.text` و`.image` و`.action` دائمًا الشريحة الأخيرة. ومع عدة شرائح يتبع نمط carousel في W3C APG: أزرار لاختيار الشريحة، وكل شريحة مجموعة اسمها "2 من 3"، وتُعلَن الشريحة المختارة بأدب. لا تتغير الشرائح إلا عندما يختار المستخدم، ما لم يستدعِ التطبيق `.rotate(seconds)` (من 4 إلى 30). عندها يأتي زر الإيقاف المؤقت والتشغيل أولًا (WCAG 2.2.2)، ويتوقف التدوير بينما يكون المؤشر فوق hero أو التركيز داخله وبينما تكون علامة التبويب مخفية، ولا يبدأ أبدًا إذا فضّل المستخدم الحركة المخفضة؛ ولا تُعلَن الشرائح أثناء تحركها. وبلا سكربت تظهر الشريحة الأولى.

تتبع الصورة قواعد `Image` (مسار من التطبيق أو عنوان `https` على مضيف ضمن `img-src` للتطبيق). وهي زخرفة خلف النص، لذا يكون نصها البديل فارغًا: ضع المهم في العنوان والنص، وهما على طبقة تعتيم داكنة تُبقي النص الأبيض بتباين لا يقل عن 4.5:1 فوق أي صورة. تحتفظ أزرار الروابط بألوانها الصلبة وتأخذ حلقة تركيز بيضاء فوق الصورة. تُحمَّل صورة الشريحة الأولى بأولوية عالية لرسم أول سريع، وتُحمَّل الباقية بتأجيل. يُحمَّل السكربت (`hero.js`، 1.6 KB gzip) فقط في الصفحات التي فيها شرائح.

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .link(t!("home.menu"), menu, ())
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero


## Reactions

صف من أزرار التفاعل يختارها التطبيق، لكل منها مفتاح واسم ورمز مثل emoji وعدّاد. يسمّي `Reactions::of(action, target)` العنصر الذي تتعلق به التفاعلات؛ وتحدد الشاشة الهدف ولا يُقرأ أبدًا من المتصفح. يرسل الضغط على الزر التفاعل إلى `POST /_zeyn/data`، الذي يتحقق من CSRF، ويُحتسب ضمن حد معدل الكتابة، ويفتح الشاشة، ويتحقق من إذن الإجراء، ويرفض أي تفاعل خارج المجموعة قبل تشغيل الإجراء. يتلقى الإجراء `ReactionPress` (target وreaction وpressed) ويحفظه ويعيد `ReactionState` الجديد (العدّادات وتفاعلات المستخدم نفسه)، وتتحدث العدّادات وحالات `aria-pressed` في مكانها. كل زر هو toggle button يُسمّى باسم التفاعل وعدّاده؛ والرمز مخفي عن قارئات الشاشة. يعطي `.state(...)` العدّادات للعرض الأول، فتظهر دون سكربت. يُحمَّل السكربت (`reactions.js`، 1.3 KB gzip) فقط في الصفحات التي فيها تفاعلات.

```rust
#[action(permission = "answers.react")]
pub async fn react_to_answer(press: ReactionPress, db: Db, ctx: Ctx) -> Result<ReactionState> {
    db.toggle_reaction(&ctx, &press.target, &press.reaction, press.pressed).await?;
    db.reaction_state(&ctx, &press.target).await
}

Reactions::of(react_to_answer, format!("answer-{}", answer.id))
    .reaction("helpful", t!("answers.helpful"), "\u{1f44d}")
    .reaction("thanks", t!("answers.thanks"), "\u{1f64f}")
    .state(answer.reactions.clone())
```

@live reactions


## RelativeTime

لحظة تُعرض بوصفها وقتًا نسبيًا، مثل "قبل 3 دقائق" أو "بعد يومين"، بلغة الصفحة. يكتب الخادم التاريخ والوقت الدقيقين بالمنطقة الزمنية للمستخدم داخل `<time datetime="...">`، فيبقى HTML نفسه من دقيقة إلى أخرى وتبقى الصفحة المخزنة مؤقتًا صحيحة. يعيد وقت التشغيل كتابة النص بـ `Intl.RelativeTimeFormat` عند تحميل الصفحة وعندما يُدخل `nav.js` شاشة وكل دقيقة؛ ويبقى الوقت الدقيق في tooltip. وبلا سكربت يكون النص هو الوقت الدقيق.

```rust
RelativeTime::new(post.published)
```


## Share

زر يشارك رابطًا: ورقة المشاركة في النظام حيث يوجد Web Share API في المتصفح (معظم الهواتف)، وإلا نسخ إلى الحافظة مع toast يبيّن ذلك. يشارك الصفحة المعروضة، أو شاشة عبر `Share::screen(screen, params)` التي يتحقق المترجم من معاملاتها. يضبط `.title` العنوان الذي تعرضه ورقة المشاركة، والتسمية هي "مشاركة" بلغة المستخدم ما لم يعطِ `.label` تسمية أخرى. يعرض الخادم الزر مخفيًا ويُظهره سكربته، فلا يكون في صفحة بلا سكربت زر لا يفعل شيئًا. يُحمَّل السكربت (`share.js`، أقل من 1 KB) فقط في الصفحات التي فيها زر مشاركة.

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


## AlphaIndex

شريط من الحروف، كل حرف منها رابط إلى screen يحمل الحرف في معاملاته، مثل الأشخاص الذين تبدأ أسماؤهم به. تأتي الحروف من لغة المستخدم بترتيبها الخاص (`zeyn::ui::alphabet`): في التركية تقع `Ç Ğ I İ Ö Ş Ü` في أماكنها، وفي الإسبانية توجد `Ñ`، وللروسية والعربية أبجديتاهما، وتحصل كل لغة أخرى على `A` إلى `Z`. يُعلَّم حرف الـ screen المعروض بـ `aria-current="page"` (يعلّم `i` التركي الحرف `İ`)، ويضيف `.other()` رابط `#` أخيرًا للأسماء التي تبدأ بأي شيء آخر. تُفتح الروابط دون إعادة تحميل الصفحة وتلتف على أسطر أكثر على الهواتف.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

سبعة أعمدة للأيام فيها صفوف من action، مثل حصص أسبوع أو ورديّاته أو فعالياته. الـ action من النوع نفسه الذي يستخدمه DataGrid ويُتحقق منه بالطريقة نفسها. يربط `.day` الصف بيومه: `Date` أو `Option<Date>` (تُستبعد الصفوف التي لا يوم لها) أو رقم يوم الأسبوع من 1 (الإثنين) إلى 7 (الأحد)؛ وأي شيء آخر لا يُصرَّف. يُعرض كل صف على هيئة `ListItem`، فيمكنه الارتباط بـ screen. يبدأ الأسبوع في أول أيام أسبوع المستخدم (الإثنين في التركية، الأحد في الإنجليزية الأمريكية)، ويُعلَّم اليوم الحالي، وتذكر الأيام التي بلا صفوف ذلك، وعلى الشاشات الضيقة تتراص الأيام. يحمّل الخادم حتى `limit` صفًا (100 ما لم يُحدَّد، وبحد أقصى 500) بترتيب الـ action؛ أعطِ الـ action صفوف الأسبوع، مثلًا بمرشح على التاريخ.

```rust
WeekSchedule::of(this_week_classes)
    .title(t!("classes.this_week"))
    .day(|c: &Class| c.date)
    .item(|c: &Class| {
        ListItem::new(c.name.clone())
            .meta(c.starts.to_string())
            .link(class, ClassParams { id: c.id })
    })
```

@live week
