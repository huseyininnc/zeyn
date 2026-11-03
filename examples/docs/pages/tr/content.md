# Metin ve içerik

Makale, profil ve liste gibi okuma sayfaları birkaç küçük bileşenden kurulur. Bunlar sunucuda render edilen düz HTML'dir, betik içermez ve her metin locale dosyalarından gelir ya da escape edilir.

@live content

## Heading

`Heading::h2(text)` ile `Heading::h6(text)` arası. Screen başlığı sayfanın tek `h1` öğesidir, bu yüzden burada `h1` yoktur: istediğiniz boyutu değil, ana hatta uyan seviyeyi seçin. `.id("anchor")` bağlantıların ona işaret etmesini sağlar.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

Escape edilmiş metinden bir paragraf. `.muted()` ikincil metin rengini, `.small()` küçük yazı boyutunu kullanır. Uzun sözcükler satır kaydırır.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

Tek satırda kısa bilgiler; örneğin bir tarih, bir okuma süresi ve bir kategori. Ayırıcı (`.separator("/")` başka bir şey söylemedikçe orta nokta) ekran okuyuculardan gizlenir ve boş parçalar atlanır.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

`rel="prev"` ve `rel="next"` ile önceki ve sonraki öğeye bağlantılar. Sayfa yenilenmeden açılırlar. "Previous" ve "Next" sözcükleri framework'ün metinlerinden gelir; `.prev_title` ve `.next_title` bunların altına öğenin başlığını ekler. Oklar sağdan sola dillerde ters döner ve telefonlarda iki bağlantı alt alta dizilir.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

Sütun sayısı kullanılabilir genişliğe uyan bir ızgaradaki alt öğeler: en az `min` piksellik (ayarlanmadıkça 240) sütunlardan sığabildiği kadar, telefonda ise tek sütun. Tarayıcı bunu `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` ile yerleştirir, bu yüzden kırılma noktası ve betik yoktur. Her türlü düğümü tutar: kartlar, paneller, grid'ler. Adı onu veri satırlarını gösteren `DataGrid`'den ayırır.

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

Bir `<ul>` içinde alt alta satırlar. Her `ListItem` bir başlığa sahiptir ve küçük bir baştaki resim (kart resimleriyle aynı güvenli adresler: bir yol, `https:` ya da `data:image/`), bir alt başlık ve sonda kısa meta metni içerebilir. `.link(screen, params)` ile satırın tamamı tek bir bağlantı olur; böylece sekme sırasında görünür odak halkasıyla tek bir duraktır ve sayfa yenilenmeden açılır. Uzun metin satır kaydırır; telefonlarda meta metni başlığın altına geçer ve sağdan sola sayfalar satırı yansıtır.

`List::new()` kendisine verilen öğeleri tutar. `List::of(action)` satırları bir DataGrid'in kullandığı türden bir action ile yükler ve bir [kart ızgarası](catalog.md#card-grid) gibi sayfalanır: `paging` başka bir şey söylemedikçe "Show more" düğmesiyle 20 satır, sıralama seçenekleri, `.search(text)` ile sunucu tarafında arama ve her sayfada action'ın erişim denetimi.

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

Bir kişinin ya da ekibin 32, 40 veya 64 piksellik yuvarlak resmi (`AvatarSize::Small`, `Medium`, `Large`). Ad zorunludur: alternatif metindir ve resim olmadığında ya da resim uygulamanın `img-src` kuralının izin verdiği bir host'ta olmadığında (`Image` ile aynı kural) gösterilen baş harflerin kaynağıdır. Baş harfler dilin büyük harf kurallarına uyar, bu yüzden Türkçe `i` harfi `İ` olur. Ad avatarın hemen yanında yazılıyorsa `.decorative()` onu ekran okuyuculardan gizler, böylece adı iki kez okumazlar.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

Okuyucu açana kadar gizli kalan içerik; örneğin bir sorunun cevabı. Yerel bir açılır öğedir (`<details>` ve `<summary>`), bu yüzden hiçbir betik olmadan klavye ve ekran okuyucularla çalışır ve içerik açılana kadar okuma sırasının dışında kalır. Etiket uygulamadan gelir.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

Her biri parametrelerinde o harfi taşıyan bir screen'e bağlantı olan bir harf çubuğu; örneğin adı o harfle başlayan kişiler. Harfler kullanıcının dilinden, o dilin kendi sırasıyla gelir (`zeyn::ui::alphabet`): Türkçede `Ç Ğ I İ Ö Ş Ü` yerindedir, İspanyolcada `Ñ` vardır, Rusça ve Arapçanın kendi alfabeleri vardır ve diğer her dil `A` ile `Z` arasını alır. Gösterilen screen'in harfi `aria-current="page"` ile işaretlenir (Türkçe bir `i`, `İ` harfini işaretler) ve `.other()` başka bir şeyle başlayan adlar için en sona bir `#` bağlantısı ekler. Bağlantılar sayfa yenilenmeden açılır ve telefonlarda birden fazla satıra kayar.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

Bir action'dan gelen satırlarla yedi gün sütunu; örneğin bir haftanın dersleri, vardiyaları ya da etkinlikleri. Action bir DataGrid'in kullandığı türdendir ve aynı şekilde denetlenir. `.day` bir satırı gününe eşler: bir `Date`, bir `Option<Date>` (günü olmayan satırlar atlanır) ya da 1 (Pazartesi) ile 7 (Pazar) arasında bir hafta günü numarası; başka bir şey derlenmez. Her satır bir `ListItem` olarak gösterilir, bu yüzden bir screen'e bağlanabilir. Hafta kullanıcının ilk hafta gününde başlar (Türkçede Pazartesi, Amerikan İngilizcesinde Pazar), bugün işaretlenir, satırı olmayan günler bunu belirtir ve dar ekranlarda günler alt alta dizilir. Sunucu action'ın sırasıyla en fazla `limit` satır (ayarlanmadıkça 100, en çok 500) yükler; action'a haftanın satırlarını verin, örneğin tarih üzerinde bir filtreyle.

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
