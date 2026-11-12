# Ağaç ve grafikler

## Ağaç

`Tree::of(action)` ihtiyaç duyuldukça yüklenen bir hiyerarşi gösterir. action bir `TreeQuery` alır ve `q.parent` öğesinin çocuklarını döndürür (en üst seviye için `None`). Bir dal çocuklarını ilk açıldığında yükler; bu yüzden büyük bir hiyerarşi üzerindeki ağaç, açılan her dal için tek bir küçük istek maliyeti taşır.

```rust
#[action(permission = "stock.product.read")]
pub async fn sample_tree(q: TreeQuery, store: SampleStore) -> Result<Vec<TreeItem>> {
    Ok(match q.parent.as_deref() {
        None => vec![TreeItem::branch("piece", "Piece"), TreeItem::branch("kg", "Kilogram")],
        Some(unit) => store.by_unit(unit)?.into_iter().map(|s| TreeItem::leaf(s.id.0.to_string(), s.name).hint(s.code)).collect(),
    })
}

Tree::of(sample_tree).label("Products by unit")
```

@live tree

Ağaç ARIA tree desenini izler: ok tuşları öğeler arasında gezinir, dalları açar veya kapatır. Sunucu, dal başına çocuk sayısını sınırlar (`MAX_TREE_CHILDREN`).

