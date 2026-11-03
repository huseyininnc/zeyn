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
```

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

صفوف بعضها تحت بعض داخل `<ul>`. لكل `ListItem` عنوان، ويمكن أن تكون له صورة صغيرة في البداية (العناوين الآمنة نفسها لصور البطاقات: مسار أو `https:` أو `data:image/`) وعنوان فرعي ونص وصفي قصير في النهاية. مع `.link(screen, params)` يصبح الصف كله رابطًا واحدًا، فيكون محطة واحدة في ترتيب Tab مع حلقة تركيز مرئية، ويُفتح دون إعادة تحميل الصفحة. يلتف النص الطويل؛ وعلى الهواتف ينتقل النص الوصفي تحت العنوان، وفي الصفحات من اليمين إلى اليسار ينعكس الصف.

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
            .meta(p.joined.format(&locale))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
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
