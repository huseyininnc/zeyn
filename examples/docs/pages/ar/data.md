# الشجرة والمخططات

## الشجرة

يعرض `Tree::of(action)` تسلسلًا هرميًا يُحمَّل عند الطلب. يأخذ الإجراء `TreeQuery` ويعيد أبناء `q.parent` (`None` للمستوى الأعلى). يحمّل الفرع أبناءه أول مرة يُفتح فيها، ولذلك فإن شجرة على تسلسل هرمي كبير تكلّف طلبًا صغيرًا واحدًا لكل فرع مفتوح.

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

تتبع الشجرة نمط ARIA tree: تتنقل مفاتيح الأسهم بين العناصر وتفتح الفروع أو تغلقها. يضع الخادم حدًا أعلى لعدد الأبناء في كل فرع (`MAX_TREE_CHILDREN`).

## المخططات

ترسم `Chart::bar` و`stacked_bar` و`line` و`area` و`combo` و`scatter` و`donut` و`gauge` و`funnel` رسوم SVG على الخادم. لا توجد مكتبة مخططات في المتصفح؛ فالمخطط جزء من HTML الصفحة.

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

لكل مخطط عنوان وملخص نصي لقارئات الشاشة ومفتاح إيضاح (legend) وأرقامه في جدول تحت *عرض البيانات*. ألوان السلاسل هي الرموز (tokens) من `--z-chart-1` إلى `--z-chart-6`؛ ويتحقق اختبار (`chart_colors_reach_three_to_one_in_both_themes`) من تباين 3:1 مقابل `--z-surface` و`--z-surface-2` في السمة الفاتحة وفي السمتين الداكنتين (راجع [السمات](theming.md)).

كل شريط ونقطة وشريحة ومرحلة وقيمة مقياس هو علامة (mark) اسمها تسميتها وقيمتها. وSVG مجموعة اسمها العنوان والملخص، والمخطط محطة Tab واحدة:

| المفتاح | ماذا يفعل |
|---|---|
| Tab | ينتقل إلى المخطط، إلى آخر علامة نالت التركيز (الأولى عند البداية) |
| Left, Right | العلامة السابقة أو التالية من السلسلة نفسها (تُعكس تحت `dir="rtl"`) |
| Up, Down | النقطة نفسها من السلسلة السابقة أو التالية |
| Home, End | أول علامة أو آخرها في السلسلة |
| Escape | يخفي التلميح (tooltip) |

يعرض التمرير فوق العنصر والتركيز واللمس النص نفسه في تلميح مخفي عن قارئات الشاشة حتى لا يُقرأ شيء مرتين. وبدون السكربت تبقى العلامات مسماة ويبقى الجدول. العلامات الصغيرة (نقاط بحجم 8 px) أقل من حجم الهدف 24 px؛ والجدول تحت *عرض البيانات* هو المسار المكافئ (WCAG 2.5.8).

## الأرقام الرئيسية والخطوط الموجزة والمخططات النقطية

يعرض `Kpi` قيمة، وتغيرها مقارنة بالفترة السابقة بسهم وكلمات تدل على الاتجاه (أخضر عندما يكون التغير جيدًا، وأحمر عندما لا يكون؛ `.lower_is_better()` للتكاليف والانتظار)، وهدفًا مع النسبة المنجزة، وخطًا موجزًا (sparkline) للقيم الأخيرة.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
```

في `DataGrid` يُرسم عمود `Vec<f64>` خطًا موجزًا، ويُرسم عمود الرقم أو المال مخططًا نقطيًا (bullet) مقابل هدف:

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

كلاهما SVG صغير واحد لكل خلية باسم مثل "Trend of 12 values from 40 to 61, lowest 12, highest 99". الميزانية: نحو 0.65 كيلوبايت من HTML لكل صف للخليتين معًا؛ وعرض 10,000 صف منهما يستغرق نحو 0.4 ثانية في بناء التنقيح (`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`)، وتعرض الشبكات الصفوف الظاهرة فقط. يضيف `charts.js` 3.7 كيلوبايت خامًا (1.4 كيلوبايت gzip) و`charts.css` 3.3 كيلوبايت (1.1 كيلوبايت gzip).
