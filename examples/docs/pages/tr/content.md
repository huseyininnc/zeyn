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
    .shortcut("Alt+Shift+N")
```

### Bağlantı kısayolları

`.shortcut(keys)` en son eklenen yan bağlantıya, `Chips::links(...).link(...).shortcut(keys)` ise en son eklenen bağlantı chip'ine bir klavye kısayolu verir. Tuşlara basmak bağlantıyı `nav.js` ile sayfa yenilemeden izler; uygulama shell'i ve site shell'i sayfalarında aynı şekilde çalışır. Odak bir girdide, metin alanında, seçim kutusunda ya da düzenlenebilir bir alandayken tuşlar hiçbir şey yapmaz, böylece yazma hiçbir zaman ele geçirilmez. Bağlantı ekran okuyucular için `aria-keyshortcuts` taşır ve tuşları, dokunmatik ekranların gizlediği bir `<kbd>` ipucunda gösterir. Kısayollar `Command::shortcut` gibi okunur: tarayıcının kendine ayırdığı (`Ctrl+T`, `Ctrl+W`) ya da kullandığı (`Ctrl+P`, `F5`) kısayollar bağlanmaz. `App::check`, site üst bilgisi slotlarındaki okunamayan, tarayıcıya ait olan, tek karakterli tuş olan (WCAG 2.2, 2.1.4: `Shift+N` sayılır, `Alt+Shift+N` sayılmaz) ya da bir komutla, paletle veya başka bir bağlantıyla çakışan bağlantı kısayollarını bildirir; `App::check` screen'leri kendisi kuramadığından, `App::check_page(&screen)` aynısını bir testte parametreleriyle kurulmuş bir screen için yapar.

Odak başka bir kökene ait bir çerçevenin, örneğin gömülü bir video oynatıcının içindeyken basılan tuşlar o çerçevenin belgesine gider ve sayfaya hiç ulaşmaz; hiçbir betik bunu değiştiremez. Kullanıcılara, kısayol kullanmadan önce çerçevenin dışına tıklamalarını ya da Tab ile çıkmalarını söyleyin.

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

Alt alta dizilen satırlar, bir `<ul>` içinde. Her `ListItem`'ın bir başlığı vardır; küçük bir baştaki görseli (kart görselleriyle aynı güvenli adresler: bir yol, `https:` ya da `data:image/`), bir alt başlığı ve sonda kısa bir meta olabilir: metin ya da `RelativeTime` gibi bir satır içi bileşen. `.link(screen, params)` ile satırın tamamı tek bir bağlantı olur: sekme sırasında tek durak, görünür odak halkası vardır ve sayfa yenilenmeden açılır. Uzun metin satır kırar; telefonlarda meta metin başlığın altına geçer, sağdan sola yazılan sayfalarda satır aynalanır.

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
            .meta(RelativeTime::new(p.joined))
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

## Chips

Birden çok satıra sarılan küçük yuvarlak bağlantılardan oluşan bir sıra. `Chips::links(label)`, bir makalenin etiketleri gibi ekranlara giden düz bağlantıları, etiketiyle adlandırılmış bir `nav` içinde tutar. `Chips::filter(label, screen, params)`, bir ekran parametresi üzerinde aç/kapa düğmeleri yapar: her chip aynı ekrana giden bir bağlantıdır, değeri parametreye eklenir ya da çıkarılır; böylece sunucu `aria-pressed` değerini aldığı parametrelerden üretir ve tarayıcıda hiçbir şey tutulmaz. Bir chip'e basmak yeni adresi `nav.js` ile sayfa yenilemeden açar, odak ve kaydırma konumu chip üzerinde kalır. `.multiple()` çağrılmadıkça aynı anda tek değer basılı olur; parametrelerde seçenek olmayan değerler yok sayılır. Klavye: Tab chip'ler arasında gezer, Enter veya Space birine basar ve bir bağlantı chip'inin kendi kısayolu olabilir (bkz. [Bağlantı kısayolları](#link-shortcuts)). Basılı bir chip ayrıca onay işareti gösterir, böylece durum yalnızca renge dayanmaz. Betik (`chips.js`, 1 KB altında) yalnızca filtre chip'i olan sayfalarda yüklenir.

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

Bir sayfanın üstünde büyük bir bölüm: bir başlık, bir metin satırı ve düğme gibi eylemlerle arka plan görseli. `.link(label, screen, params)` başka bir sayfaya bir `LinkButton` ekler: bir slaytın ilki birincil görünümde, sonrakiler ikincil görünümde olur. Varsayılan olarak durağandır. `.slide(title)` bir slayt daha ekler; `.text`, `.image` ve `.action` her zaman sonuncuyu ayarlar. Birden çok slaytla W3C APG carousel desenini izler: slayt seçici düğmeler, "2 / 3" adlı bir grup olan her slayt ve nazikçe duyurulan seçili slayt. Uygulama `.rotate(seconds)` (4 ile 30) çağırmadıkça slaytlar yalnızca kullanıcı seçince değişir. Çağrılırsa önce bir duraklat ve oynat düğmesi gelir (WCAG 2.2.2), işaretçi hero üzerindeyken, odak içindeyken ve sekme gizliyken döndürme durur, kullanıcı azaltılmış hareketi tercih ediyorsa hiç başlamaz; slaytlar hareket ederken duyurulmaz. Betik olmadan ilk slayt görünür.

Görsel `Image` kurallarını izler (uygulamanın bir yolu ya da uygulamanın `img-src` listesindeki bir ana bilgisayarda `https` adresi). Metnin arkasında bir süstür, bu yüzden alternatif metni boştur: önemli olanı, herhangi bir resmin üzerinde beyaz metni en az 4,5:1 kontrastta tutan koyu bir perdenin üzerinde duran başlığa ve metne yazın. Bağlantı düğmeleri dolu renklerini korur ve görselin üzerinde beyaz bir odak halkası alır. İlk slaytın görseli hızlı ilk çizim için yüksek öncelikle, diğerleri tembel yüklenir. Betik (`hero.js`, 1,6 KB gzip) yalnızca slaytı olan sayfalarda yüklenir.

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

Uygulamanın seçtiği, her biri bir anahtar, bir ad ve emoji gibi bir sembol ile sayısı olan tepki düğmelerinden oluşan bir sıra. `Reactions::of(action, target)`, tepkilerin hangi öğe hakkında olduğunu belirtir; hedefi ekran belirler ve tarayıcıdan asla okunmaz. Bir düğmeye basmak tepkiyi `POST /_zeyn/data` adresine gönderir; bu uç nokta CSRF'yi kontrol eder, yazma hız sınırına sayılır, ekranı açar, eylemin iznini kontrol eder ve eylem çalışmadan önce kümenin dışındaki bir tepkiyi reddeder. Eylem bir `ReactionPress` (hedef, tepki, basılı mı) alır, kaydeder ve yeni `ReactionState` değerini (sayılar ve kullanıcının kendi tepkileri) döndürür; sayılar ve `aria-pressed` durumları yerinde güncellenir. Her düğme, tepkinin adı ve sayısıyla adlandırılmış bir toggle button'dır; sembol ekran okuyuculardan gizlenir. `.state(...)` ilk çizim için sayıları verir, böylece betik olmadan da görünürler. Betik (`reactions.js`, 1,3 KB gzip) yalnızca tepkisi olan sayfalarda yüklenir.

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

Bir anın, sayfanın dilinde "3 dakika önce" ya da "2 gün içinde" gibi göreli zaman olarak gösterilmesi. Sunucu, kullanıcının saat diliminde tam tarihi ve saati `<time datetime="...">` içine yazar; böylece HTML dakikadan dakikaya aynı kalır ve önbelleğe alınmış sayfa yine doğrudur. Çalışma zamanı metni sayfa yüklendiğinde, `nav.js` bir ekranı değiştirdiğinde ve dakikada bir `Intl.RelativeTimeFormat` ile yeniden yazar; tam zaman ipucunda kalır. Betik olmadan metin tam zamandır.

```rust
RelativeTime::new(post.published)
```


## Share

Bir bağlantıyı paylaşan düğme: tarayıcıda Web Share API varsa (çoğu telefon) sistem paylaşım sayfası, yoksa bunu söyleyen bir toast ile panoya kopyalama. Gösterilen sayfayı ya da parametreleri derleyici tarafından denetlenen `Share::screen(screen, params)` ile bir ekranı paylaşır. `.title`, paylaşım sayfasının gösterdiği başlığı ayarlar; etiket, `.label` vermedikçe kullanıcının dilinde "Paylaş" olur. Sunucu düğmeyi gizli çizer ve betiği onu gösterir, böylece betiksiz bir sayfada hiçbir şey yapmayan bir düğme olmaz. Betik (`share.js`, 1 KB altında) yalnızca paylaşım düğmesi olan sayfalarda yüklenir.

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


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
