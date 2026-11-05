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

Her grafikte bir başlık, ekran okuyucular için metin özeti, bir gösterge (legend) ve sayıları *Show data* altında bir tablo olarak bulunur. Seri renkleri `--z-chart-1` ile `--z-chart-6` arasındaki token'lardır; bir test (`chart_colors_reach_three_to_one_in_both_themes`) açık temada ve iki koyu temada `--z-surface` ve `--z-surface-2` karşısında 3:1 kontrastı denetler (bkz. [Tema](theming.md)).

Her çubuk, nokta, dilim, aşama ve gauge değeri, etiketi ve değeri adı olan bir işarettir. SVG, başlık ve özet ile adlandırılmış bir gruptur ve grafik tek bir Tab durağıdır:

| Tuş | Ne yapar |
|---|---|
| Tab | Grafiğe girer, en son odaklanılan işarete gider (başlangıçta ilkine) |
| Sol, Sağ | Aynı serinin önceki veya sonraki işareti (`dir="rtl"` altında yer değiştirir) |
| Yukarı, Aşağı | Önceki veya sonraki serinin aynı noktası |
| Home, End | Serinin ilk veya son işareti |
| Escape | İpucu kutusunu (tooltip) gizler |

Üzerine gelme, odaklanma ve dokunma aynı metni bir tooltip içinde gösterir; tooltip ekran okuyuculardan gizlenir, böylece hiçbir şey iki kez okunmaz. Betik olmadan da işaretler adlandırılmış kalır ve tablo durur. Küçük işaretler (8 px'lik noktalar) 24 px hedef boyutunun altındadır; *Show data* altındaki tablo eşdeğer yoldur (WCAG 2.5.8).

## Anahtar göstergeler, sparkline ve bullet

`Kpi` bir değeri, önceki döneme göre değişimini (bir ok ve yönü anlatan kelimelerle; değişim iyiyse yeşil, değilse kırmızı; maliyetler ve bekleme süreleri için `.lower_is_better()`), ulaşılan payla birlikte bir hedefi ve son değerlerin bir sparkline'ını gösterir.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` bir tutarı kullanıcının yerel ayarında para birimiyle gösterir (İngilizcede `₺184,250.50`, Türkçede `184.250,50 ₺`); hedefi de aynı para birimindedir.

Bir `Row` ya da `AutoGrid` içindeki Kpi'lar, eğilimleri olsun ya da olmasın, en uzun olan kadar yüksektir ve sparkline her kartın altında durur. Kpi'lardan oluşan bir `Row` her birine genişliğin dörtte birini verir: masaüstünde dördü satırı doldurur, satır yaklaşık 830 px altında iki, 480 px altında bir sütuna iner (satır üzerinde container query ile; bu yüzden dar bir sütundaki Kpi satırı daha erken kırılır). Grafikler kapsayıcılarının genişliğini doldurur ve oranlarını korur. Halka ve gösterge grafikleri en fazla 18rem yüksekliğinde kalır ve ortada durur.

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