`.draggable()` yaprakların bir bırakma hedefine, örneğin bir island'a sürüklenmesini sağlar. Sürükleme `application/x-zeyn-tree` MIME türüyle `{ "id", "path", "label" }` JSON'unu (`path` en üst seviyeden yaprağa kadar id'leri tutar) ve etiketi `text/plain` olarak taşır; bırakma hedefi `dragover` işleyicisinde `dropEffect = "copy"` ayarlar. Sürüklemeden, odaktaki yapraktaki Ekle düğmesi (işaretçi için) ve Control+Enter, Mac'te Command+Enter (klavye için), aynı nesneyi `detail` olarak taşıyan iptal edilebilir `zeyn:tree-insert` DOM olayını gönderir. Kendi o anki konumuna ekleyen hedef `preventDefault()` çağırır; ağaç eklemeyi ekran okuyuculara ancak o zaman duyurur. Bir yaprak odaktayken Control+Enter ağaca aittir, bu yüzden aynı tuşlardaki bir uygulama kısayolu orada çalışmaz. Herhangi bir sayfa bu veriyi taklit edebilir; hedef onu güvenilmeyen girdi sayar ve erişimi sunucuda denetler. Sürükleme kodu (`treedrag.js`) yalnızca sürüklenebilir ağaç olan sayfalarda yüklenir.

```rust
Tree::of(report_fields).label("Fields").draggable()
```

## Grafikler

`Chart::bar`, `stacked_bar`, `line`, `area`, `combo`, `scatter`, `donut`, `gauge` ve `funnel` SVG'yi sunucuda çizer. Tarayıcıda grafik kütüphanesi yoktur; grafik, sayfa HTML'inin bir parçasıdır.

```rust
Row::new()
    .col(Chart::bar("Sales").labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]))
    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0]))

Chart::stacked_bar("Channels").labels(months).series("Shop", shop).series("Web", web)
Chart::combo("Sales and margin").labels(months).series("Sales", sales).line_series("Margin %", margin).right_axis()
Chart::area("Visits").labels(months).series("Visits", visits)
Chart::scatter("Price and units").points("Tea", [(1.5, 40.0), (2.5, 22.0)])
Chart::gauge("Capacity").value(72.0).range(0.0, 100.0).target(85.0)
Chart::funnel("Checkout").labels(["Visited", "Cart", "Paid"]).series("People", [1000.0, 420.0, 150.0])
```

@live chart

Her grafikte bir başlık, ekran okuyucular için metin özeti, iki ya da daha fazla serisi olduğunda bir gösterge (legend) ve sayıları *Show data* altında bir tablo olarak bulunur. Çubuk, çizgi, alan ve birleşik (combo) grafikler SVG'de yalnızca geometrilerini çizer; bu geometri kartla birlikte esner (`--z-chart-h`, 240 px, telefonlarda 200 px). Eksen etiketleri yanındaki HTML'dir, bu yüzden her genişlikte 12 px kalır, on binden itibaren kısaltılır (`600K`, `1.2M`) ve kategori etiketleri en fazla on ikisi görünecek şekilde seyreltilir (480 px'ten dar grafiklerde bunun yarısı). Çubuklar bantlarının %60'ını kaplar; çizgiler noktasız 2 px'tir, son değerde ve üzerine gelinen kategoride yuvarlak bir nokta taşır; alanlar giderek solar. Bir kategori üzerindeki işaretçi ya da klavye odağı bir artı imleci (crosshair) ve o kategorideki her seriyi gösteren bir tooltip çizer. Seri renkleri `--z-chart-1` ile `--z-chart-8` arasındaki token'lardır ve sıraları ilk beşini renk körlüğünde de birbirinden ayrı tutar; 7. ve 8. seriler çizgilerini ayrıca kesikli çizer. Başlangıç denetimi ve bir test (`chart_colors_reach_three_to_one_in_both_themes`) her seriyi `--z-surface` karşısında 3:1'e tutar (bkz. [Tema](theming.md)).

Her çubuk, nokta, dilim, aşama ve gauge değeri, etiketi ve değeri adı olan bir işarettir. SVG, başlık ve özet ile adlandırılmış bir gruptur ve grafik tek bir Tab durağıdır:

| Tuş | Ne yapar |
|---|---|
| Tab | Grafiğe girer, en son odaklanılan işarete gider (başlangıçta ilkine) |
| Sol, Sağ | Aynı serinin önceki veya sonraki işareti (`dir="rtl"` altında yer değiştirir) |
| Yukarı, Aşağı | Önceki veya sonraki serinin aynı noktası |
| Home, End | Serinin ilk veya son işareti |
| Escape | İpucu kutusunu (tooltip) gizler |

Üzerine gelme, odaklanma ve dokunma aynı metni bir tooltip içinde gösterir; tooltip ekran okuyuculardan gizlenir, böylece hiçbir şey iki kez okunmaz. Betik olmadan da işaretler adlandırılmış kalır ve tablo durur. Küçük işaretler (8 px'lik noktalar) 24 px hedef boyutunun altındadır; *Show data* altındaki tablo eşdeğer yoldur (WCAG 2.5.8).

## Gantt

Günlere yayılmış çubuklar olarak görevler; örneğin bir projenin planı. `Gantt::new(label, start, end)` aralığı belirler, en fazla iki yıl; `.task(title, from, to)` bir görev ekler, `.done(percent)` ve `.tone(...)` son eklenen göreve uygulanır, `.today(day)` ise o güne bir çizgi çizer. Her görevin ilk ve son gününü metin olarak veren bir tablodur, çubuklar yalnızca bunu yineler; çubuklar aralığın uçlarında kesilir.

```rust
Gantt::new(t!("project.plan"), project.starts, project.ends)
    .task(t!("project.design"), design.from, design.to)
    .done(design.percent)
    .today(ctx.today())
```

@live gantt

## OrgChart

Tepedeki tek bir kutudan başlayan, çizgilerle bağlı kutulardan bir hiyerarşi: bir şirketin çalışanları, bir ürünün parçaları. `OrgNode::new(name)` bir kutudur, `.role(text)` adın altında daha silik bir satır, `.child(node)` ise altına bir kutu ekler. İç içe listelerden oluşur, böylece ekran okuyucular düzeyleri alır; bir şema en fazla 8 düzeyde 200 kutu çizer ve geniş olduğunda kendi kutusunun içinde kayar.

```rust
OrgChart::new(
    t!("company.chart"),
    OrgNode::new(director.name.clone())
        .role(t!("company.director"))
        .child(OrgNode::new(lead.name.clone())),
)
```

## FlowDiagram

Bir sürecin adımları, oklarla bağlı kutular olarak, baş taraftan son tarafa doğru: sepetten teslimata bir sipariş, dallarıyla bir onay. `.node(id, title)` bir adım ekler, `.tone(...)` son eklenen adımı renklendirir, `.edge(from, to)` iki adım arasına kimlikleriyle bir ok çizer, `.edge_with(from, to, label)` ise üzerinde bir sözcük olan bir ok çizer. Sunucu adımları, ilk adımlara uzaklıklarına göre sütunlara yerleştirir; önceki bir adıma giden ok kutuların altından geçer. Adımlar sıralı bir listedir ve her biri, ekran okuyucular için gittiği adımları adlandırır; bir diyagram en fazla 40 adım ve 80 ok çizer, geniş olduğunda kendi kutusunun içinde kayar.

```rust
FlowDiagram::new(t!("order.process"))
    .node("cart", t!("order.cart"))
    .node("pay", t!("order.payment"))
    .node("ship", t!("order.shipping"))
    .edge("cart", "pay")
    .edge_with("pay", "ship", t!("order.paid"))
```

@live flow

## Funnel

Her adımda bir kısmını kaybeden bir sürecin aşamaları; örneğin ziyaretler, sepetler ve siparişler. `Funnel::new(label)` ile `.stage(title, value)`, her aşamayı ilk aşamaya oranı kadar geniş, ortalanmış bir çubuk olarak çizer; sayısı ve bu oran metin olarak yazılır. Bir huni en fazla 10 aşama tutar.

```rust
Funnel::new(t!("sales.funnel"))
    .stage(t!("sales.visits"), visits)
    .stage(t!("sales.orders"), orders)
```

## ActivityRings

Bir bütünün en fazla dört payı, iç içe halkalar olarak; örneğin bir günün hedefleri. `.ring(title, value, max, tone)` dıştan içe doğru bir halka ekler, `.size(px)` çapı belirler. Her ad ve yüzde halkaların yanına yazılır, bu yüzden çizim ekran okuyuculardan gizlidir.

```rust
ActivityRings::new(t!("day.goals"))
    .ring(t!("day.packed"), packed, 60, Tone::Brand)
    .ring(t!("day.calls"), calls, 20, Tone::Success)
```

@live visuals

## ComparisonTable

Planlar ya da ürünler yan yana: her biri bir sütun, her özellik bir satır. `.plan(name)` bir sütun ekler, `.featured()` son eklenen planı öne çıkarır; `.row(feature, cells)` her plan için bir hücre alır: `true` ve `false` bir onay ve bir çarpı olur, "Var" ve "Yok" diye okunur, metin ise olduğu gibi yazılır. En fazla 6 plan ve 100 satır tutar.

```rust
ComparisonTable::new(t!("pricing.plans"))
    .plan(t!("pricing.free"))
    .plan(t!("pricing.team"))
    .featured()
    .row(t!("pricing.audit_log"), [false, true])
```

## Table

Ekranın zaten elinde olan metinlerden küçük bir tablo: bir fiyat listesinin planları, bir mağazanın açılış saatleri, bir raporun toplamları. `Table::new(caption)` tabloyu adlandıran bir başlık alır, `.head([...])` sütun başlıklarını, `.row([...])` ise her satırı verir; `.row_headers()` her satırın ilk hücresini o satırın başlığı yapar, böylece ekran okuyucular her hücrenin sütununu ve satırını söyler; `.numeric(index)` bir sayı sütununu sona hizalar, `.striped()` ise her ikinci satırı gölgelendirir. Başlıktan kısa bir satır boş hücrelerle doldurulur, uzun olan kesilir; bir tablo en fazla 200 satır ve 12 sütun tutar. Sayfalama, sıralama ya da filtre içermez ve istek göndermez: bir action'dan gelen ya da kullanıcının üzerinde çalıştığı satırların yeri `DataGrid`'dir. Dar ekranlarda kendi kutusunun içinde yana doğru kayar; kutu odak alır, böylece ok tuşları onu kaydırır.

```rust
Table::new(t!("shop.opening_hours"))
    .head([t!("shop.day"), t!("shop.opens"), t!("shop.closes")])
    .row([t!("shop.weekdays"), "09:00".into(), "18:00".into()])
    .row([t!("shop.saturday"), "10:00".into(), "14:00".into()])
    .row_headers()
```

@live table

## Heatmap

Son haftaların her günü için bir sayı, örneğin alınan siparişler ya da kaydedilen saatler, karelerden oluşan bir takvim olarak: her sütun bir hafta, her satır bir hafta günü, sayı büyüdükçe daha koyu. `Heatmap::ending(label, end)`, genellikle bugün olan `end` gününün haftasıyla biter ve ondan sonraki günler boş kalır. `.weeks(n)` kaç hafta gösterileceğini belirler: verilmedikçe 17, en fazla 53; `.value(day, n)` ya da `.values(days)` sayıları, `.tone(...)` ise rengi verir. Gösterilen en büyük sayı dört tonun en koyusunu alır. Hafta, kullanıcının dilinde haftanın başladığı günle başlar.

```rust
Heatmap::ending(t!("sales.orders_a_day"), ctx.today())
    .weeks(12)
    .values(orders.iter().map(|o| (o.day, o.count)))
    .tone(Tone::Success)
```

@live heatmap

Sunucunun çizdiği, script gerektirmeyen bir tablodur. Ekran okuyucular bir satırın hafta gününü ve her gün için tarihini ve sayısını kullanıcının dilinde okur; böylece ton yalnızca metnin söylediğini yineler. Bir karenin üzerine gelindiğinde aynı metin görünür. Dar ekranlarda tablo kendi kutusunun içinde yana doğru kayar ve kutu odak alır, böylece ok tuşları onu kaydırır. Her gün iki öğedir: varsayılan 17 hafta, bir bileşen için 300 öğelik bütçenin içinde kalır; tam bir yıl ise kalmaz.

## Anahtar göstergeler, sparkline ve bullet

`Kpi` bir değeri kartın en büyük metni olarak gösterir (30 px, sabit genişlikli rakamlar, para birimi simgesi ya da birim daha küçük ve soluk). Önceki döneme göre değişimini `+48.8%` gibi küçük bir çip olarak, yanında "önceki döneme göre" yazısıyla gösterir (değişim iyiyse yeşil, değilse kırmızı, yüzde yarımın altındaysa gri; maliyetler ve bekleme süreleri için `.lower_is_better()`; ekran okuyucular cümlenin tamamını duyar). Bunlara ulaşılan payla birlikte bir hedef ve son değerlerin çipin renginde, solan bir alan üzerinde tam genişlikte bir eğilimi eklenir. Yedi ve daha fazla basamaklı değerler kısaltılır (İngilizcede `$1.23M`, Türkçede `1,23 Mn`); tam değer tooltip içinde ve ekran okuyucular için yer alır.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` bir tutarı kullanıcının yerel ayarında para birimiyle gösterir (İngilizcede `₺184,250.50`, Türkçede `184.250,50 ₺`, simge daha küçük yazılır); hedefi de aynı para birimindedir.

Bir `Row` ya da `AutoGrid` içindeki Kpi'lar, eğilimleri olsun ya da olmasın, en uzun olan kadar yüksektir ve sparkline her kartın altında durur. Kpi'lardan oluşan bir `Row` her birine genişliğin dörtte birini verir: masaüstünde dördü satırı doldurur, satır yaklaşık 830 px altında iki sütuna iner; 360 px altında kartlar eğilimlerini bırakır (satır üzerinde container query ile; bu yüzden dar bir sütundaki Kpi satırı daha erken kırılır). Her kart, eğilimi olsun ya da olmasın, en az 132 px yer tutar. Grafikler kapsayıcılarının genişliğini doldurur. Halka, gösterge, huni ve dağılım grafikleri oranlarını korur; halka ve gösterge grafikleri en fazla 18rem yüksekliğinde kalır ve ortada durur.

```rust
Row::new()
    .col(Kpi::money("Revenue", revenue).delta(12.4).trend(weekly))
    .col(Kpi::new("Orders", 1_284.0).delta(3.1))
    .col(Kpi::new("New customers", 312.0).delta(-2.0))
    .col(Kpi::money("Average order", average).delta(0.0))
```

Bir `DataGrid` içinde `Vec<f64>` sütunu sparkline olarak, sayı veya para sütunu ise bir hedefe karşı bullet olarak çizilir:

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

İkisi de hücre başına tek bir küçük SVG'dir ve "Trend of 12 values from 40 to 61, lowest 12, highest 99" gibi bir adı vardır. Bütçe: iki hücre için satır başına yaklaşık 0,65 KB HTML; her iki hücreli 10.000 satır debug derlemesinde yaklaşık 0,4 sn'de çizilir (`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`) ve gridler yalnızca görünen satırları çizer. `charts.js` ham 3,7 KB (gzip ile 1,4 KB), `charts.css` 4,1 KB (gzip ile 1,3 KB) ekler.
