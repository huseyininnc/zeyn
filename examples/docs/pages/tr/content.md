# Metin ve içerik

Makale, profil ve liste gibi okuma sayfaları birkaç küçük bileşenden kurulur. Bunlar sunucuda üretilen düz HTML'dir, script içermez ve her metin locale dosyalarından gelir ya da kaçışlanır.

@live content

## Heading

`Heading::h2(text)` ile `Heading::h6(text)` arası. Screen başlığı sayfanın tek `h1`'idir, bu yüzden burada `h1` yoktur: istediğiniz boyutu değil, anahatta sıradaki seviyeyi seçin. `.id("anchor")` bağlantıların ona işaret etmesini sağlar.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

Kaçışlanmış metinden oluşan bir paragraf. `.muted()` ikincil metin rengini, `.small()` küçük yazı boyutunu kullanır. Uzun sözcükler satır sonunda kırılır.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

Tarih, okuma süresi ve kategori gibi kısa bilgileri tek satırda gösterir. Ayırıcı (`.separator("/")` ile değiştirilmediyse bir orta nokta) ekran okuyuculardan gizlenir ve boş parçalar atlanır.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

Önceki ve sonraki öğeye `rel="prev"` ve `rel="next"` ile bağlantılar verir. Sayfa yenilenmeden açılırlar. "Önceki" ve "Sonraki" sözcükleri framework'ün metinlerinden gelir; `.prev_title` ve `.next_title` öğenin başlığını bunların altına ekler. Okler sağdan sola yazılan dillerde ters döner ve telefonlarda iki bağlantı alt alta dizilir.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

Sütun sayısı kullanılabilir genişliğe göre değişen bir ızgarada alt öğeler: en az `min` piksel (ayarlanmadıysa 240) genişliğinde sığabildiği kadar sütun, telefonda tek sütun. Tarayıcı bunu `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` ile yerleştirir, yani kesme noktası ve script yoktur. Kartlar, paneller, ızgaralar gibi her düğümü alır. Adı, veri satırlarını gösteren `DataGrid`'den ayırmak içindir.

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

Alt alta dizilen satırlar, bir `<ul>` içinde. Her `ListItem`'ın bir başlığı vardır; küçük bir baştaki görseli (kart görselleriyle aynı güvenli adresler: bir yol, `https:` ya da `data:image/`), bir alt başlığı ve sonda kısa bir meta metni olabilir. `.link(screen, params)` ile satırın tamamı tek bir bağlantı olur: sekme sırasında tek durak, görünür odak halkası vardır ve sayfa yenilenmeden açılır. Uzun metin satır kırar; telefonlarda meta metin başlığın altına geçer, sağdan sola yazılan sayfalarda satır aynalanır.

`List::new()` kendisine verilen öğeleri tutar. `List::of(action)` satırları DataGrid'in kullandığı türden bir action ile yükler ve [kart ızgarası](catalog.md#card-grid) gibi sayfalar: `paging` başka türlü söylemedikçe 20 satır ve bir "Show more" düğmesi, sıralama seçenekleri, `.search(text)` ile sunucu tarafı arama ve her sayfa için action'ın erişim denetimi.

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

Bir kişinin ya da ekibin yuvarlak resmi; 32, 40 ya da 64 piksel (`AvatarSize::Small`, `Medium`, `Large`). Ad zorunludur: alternatif metindir ve resim yokken ya da resim uygulamanın `img-src` ayarının izin verdiği bir sunucuda değilken gösterilen baş harflerin kaynağıdır (`Image` ile aynı kural). Baş harfler dilin büyük harflerini izler, yani Türkçede `i` `İ` olur. Ad avatarın hemen yanında yazılıysa `.decorative()` onu ekran okuyuculardan gizler, böylece ad iki kez okunmaz.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

Okuyucu açana kadar gizli kalan içerik; örneğin bir sorunun cevabı. Yerel bir açılır bölümdür (`<details>` ve `<summary>`), bu yüzden script olmadan klavye ve ekran okuyucularla çalışır ve içerik açılana kadar okuma sırasının dışında kalır. Etiket uygulamadan gelir.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

Her biri, içinde harfin parametre olarak bulunduğu bir screen'e bağlantı olan harflerden oluşan bir çubuk; örneğin adı o harfle başlayan kişiler. Harfler kullanıcının dilinden ve o dilin kendi sırasıyla gelir (`zeyn::ui::alphabet`): Türkçede `Ç Ğ I İ Ö Ş Ü` yerindedir, İspanyolcada `Ñ` vardır, Rusça ve Arapçanın kendi alfabeleri vardır, diğer her dil `A`'dan `Z`'ye alır. Gösterilen screen'in harfi `aria-current="page"` ile işaretlenir (Türkçe `i`, `İ`'yi işaretler) ve `.other()`, başka bir şeyle başlayan adlar için sona bir `#` bağlantısı ekler. Bağlantılar sayfa yenilenmeden açılır ve telefonlarda birkaç satıra sarılır.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

Bir action'dan gelen satırların yedi günlük sütunu; örneğin bir haftanın dersleri, vardiyaları ya da etkinlikleri. Action, DataGrid'in kullandığı türdendir ve aynı şekilde denetlenir. `.day` bir satırı gününe eşler: bir `Date`, bir `Option<Date>` (günü olmayan satırlar dışarıda kalır) ya da 1 (Pazartesi) ile 7 (Pazar) arası bir hafta günü numarası; başka bir şey derlenmez. Her satır bir `ListItem` olarak gösterilir, dolayısıyla bir screen'e bağlanabilir. Hafta kullanıcının ilk gününde başlar (Türkçede Pazartesi, Amerikan İngilizcesinde Pazar), bugün işaretlenir, satırı olmayan günler bunu söyler ve dar ekranlarda günler alt alta dizilir. Sunucu action'ın sırasıyla en fazla `limit` satır yükler (ayarlanmadıysa 100, en çok 500); action'a haftanın satırlarını verin, örneğin tarih üzerinde bir süzgeçle.

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
