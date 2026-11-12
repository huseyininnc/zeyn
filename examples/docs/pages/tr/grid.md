# Data Grid

Durum: `zeyn::DataGrid`, `zeyn::server` içindeki grid uç noktaları ve `assets/zeyn/*.js` (`/_zeyn/zeyn.js` olarak sunulur) ile uygulanmıştır.

Grid sunucuda render edilir. Satırları, bir `DataQuery` alıp `Page<E>` döndüren bir action'dan gelir.
Tarayıcı çalışma zamanı (runtime) sayfaları ister, kullanıcının yaptıklarını (sıralama, filtre,
yerleşim, düzenleme, action'lar) gönderir ve geri aldığı HTML'i yerine koyar. Her çağrı yetkileri
sunucuda denetler ve istemcinin gönderdiği hiçbir şeye güvenilmez.

```rust
DataGrid::of(search_products)
    .mode(DataMode::Server)
    .paging(Paging::pages(25))
    .column(Product::code).width(120).pin_start().filter()
        .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
    .column(Product::name).flex(1).filter().editable()
    .column(Product::unit).filter().editable().badge(|u| match u {
        Unit::Kg => Tone::Info,
        _ => Tone::Neutral,
    })
    .column(Product::price).filter().editable().avg().max()
    .column(Product::stock).filter().sum().progress(120)
    .column(Product::active).filter().check_icon().editable()
    .edit_with(save_product)
    .row_action(t!("stock.restock"), restock_product, |p| p.id)
    .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
    .row_menu(|p| ContextMenu::new()
        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
        .action(t!("stock.restock"), restock_product, p.id))
    .exportable()
```

## Çalışma anında belli olan sütunlar

`DataGrid::dynamic(action)`, sütunları ancak sorgu çalıştıktan sonra belli olan bir sonucu gösterir, örneğin bir SQL önizlemesi ya da bir veri kaynağı tarayıcısı. Action bir `DataQuery` alır ve bir `DynamicPage` döndürür: sütunlar (`DynamicColumn { name, label, kind }`) ve her biri sütun sırasında bir JSON dizisi ya da sütun adına göre bir JSON nesnesi olan bir sayfa satır.

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

Grid sütunlarını her sayfadan kurar ve satırları tipli grid'in kendi koduyla çizer; sayfalama, sanal kaydırma, sütun sanallaştırma, klavye, kopyalama ve sütunları boyutlandırma, taşıma ve gizleme aynı çalışır. `DynamicKind` bir sütunun değerlerini nasıl okuyup gösterdiğini seçer: `Text`, `Integer` (kullanıcının yerel ayarında gruplanır), `Decimal`, `Bool` (kullanıcının dilinde evet ve hayır), `Date`, `Time`, `DateTime` (kullanıcının saat diliminde) ve `LocalDateTime`. Türüne göre okunamayan bir değer metin olarak gösterilir. Her sütun türüne uyan bir genişlikle başlar (metin için 180 px, sayılar için 130, tarihler için 120), böylece geniş bir sonuç yana kayar.

Action'ın döndürdüğü her şey kullanıcı verisi sayılır. Sütun adları, etiketler ve hücreler diğer değerler gibi kaçışlanır. Sunucu en fazla `MAX_DYNAMIC_COLUMNS` (200) sütun ve sorgunun sayfa boyutu kadar satır tutar (`DataQuery::limit`, en fazla 500), adları ve etiketleri `MAX_COLUMN_NAME` (200), metin hücrelerini `MAX_CELL_TEXT` (1.000) karakterde keser. Sıralama anahtarları, filtreler, toplamlar ve gruplar action çalışmadan önce sorgudan çıkarılır ve başlıklarda sıralama düğmesi yoktur; action istemciden hiçbir sütun adı almaz. Erişim, her grid'de olduğu gibi action'ınkidir.

Boş adlar `col_<sıra>` olur, tekrar eden adlar `_2`, `_3` alır; nesne satırları yine action'ın kendi adlarıyla okunur. Bir sayfa en fazla `MAX_PAGE_CELLS` (50.000) hücre ve `MAX_PAGE_TEXT` (8 MB) hücre metni tutar. Her metin hücresi metin bütçesinden eşit pay alır, bu yüzden geniş bir sayfa daha kısa metin gösterir; hücre bütçesini aşan satırlar sayfanın `next` değerinin gösterdiği bir sonraki sayfayla gelir. İç içe JSON değerleri yalnızca kendi paylarına kadar yazılır. Action'da 403 MB tutan 500 satır, 200 sütun ve 1.000 emojilik bir cevap, action'ın kendi cevabının üstüne bellek harcamadı ve 4,6 MB gönderdi.

`DynamicColumn::masked(MaskStyle::LastFour)` bir sütunu sayfa okunur okunmaz maskeler; ham değerleri HTML'e, loglara ya da yapay zekâ ajanlarına hiç ulaşmaz ve kimse açamaz. `DynamicGrid::sensitive(model, key)` gösterilen her satırı, `key` sütunuyla anahtarlanmış bir `model` okuması olarak denetim kaydına yazar, `sensitive` bir entity'nin satırları gibi. Bunlar olmadan sonucun ne gösterebileceğine action karar verir.

Sütunlar değiştiğinde tarayıcı eski sütunlardan sakladığı satırları bırakır. Kayıtlı bir görünüm yalnızca yapıldığı sütunlar için kullanılır.

Başlık sütunların bir parmak izini taşır (`data-zeyn-shape`). Tarayıcı bunu her sayfa isteğiyle gönderir ve sunucu yeni bir başlığı yalnızca sütunlar değiştiğinde gönderir, örneğin `.listens(topic)` grid'i yeni bir sorguyla yeniden yüklediğinde. Satırlar salt okunurdur: dinamik bir grid'de düzenleme, satır eylemleri, seçim, ayrıntılar, gruplama ve dışa aktarma yoktur.

## Sütun toplamları

`sum()` ve `avg()` tam sayılarda ve parada, `min()` ve `max()` ayrıca tarih, saat ve an (moment)
türlerinde, `count()` ise her sütunda çalışır. Bunları başka bir türde kullanmak derlenmez.
Toplamlar yalnızca sayfayı değil, filtrelerle eşleşen her satırı kapsar.

`DataQuery::apply` bunları bellekteki veri için hesaplar. Sayfalamayı SQL'de yapan bir veri katmanı
`query.aggregates` değerini okur ve tek bir toplama sorgusundan `Page::with_total(field, op, value)`
ekler. Grid yalnızca sütunlarının bildirdiği toplamları tutar, bu yüzden bir istemci başkalarını
isteyemez. Toplamlar sunucuda kullanıcının dilinde biçimlendirilir. Tam sayıların ortalamasında iki
ondalık basamak bulunur (`Value::Decimal`), para ortalamaları ise en küçük birime, yarısı sıfırdan
uzağa doğru yuvarlanır.

## Filtreler ve sıralama

`filter()` filtre satırına bir filtre ekler. Giriş alanı sütun türüne göre seçilir:

| Sütun | Filtre | Sorgu |
|---|---|---|
| Metin | içerir, ile başlar, eşittir | `contains`, `starts_with`, `equals` |
| Tam sayı, para | başlangıç, bitiş | `from`, `to` değerleri `1234.56` biçiminde |
| Tarih | başlangıç, bitiş | `from`, `to` değerleri `2026-03-31` biçiminde |
| An (`DateTime`) | başlangıç, bitiş (tarihler) | kullanıcının saat diliminde tam bir gün |
| Saat | başlangıç, bitiş | `14:30` |
| Seçim (choice) | seçeneklerden herhangi biri | `any_of` |
| Evet/hayır | tümü, evet, hayır | `is` |

Değerler, kullanıcının dili ne olursa olsun sabit bir makine biçiminde taşınır. Eski
`{ "field", "contains" }` biçimi hâlâ çalışır. Sunucu, `filter()` olmayan sütunlardaki filtreleri
ve gridde bulunmayan sütunlardaki sıralamaları düşürür; ayrıca filtreleri (32), filtre metnini
(200 karakter), seçenekleri (100) ve sıralama anahtarlarını (4) sınırlar. Okunamayan bir değer
yok sayılır. Tarih filtrelerinin saat dilimi sunucudaki kullanıcı ayarlarından gelir.

Bir başlığa Shift+tıklamak, o sütunu sıralama anahtarlarına ekler. Başlıktaki bir sayı sırayı gösterir.

## Sütunlar: sabitleme, yeniden boyutlandırma, gizleme, taşıma

`pin_start()` ve `pin_end()`, grid yana kayarken bir sütunu bir kenarda tutar (her tarafta en fazla
üç). `hidden()` bir sütunu kullanıcı gösterene kadar dışarıda bırakır. Her başlığın bir sütun menüsü
vardır (düğme, sağ tıklama, Shift+F10 veya bağlam menüsü tuşu); menüde sıralama, sabitleme, sola ve
sağa taşıma, gizleme, filtre, sütun seçici ve sıfırlama bulunur. Başlığın kenarı sütunu yeniden
boyutlandırır, sürükleyerek veya ok tuşlarıyla (daha büyük adımlar için Shift). Bir başlığı aynı
sabitlenmiş gruptaki başka bir başlığın üzerine sürüklemek sütunu taşır; kısa bir basış ise yine
sıralar. Hepsi sağdan sola yönde de çalışır.

## Arama

`search()` tablonun üstüne bir arama kutusu koyar. Kullanıcının yazdığı metin kısa bir aradan
sonra tablonun eylemine `DataQuery::q` olarak gider; her sayfa, blok, dal, eşleşen tüm satırlar
üzerindeki toplu işlem ve dışa aktarma ile birlikte. Yeni bir arama öncekinin isteğini iptal eder.
`DataQuery::apply` ve SQL veri katmanları metni büyük küçük harf ve aksan farkı gözetmeden metin
alanlarında arar; maskeli alanlarda asla aramaz. `search()` olmayan tablo, istemcinin gönderdiği
`q` değerini atar. Sonuç tablonun canlı bölgesinde duyurulur ve arama bir şey bulamazsa
`empty_filtered` gösterilir. Arama metni kayıtlı görünüme girmez. Yapay zekâ ajanları `q`
alanını tablonun araç şemasında bulur.

## Bant başlıkları

`band(label, |b| ...)` kapanışın eklediği sütunları, sütun başlıklarının üstündeki bir satırda tek
bir başlık hücresi altında toplar:

```rust
DataGrid::of(search_products)
    .search()
    .column(Product::code).pin_start()
    .band(t!("stock.price_and_stock"), |b| b
        .column(Product::price).avg()
        .column(Product::stock).sum())
    .column(Product::active)
```

Bant, gösterilen sütunları üzerinde `colspan` taşıyan, `scope="colgroup"` bir `th` hücresidir;
bant dışındaki sütunların üstündeki hücreler boş kalır. Bant satırı kullanıcının düzenine uyar:
gizlenen sütun bandından çıkar, bant yan yana ve aynı sabitleme tarafında kalan her sütun dizisi
için bir hücre gösterir (bir sütunu uzağa taşımak bandı böler), sabitlenmiş diziler sütunlarıyla
birlikte yerinde kalır ve sütun sanallaştırması bant satırını bütün tutar. Dışa aktarma ve
kopyalama bantları içermez.

## Kayıtlı görünümler

Kullanıcının yerleşimi (gösterilen sütunlar sırasıyla, genişlikler, sabitlemeler), sıralaması ve
filtreleri bir `DataGridView` oluşturur. `App::datagrid_views(store)` ile bu görünüm sunucuda her
kullanıcı için, her tenant, screen ve grid başına kaydedilir ve sayfa, ilk sayfası zaten sıralanmış
ve filtrelenmiş olarak o görünümle gelir. Bir veritabanı için `DataGridViewStore` uygulayın;
`MemoryViewStore` geliştirme ve testler içindir. Store yoksa tarayıcı görünümü `localStorage` içinde
tutar. Sunucu aldığı her görünümü gridin kendi sütunları ve sınırlarıyla temizler.

## Hücreler

| Yöntem | Sütunlar | Gösterir |
|---|---|---|
| `badge(\|v\| Tone)` | `#[choice]` enum'ları | her seçenek için renkli bir rozet |
| `progress(max)` | sayılar, para | bir çubuk ve sayı |
| `link(\|row\| Effect)` | herhangi biri | bir effect çalıştıran bağlantı olarak değer |
| `check_icon()` | `bool` | ekran okuyucular için sözcüklerle birlikte bir onay işareti veya tire |
| `relative_time()` | `DateTime` | "3 dakika önce", tam zaman ipucu (tooltip) olarak |
| `cell(renderer)` | herhangi biri | kendi HTML'iniz (`CellRenderer<T>`) |

Bir `CellRenderer` kullanıcı verisini escape etmelidir. Dışa aktarmalar ve kopyalamalar düz değeri kullanır.

## Satır action'ları, menüler ve toplu action'lar

`row_action(label, action, |row| input)` her satıra bir düğme ekler. `row_menu(|row| ContextMenu)`
bir bağlam menüsü ekler. `Screen::context_menu(...)` bir screen'in boş alanına bir menü ekler. Bir
action çalıştıran öğeler yalnızca o action'ın iznine sahip kullanıcılara görünür.

Bir satır öğesi çalıştığında istemci yalnızca satır anahtarını ve öğeyi gönderir. Sunucu satırı
gridin action'ı üzerinden yeniden yükler, öğeyi o satırdan oluşturur, action'ın iznini denetler,
interceptor'ları ve ardından action'ı çalıştırır. Satır için artık var olmayan bir öğe (örneğin
devre dışı kalmış biri) reddedilir. Girdi hiçbir zaman istemciden gelmez.

`bulk_action(label, action, Model::key)` seçim çubuğuna bir düğme ekler. Action, anahtar alanının
türü olan `Vec<K>` alır; başka bir şey derlenmez. "Eşleşen tüm satırlar" seçildiğinde sunucu
anahtarları kullanıcının filtreleriyle kendisi toplar, en fazla 10.000. Tek tek seçilen anahtarlar
istemciden gelir, bu nedenle action her kaydı bir API action'ının yapacağı gibi denetlemelidir.

Tarayıcı, başka her yerde kendi bağlam menüsünü korur. `App::browser_context_menu(false)` bunu metin
girişleri ve düzenlenebilir metin dışında kapatır. Bu hiçbir şeyi korumaz: kullanıcılar yine sayfa
kaynağını okuyabilir, geliştirici araçlarını açabilir veya betikleri kapatabilir. Ayrıca yazım
denetimini, bağlantı kopyalamayı, çeviriyi ve yardımcı teknolojileri de ortadan kaldırır. Varsayılanı
koruyun.

## Satır içi düzenleme

`edit_with(save)`, `Valid<E>` alan bir action ile (formlarla aynı kural) ve sütunlarda `editable()`
ile kullanılır. Anahtarlar, salt okunur alanlar ve satır sürümleri hiçbir zaman düzenlenebilir
olmaz. Düzenlenebilir bir hücrede F2 veya çift tıklama satırı düzenlemeye başlatır; Enter kaydeder,
Escape iptal eder, Ctrl+Z veya toast düğmesi son kaydı geri alır.

Sunucu satırı anahtarla yeniden yükler ve yalnızca düzenlenebilir sütunları ve satır sürümünü
`Form::read` üzerinden uygular; böylece değerler bir formun yapacağı gibi okunur ve doğrulanır. Başka
herhangi bir sütun için gelen değer `403` ile reddedilir. Eski bir sürüm `409` olarak döner,
doğrulama hataları `422` olarak döner ve kendi hücrelerinde gösterilir.

## Satır ekleme, silme ve toplu düzenleme

```rust
DataGrid::of(search_products)
    .column(Product::name).editable()
    .column(Product::price).editable()
    .edit_with(save_product)
    .add_with(create_product)
    .delete_with(delete_products, Product::id)
    .batch_edit()
```

`add_with(create)` tablonun üstüne "Satır ekle" düğmesi koyar. Düğme en üstte, her `editable()`
sütun için bir giriş kutusu olan boş bir satır açar; Enter kaydeder, Escape satırı atar. Sunucu
modelin varsayılanından başlar, yalnızca düzenlenebilir sütunları bir formun yaptığı gibi okur
(başka bir alan `403` ile reddedilir, geçersiz değerler `422` ile döner ve hücrelerinde görünür),
`create` eyleminin iznini denetler ve eylemi çalıştırır; ardından tablo yeniden yüklenir. Sanal
tablolar satır eklemez.
Modelin varsayılan değerinde bırakamayacağı alanlar, örneğin isteğe bağlı olmayan bir tarih,
düzenlenebilir bir sütun ister.

`delete_with(action, Model::key)` seçim çubuğuna çalışmadan önce soran kırmızı bir "Sil" düğmesi
ekler; bir satırda Delete tuşu seçili satırları, seçim yoksa o satırı siler. Eylem anahtarları
toplu işlemdeki gibi alır, bu yüzden her kaydı kendisi denetlemelidir.

`batch_edit()` satır içi düzenlemeleri kullanıcı hepsini kaydedene kadar tutar. Enter değişen
satırı başında bir işaretle düzenleme durumunda bırakır ve tablonun üstündeki çubuk değişen
satırları sayar, "Değişiklikleri kaydet" ve "Değişiklikleri at" düğmelerini gösterir. Kaydetme
satırları satır içi düzenleme yolundan tek tek gönderir; böylece her satır ayrı ayrı okunur,
doğrulanır, sürümü denetlenir. Kaydedilemeyen satırlar değişikliklerini korur ve nedenini
gösterir. Başka satırlar yüklemek (sayfa, sıralama, süzgeç, arama) kaydedilmemiş değişiklikleri
atmadan önce sorar.

## Master detail

`detail(|row| (screen, params))`, her satırın o satırdan oluşturulan bir screen'e genişlemesini
sağlar; screen yalnızca kullanıcı açtığında yüklenir:

```rust
DataGrid::of(search_today)
    .column(Order::id)
    .detail(|o| (order_lines, OrderParams { id: o.id }))

#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        DataGrid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .column(OrderLine::product)
            .column(OrderLine::amount).sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}
```

Parametreler screen tarafından türlendirilir (`ScreenDef::Params`), bu yüzden yanlış parametreler
derlenmez. Bir detail tam bir screen'dir: bir form, iç içe bir grid veya kendi detail'leri olan bir
grid, istenen derinliğe kadar. Genişletici sütun yalnızca screen'i açabilecek kullanıcılara
görünür ve sunucu detail yüklenirken screen'in iznini yeniden denetler. Parametreler tarayıcı
üzerinden bir pencerenin (window) parametreleri gibi taşınır, bu yüzden detail screen'i,
kullanıcının adı geçen kaydı görmeye yetkili olup olmadığını denetlemelidir.

`scope(field, value)`, gridi her istekte `field` alanı `value`'ya eşit olan satırlarla sınırlar.
Filtre `DataQuery::filters` içinde ilk sırada yer alır ve istemci onu kaldıramaz; böylece iç içe bir
grid, tarayıcı ne gönderirse göndersin üst satırında kalır. Filtrelerden SQL üreten bir veri katmanı
bunu diğer filtreler gibi okur (sayılar ve tarihler için eşit sınırlı `Filter::range`, metin için
`equals`, seçimler için `any_of`, evet/hayır için `is`, boş değerler için `blank`).

Genişletici düğme, üzerindeyken Enter veya satırda Sağ ve Sol tuşları detail'i açar ve kapatır.
Kapatmak onu kaldırır, böylece bir sonraki sefer yeniden yüklenir.

Sanal (virtual) gridler her satırı sabit bir yükseklikte tutar, böylece her konuma anında erişilebilir;
genişleticileri göstermezler. Detail'li gridler için sayfalar veya sonsuz sayfalama kullanın.

## Gruplama

`group_by(field)` satırları sunucuda gruplar; her grubun içinde gruplamak için yeniden çağırın:

```rust
DataGrid::of(search_today)
    .paging(Paging::pages(20))
    .group_by(Order::city)
    .group_by(Order::status)
    .column(Order::amount).sum().avg()
```

Grid her grup için sütun etiketini, değeri, satır sayısını ve o gruptaki her sütun toplamını kendi
sütununun altında gösteren bir başlık satırı gösterir. Bir grubu açmak (düğmesi veya Sağ ve Sol
tuşları) alt gruplarını veya satırlarını yükler; daha fazlası kaldıysa her seferinde bir sayfa ve bir
"Daha fazla yükle" satırı ile. Kapatmak onları kaldırır. En üst düzey grupların üzerinde sayfalanır,
yani sayfalayıcı grupları sayar. Alt bilgi, eşleşen her satırın toplamlarını korur. Grupların içindeki
satırlar diğer her şeyi korur: düzenleme, satır action'ları, menüler, seçim ve detail'ler.

Veri katmanı `DataQuery::group` isteğini yanıtlar: ayarlandığında, o alandaki sıralama anahtarının
yönünde değere göre sıralanmış, satırlar gibi sayfalanmış ve `total` grupları sayan `Page::groups`
döndürür (grup başına değer, sayı ve sorgunun istediği toplamlar). `DataQuery::apply` bunu bellekteki
veri için yapar; SQL `GROUP BY` kullanır. `group` gridin kendisi tarafından ayarlanır ve istemciden
asla okunmaz. İstemci yalnızca açtığı grup değerlerinin yolunu gönderir; sunucu uzunluğunu denetler
ve her değeri o grubun sütunu üzerinde bir filtreye, kullanıcının filtrelerinin önüne koyarak
dönüştürür (boş grup için `blank`).

## Ağaç verisi

`tree(Model::parent)` satırları isteğe bağlı bir üst anahtara göre ağaç olarak gösterir: üstü olmayan
satırlar en üstte, bir satırın çocukları ise açıldığında, her seferinde bir sayfa olarak, en fazla 32
seviyeye kadar yüklenir.

```rust
DataGrid::of(search_employees)
    .tree(Employee::manager_id)
    .column(Employee::name)
    .column(Employee::salary).sum()
```

İlk sütun girintiyi ve düğmeyi taşır. Hangi satırların çocuğu olduğunu bilmek için grid, sayfa başına
action'ını bir kez daha çağırır: üst alanı sayfanın anahtarlarından biri olan satırlar, üst alana göre
gruplanmış olarak (anahtarlarda `Filter::any_of` ve `DataQuery::group`). Filtreler ve sıralama her
seviyede uygulanır; alt bilgi toplamları eşleşen her satırı kapsar. Ağaç gridleri gruplama yapmaz ve
satır detail'i göstermez. Satır içi düzenlemeden sonra satır ağaçtaki yerini korur.

Sanal gridler ne gruplar ne de ağaç gösterir, çünkü oradaki her satırın sabit bir yüksekliği vardır.

## Hücreler: gezinme, kopyalama ve yapıştırma

Bir satırda Sağ tuşu (veya bir hücreye tıklama) bir hücre imleci koyar, tıpkı bir elektronik tabloda
olduğu gibi. Ok tuşları onu taşır, bir ok ile Shift (veya Shift+tıklama) bir aralığı genişletir, Home
ve End satırın uçlarına, Ctrl+Home ve Ctrl+End ise gridin ilk ve son hücresine gider. Tab ve
Shift+Tab satır boyunca ve sonraki satıra ilerler, uçlarda gridden çıkar. İlk hücrede Sol tuşu
satıra geri döner. Enter veya F2 düzenlenebilir bir hücreyi düzenler, Enter bir hücrenin tek düğmesine
veya bağlantısına basar, Escape aralığı bırakır. Aralıktaki hücreler `aria-selected` taşır.

Bir hücrede Ctrl+C, aralığı sekmeyle ayrılmış metin olarak kopyalar; değerler kullanıcının gördüğü
gibidir. Bir satırda Ctrl+C hâlâ seçili satırları kopyalar.

Bir hücrede Ctrl+V, bir elektronik tablodan sekmeyle ayrılmış metni imleçten itibaren yapıştırır; tek
bir değer tüm aralığı doldurur. Yalnızca düzenlenebilir hücreler değişir (salt okunur hücrelere
yapıştırma bunu söyler), seçimler etiketle yapıştırılabilir, evet ve hayır kullanıcının sözcükleriyle.
Değişen her satır satır içi düzenleme yolundan (`/_zeyn/grid/edit` ve satır sürümü) birbiri ardına
kaydedilir; böylece her satır, satır içi düzenlemedeki gibi sunucuda okunur, doğrulanır ve denetlenir.
Başarısız olan bir satır hatalarını kendi hücrelerinde gösterir ve diğerleri kaydedilir. 10'dan fazla
satır önce sorar, 500'den fazlası reddedilir ve Ctrl+Z yapıştırmanın tamamını geri alır. Grid
düzenlemeleri, satır action'ları, toplu action'lar ve menü action'ları `App::write_limit` kapsamına
sayılır; bu yüzden büyük bir yapıştırma uygulamanın yazma sınırına takılabilir.

## Sütun sanallaştırma

40 veya daha fazla sütunlu bir grid, sayfada yalnızca sabitlenmiş sütunları ve görünümdeki sütunları
(her yanda altıya kadar ek ile birlikte) tutar. Her sütuna sabit bir genişlik verilir ve dışarıda
bırakılan sütunlar için her yanda tek bir boşluk hücresi durur; böylece genişlikler, sabitlemeler ve
kaydırma eskisi gibi kalır. Satırlar tam hücre listesini korur, bu yüzden kopyalama, yapıştırma ve
düzenleme her sütunu görür ve hücreler `aria-colindex` taşır. Pencere yalnızca dışındaki bir sütun
görünüme girdiğinde hareket eder. Sunucu yine de bir satırın her hücresini gönderir. Bu kadar geniş bir sanal grid (`VIRTUAL_WIDE_COLUMNS`; seçim, detay ve satır action sütunları dahil sayılır) ilk bloğunun tamamını 64 canlı satır yerine `<template data-zeyn-rows-later>` içinde gönderir; böylece tarayıcı hiçbir zaman her sütunu içeren bir satırı yerleşime sokmaz. Galerinin geniş gridinde en uzun başlangıç görevi 259 ms'den 112 ms'ye, DOMContentLoaded 402 ms'den 228 ms'ye düştü (8 ardışık yüklemenin medyanı, release sunucu, yoğun makine).

Galerinin geniş gridinde (62 sütun, 100.000 satır, sanal kaydırma, debug sunucu, Chromium, 1400'e 900
pencere, 144 Hz ekran) sütun sanallaştırmasız ve sanallaştırmalı olarak ölçülmüştür:

| | Sayfadaki hücreler | Yana, kare başına 60 px (p50 / p95) | Aşağı, kare başına 60 px (p50 / p95) | Aşağı, kare başına 2.000 px (p50 / p95) |
|---|---|---|---|---|
| Olmadan | 2.976 | 6,9 / 7,5 ms | 42,7 / 64,8 ms | 78,5 / 141 ms, 178 uzun görev |
| İle | 1.056 | 7,0 / 28,8 ms | 24,6 / 41,7 ms | 28,9 / 48,9 ms, 7 uzun görev |

Aşağı kaydırma yaklaşık iki kat hızlıdır ve sayfa hücrelerin üçte birini tutar. Yana kaydırma zaten
akıcıydı; pencerenin hareket ettiği karelerin maliyeti 29 ms'ye kadar çıkar. Maliyetin geri kalanı,
sütun sanallaştırmasının değiştirmediği, her sütunu içeren 200 satırlık blokların ayrıştırılmasıdır.

## Dışa aktarma

`exportable()` (ve isteğe bağlı olarak `export_permission(...)`, `export_limit(n)`) araç çubuğunun
`Export::Csv` düğmesinin eşleşen her satırı indirmesini sağlar. Sunucu gridin action'ı üzerinden
sayfaları gezer ve her sayfayı hemen, kullanıcının yerleşiminde ve dilinde akıtır. Formüller etkisiz
hale getirilir. Varsayılan olarak 100.000 satıra kadar.

`xlsx` Cargo özelliği (feature) ile araç çubuğunun `Export::Xlsx` düğmesi aynı satırları bir Excel
çalışma kitabı olarak indirir (`GET /_zeyn/grid/export?format=xlsx`). Hücreler türlerini korur:
sayılar, para birimi biçimiyle bir sayı olarak para, kullanıcının saat diliminde gerçek Excel tarihi
olarak tarihler ve tarih saatler ile mantıksal değerler. Seçimler etiketleriyle, maskelenmiş alanlar
maskelenmiş metinleriyle yazılır ve metin her zaman metindir, bu yüzden hiçbir şey formüle dönüşmez.
Başlık satırı kullanıcının dilindeki etiketleri içerir, kalındır ve dondurulmuştur; sütunlar
kullanıcının sırasını ve genişliklerini korur.

Çalışma kitabı `rust_xlsxwriter` ile sabit bellek kipinde yazılır: her sayfa geldikçe geçici bir
dosyaya gider (bellekte en fazla iki sayfa bekler) ve sıkıştırılmış çalışma kitabı sonunda yanıta
akıtılır. Altı sütunlu 100.000 satırı dışa aktarmak, satırlar yazılırken heap'i yaklaşık 134 KiB'de
tuttu (10.000 satırdan sonra ve 100.000 satırdan sonra aynı) ve sıkıştırma sırasında 460 KiB; dosya
3,4 MB idi.

`pdf` Cargo özelliği ile `Export::Pdf` aynı satırları bir liste raporu olarak yazdırır (en fazla 2.000
satır); bkz. [PDF belgeleri](operations.md#pdf-documents).

## Seçim

`selectable()` (`bulk_action` tarafından ima edilir) bir onay kutusu sütunu ekler. Shift bir aralık
seçer, Boşluk odaktaki satırı seçer ve başlıktaki kutu sayfayı seçer. Seçim çubuğu eşleşen her satırı
önerir.

## Erişilebilirlik

Tabloda `role="grid"`, `aria-rowcount` (sanal gridler), `aria-colcount`, `aria-colindex` ve
`aria-sort` bulunur. Satırlar gezinen bir tab durağı (roving tab stop) kullanır. Etkin satırdaki
düğmelere Tab ile ulaşılır. Gruplanmış ve ağaç gridler `role="treegrid"` olup satırlarda `aria-level`
ve `aria-expanded` taşır. Menüler `menuitem`, `menuitemcheckbox` ve `menuitemradio` ile
`role="menu"` kullanır ve odak önceki yerine döner. Boş ve hata durumları duyurulur (`role="status"`).

## Performans

Performance API ile, showcase arşivinde (1.000.000 satır, sanal kaydırma, debug sunucu, Chromium,
144 Hz ekran) ölçülmüştür:

| Kaydırma | Kare p50 | Kare p95 | En yüksek | Uzun görevler |
|---|---|---|---|---|
| Tekerlek hızı (kare başına 60 px) | 13,9 ms | 20,9 ms | 34,6 ms | 0 |
| Hızlı (kare başına 2.000 px) | 14,1 ms | 27,8 ms | 54,9 ms | 0 |
| Tüm liste boyunca taramalar | 13,9 ms | 27,8 ms | 48,6 ms | 0 |

DOM 62 satır (yaklaşık 660 öğe) tutar ve JS heap'i 4 MB civarında kalır. Aynı arşivden 100.000 satırı
dışa aktarmak 3,4 saniyede 7 MB akıttı.

### Ekran oluşturucuyu ucuz tutun

İstekler durumsuzdur, bu yüzden her grid isteği gridi tutan ekranı yeniden kurar: sanal bir gridin getirdiği her blok (her 200 satır kaydırmada), her sayfa, sıralama, filtre ve dışa aktarma, her satır içi düzenleme ve her satır action'ı, gridin action'ını çalıştırmadan önce `#[screen]` fonksiyonunu yeniden çalıştırır. Sunucu, gridi düğüm yoluyla bulmak ve erişimi denetlemek için ekrana ihtiyaç duyar; kullanıcı başına durum tutmamanın bedeli budur.

Yalnızca bileşenleri dizen bir oluşturucu mikrosaniyelere mal olur. Veritabanını sorgulayan bir oluşturucu ise bu sorgunun bedelini o isteklerin her birinde öder. Bu yüzden:

- Satırları gridin action'ında yükleyin (`DataGrid::of(search)`). Zaten istek başına bir kez çalışır ve sayfayı, sıralamayı ve filtreleri içeren `DataQuery`'yi alır.
- Seçenekler ve aramalar alan tipine (`#[choice]`), bir form aramasına (`Form::lookup`, kullanıcı yazdığında action'ını çağırır) ya da sınırlı bir önbellekte tutan bir servise aittir; oluşturucunun çalıştırdığı bir sorguya değil.
- Bir ekran kayıt yüklemek zorundaysa (`Form::of(record)`), büyük veya sanal gridleri ondan uzak tutun: kendi ekranlarına, bir detay ekranına (`DataGrid::detail`) ya da bir pencereye koyun; böylece blok istekleri hiçbir şey yüklemeyen bir ekranı yeniden kurar.
- Ekran parametreleri oluşturucunun ihtiyaç duyduğu girdidir; onları anahtarlarla sınırlı tutun ve gridin action'ının bunlara göre filtrelemesine izin verin (`DataGrid::scope`).
