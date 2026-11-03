# النص والمحتوى

تُبنى صفحات القراءة، مثل المقالات والملفات الشخصية والقوائم، من بضعة مكونات صغيرة. وهي HTML عادي يُعرض على الخادم، بلا سكربت، ويمر كل نص عبر ملفات اللغة أو يُهرَّب.

@live content

## Heading

من `Heading::h2(text)` إلى `Heading::h6(text)`. عنوان الشاشة هو `h1` الوحيد في الصفحة، لذلك لا يوجد `h1` هنا: اختر المستوى الذي يتبع مخطط الصفحة، لا الحجم الذي تريده. يتيح `.id("anchor")` للروابط أن تشير إليه.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

فقرة من نص مُهرَّب. يستخدم `.muted()` لون النص الثانوي، ويستخدم `.small()` حجم الخط الصغير. تلتف الكلمات الطويلة إلى السطر التالي.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

معلومات قصيرة في سطر واحد، مثل تاريخ ومدة قراءة وفئة. يُخفى الفاصل (نقطة وسطى ما لم يحدد `.separator("/")` غير ذلك) عن قارئات الشاشة، وتُحذف الأجزاء الفارغة.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

روابط إلى العنصر السابق والعنصر التالي، مع `rel="prev"` و `rel="next"`. تُفتح دون إعادة تحميل الصفحة. تأتي الكلمتان "Previous" و "Next" من نصوص إطار العمل؛ ويضيف `.prev_title` و `.next_title` عنوان العنصر تحتهما. تنقلب الأسهم في اللغات التي تُكتب من اليمين إلى اليسار، وعلى الهواتف يتراص الرابطان أحدهما فوق الآخر.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

عناصر أبناء في شبكة يتبع عدد أعمدتها العرض المتاح: أكبر عدد يتسع من الأعمدة التي لا يقل عرضها عن `min` بكسل (240 ما لم يُحدَّد)، وعمود واحد على الهاتف. يرتبها المتصفح بالقاعدة `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`، فلا توجد نقاط توقف ولا سكربت. تتسع لأي عقد: بطاقات ولوحات وجداول. يميزها الاسم عن `DataGrid` الذي يعرض صفوفاً من البيانات.

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

صفوف بعضها تحت بعض داخل `<ul>`. لكل `ListItem` عنوان، ويمكن أن تكون له صورة صغيرة في البداية (العناوين الآمنة نفسها التي لصور البطاقات: مسار أو `https:` أو `data:image/`)، وعنوان فرعي، ونص وصفي قصير في النهاية. مع `.link(screen, params)` يصبح الصف كله رابطاً واحداً، فيكون محطة واحدة في ترتيب التنقل بمفتاح Tab مع حلقة تركيز ظاهرة، ويُفتح دون إعادة تحميل الصفحة. يلتف النص الطويل؛ وعلى الهواتف ينتقل النص الوصفي تحت العنوان، وتعكس الصفحات المكتوبة من اليمين إلى اليسار الصف.

يحمل `List::new()` العناصر التي تُعطى له. ويحمّل `List::of(action)` الصفوف عبر النوع نفسه من الـ action الذي يستخدمه DataGrid، ويقسّمها إلى صفحات مثل [شبكة البطاقات](catalog.md#card-grid): 20 صفاً مع زر "Show more" ما لم يحدد `paging` غير ذلك، وخيارات ترتيب، وبحث على الخادم عبر `.search(text)`، مع فحص صلاحية الوصول إلى الـ action لكل صفحة.

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
            .meta(p.joined.format(&locale))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

صورة دائرية لشخص أو فريق، بحجم 32 أو 40 أو 64 بكسل (`AvatarSize::Small` و `Medium` و `Large`). الاسم مطلوب: فهو النص البديل ومصدر الأحرف الأولى التي تظهر حين لا توجد صورة، أو حين لا تكون الصورة على مضيف يسمح به `img-src` في التطبيق (القاعدة نفسها التي في `Image`). تتبع الأحرف الأولى قواعد الأحرف الكبيرة في اللغة، فيصبح الحرف التركي `i` هو `İ`. وحين يُكتب الاسم بجوار الصورة الرمزية مباشرة، يخفيها `.decorative()` عن قارئات الشاشة حتى لا تقرأ الاسم مرتين.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

محتوى مخفي حتى يفتحه القارئ، مثل إجابة سؤال. وهو عنصر إفصاح أصلي (`<details>` و `<summary>`)، لذلك يعمل مع لوحة المفاتيح وقارئات الشاشة دون أي سكربت، ويبقى المحتوى خارج ترتيب القراءة حتى يُفتح. تأتي التسمية من التطبيق.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

شريط من الحروف، كل حرف فيه رابط إلى شاشة تحمل ذلك الحرف في معاملاتها، مثل الأشخاص الذين تبدأ أسماؤهم به. تأتي الحروف من لغة المستخدم بترتيبها الخاص (`zeyn::ui::alphabet`): في التركية تكون `Ç Ğ I İ Ö Ş Ü` في مواضعها، وفي الإسبانية يوجد `Ñ`، وللروسية والعربية أبجديتاهما، وتحصل كل لغة أخرى على الحروف من `A` إلى `Z`. يُعلَّم حرف الشاشة المعروضة بالسمة `aria-current="page"` (الحرف التركي `i` يعلّم `İ`)، ويضيف `.other()` رابط `#` أخيراً للأسماء التي تبدأ بأي شيء آخر. تُفتح الروابط دون إعادة تحميل الصفحة وتلتف على عدة أسطر في الهواتف.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

سبعة أعمدة للأيام فيها صفوف من action، مثل حصص أسبوع أو مناوباته أو فعالياته. الـ action من النوع نفسه الذي يستخدمه DataGrid ويُفحص بالطريقة نفسها. يربط `.day` الصف بيومه: `Date`، أو `Option<Date>` (تُحذف الصفوف التي ليس لها تاريخ)، أو رقم يوم في الأسبوع من 1 (الاثنين) إلى 7 (الأحد)؛ وأي شيء آخر لا يُترجَم. يُعرض كل صف على أنه `ListItem`، لذلك يمكن أن يرتبط بشاشة. يبدأ الأسبوع في أول أيام الأسبوع لدى المستخدم (الاثنين في التركية، والأحد في الإنجليزية الأمريكية)، ويُعلَّم اليوم الحالي، وتذكر الأيام التي لا صفوف فيها ذلك، وعلى الشاشات الضيقة تتراص الأيام بعضها تحت بعض. يحمّل الخادم ما يصل إلى `limit` صف (100 ما لم يُحدَّد، و500 على الأكثر) بترتيب الـ action؛ فأعطِ الـ action صفوف الأسبوع، مثلاً بمرشح على التاريخ.

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
