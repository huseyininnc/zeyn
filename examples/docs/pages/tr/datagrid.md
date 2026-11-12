# DataGrid

`DataGrid`, `DataQuery` alan ve `Page<E>` döndüren bir action'dan satırları gösterir. Sayfalama, sıralama, filtreler ve toplamlar sunucuda çalışır; dolayısıyla milyon satırlık bir grid, tarayıcıya yüz satırlık bir grid ile aynı maliyeti yükler.

## Bir grid

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

Bir başlığa tıklayarak sıralayın, filtre kutusuna yazın, bir sütunu taşımak için başlığını, yeniden boyutlandırmak için kenarını sürükleyin. Toplamlar yalnızca sayfayı değil, filtrelenmiş her satırı kapsar.

## Action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply`, bellekteki bir `Vec` üzerinde filtreler, sıralar, toplar ve sayfalar; bu küçük listeler ve testler için uygundur. Veritabanı destekli bir store aynı `DataQuery` değerini (sayfa, sıralama, filtreler, toplamlar) okur ve SQL ile yanıt verir; bkz. [Veri ve veritabanları](database.md).

## Sayfalama

| Sayfalama | Kullanım |
|---|---|
| `Paging::pages(n)` | Numaralı sayfalar, ayarlanmazsa 50 satır |
| `Paging::infinite(n)` | Kaydırırken daha fazla satır yükler |
| `Paging::virtual_scroll()` | Çok büyük listeler; sayfada yalnızca görünen satırlar bulunur |
| `Paging::none()` | Tek sayfaya sığan kısa listeler |

İstemci ne isterse istesin, sunucu sayfa boyutunu sınırlar.

## Sütunlar

| Çağrı | Sonuç |
|---|---|
| `.width(px)`, `.flex(n)` | Sabit genişlik veya boş genişlikten bir pay |
| `.filter()` | Türe uyan bir filtre kutusu: metin, sayı aralığı, tarih aralığı, seçim listesi |
| `.sum()`, `.avg()`, `.min()`, `.max()` | Alt bilgide toplamlar. Metin sütununda `sum` derlenmez |
| `.badge(\|v\| tone)` | Değeri tonun renginde, tonun soluk zemini üzerinde bir etiket olarak gösterir |
| `.status(\|v\| tone)` | Değeri durum olarak gösterir: düz metnin önünde tonun renginde bir nokta; rozetten daha sakindir |
| `.progress(max)` | 0'dan `max`'a bir çubuk |
| `.check_icon()` | `true` için bir onay işareti |
| `.link(\|row\| effect)` | Hücreyi, bir effect çalıştıran bağlantı yapar |
| `.pin_start()`, `.hidden()` | Başa sabitlenir ya da kullanıcı gösterene kadar gizlenir |
| `.editable()` | Yerinde düzenlenebilir, `.edit_with(action)` ile kaydedilir |
| `.plain()` | Tam sayıları grup ayırıcısı olmadan gösterir; kodlar ve yıllar için |
| `.wrap()` | Uzun değerler birden çok satıra kaydırılır. Ayarlanmadıkça hücreler tek satırda kalır ve üç noktayla biter, böylece her satır aynı yükseklikte olur |

Satırlar 40 px'tir (kompakt yoğunlukta 32 px) ve yalnızca çizgilerle ayrılır; çok geniş grid'ler için `DataGrid::zebra()` satırları birer atlayarak gölgelendirir. Hücrelerdeki bağlantılar 500 ağırlığında metin gibi görünür ve üzerine gelindiğinde altı çizilir.

Sayılar ve para tutarları sağa hizalanır ve sabit genişlikli rakamlarla yazılır. Tam sayılar kullanıcının yerel ayarına göre gruplanır (İngilizcede `1,234,567`, Türkçede `1.234.567`); bu, grid hücrelerinde ve `DescriptionList` gibi görüntüleme değerlerinde geçerlidir. `#[key]` kimlikleri hiç gruplanmaz, `.plain()` ise tek bir sütunda gruplamayı kapatır.

## Satırlar

- `.on_open(|row| effect)`: bir satırda çift tıklama veya Enter.
- `.row_action(label, action, |row| input)`: her satırda bir düğme.
- `.bulk_action(label, action, M::id)`: seçili satırlarda çalışır.
- `.row_action` ya da `.bulk_action` hemen ardından `.confirm(message)` ve `.tone(Tone::Danger)`: o işlem çalışmadan önce `ActionButton::confirm` ile aynı pencerede sorar (odak önce İptal'dedir) ve düğmesini kırmızı çizer.
- `.row_menu(|row| ContextMenu)`: bir satırın sağ tık menüsü.
- `.row_style(|row| Option<RowStyle>)`: danger, warning veya success satırlar.
- `.exportable()`: filtrelenmiş satırların CSV dışa aktarımı, sunucu tarafından akıtılır.

## Çalışma anında belli olan sütunlar

`DataGrid::dynamic(action)`, sütunları satırlarla birlikte gelen sonuçlar içindir, örneğin bir SQL önizlemesi. Action, sütunları (ad, etiket ve `Integer`, `Decimal` ya da `Date` gibi bir `DynamicKind`) ve JSON olarak bir sayfa satırı içeren bir `DynamicPage` döndürür:

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

Sayfalama, sanal kaydırma, klavye kullanımı ve sayıların ve tarihlerin kullanıcının dilinde biçimlenmesi tipli bir grid'deki gibi çalışır. Adlar, etiketler ve hücreler kaçışlanır; sunucu sütun ve satır sayısını ve metin uzunluğunu sınırlar. Grid sıralamaz ve filtrelemez, satırları salt okunurdur. [DataGrid başvurusu](grid.md) ayrıntıları verir.

## Arama ve bantlar

`.search()` tablonun üstüne bir arama kutusu koyar. Eylem metni `DataQuery::q` olarak alır;
`DataQuery::apply` onu maskeli olanlar dışındaki metin alanlarında arar.
`.band(label, |b| b.column(..).column(..))` sütunları tek bir başlık hücresi altında toplar.
Ayrıntılar [DataGrid başvurusunda](grid.md).

## Yerinde yenileme

En son çağrılan `.listens(topic)`, topic adlandırıldığında grid'in yerinde yeniden yüklenmesini sağlar ([Topic'ler](effects.md#topics) bölümüne bakın): `ui.refresh(topic)` sonrasında geçerli sayfasını sıralaması ve filtreleriyle yeniden yükler, `.drives(topic)` diyen bir filtre kontrolü değiştikten sonra yeni screen params ile ilk sayfadan başlar. Grid her izin denetimiyle `POST /_zeyn/grid` üzerinden yeniden yüklenir ve filtrenin değiştirdiği bir screen param'ı grid'e screen fonksiyonu üzerinden ulaşır, örneğin bir scope olarak:

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

[DataGrid başvurusu](grid.md) kayıtlı görünümleri, master detail, gruplamayı, ağaç verisini, satır içi düzenlemeyi, hücre gezinmesini, kopyala yapıştırı, sütun sanallaştırmayı ve erişilebilirliği kapsar.
