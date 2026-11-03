# Дерево и диаграммы

## Tree

`Tree::of(action)` показывает иерархию, которая загружается по требованию. Действие (action) принимает `TreeQuery` и возвращает дочерние элементы `q.parent` (`None` для верхнего уровня). Ветка загружает свои дочерние элементы при первом открытии, поэтому дерево над большой иерархией стоит один небольшой запрос на каждую открытую ветку.

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

Дерево следует шаблону ARIA tree: стрелки перемещают между элементами и открывают или закрывают ветки. Сервер ограничивает число дочерних элементов у ветки (`MAX_TREE_CHILDREN`).

## Диаграммы

`Chart::bar`, `stacked_bar`, `line`, `area`, `combo`, `scatter`, `donut`, `gauge` и `funnel` рисуют SVG на сервере. В браузере нет библиотеки диаграмм; диаграмма это часть HTML страницы.

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

У каждой диаграммы есть заголовок, текстовое описание для программ чтения с экрана, легенда и её числа в виде таблицы под *Show data*. Цвета рядов это токены от `--z-chart-1` до `--z-chart-6`; тест (`chart_colors_reach_three_to_one_in_both_themes`) проверяет контраст 3:1 относительно `--z-surface` и `--z-surface-2` в светлой и обеих тёмных темах (см. [Темы оформления](theming.md)).

Каждый столбец, точка, сектор, этап и значение индикатора это метка, в имени которой подпись и значение. SVG это группа, названная по заголовку и описанию, а диаграмма это одна точка остановки по Tab:

| Клавиша | Действие |
|---|---|
| Tab | Переходит в диаграмму, на метку, бывшую в фокусе последней (в начале на первую) |
| Влево, вправо | Предыдущая или следующая метка того же ряда (меняются местами при `dir="rtl"`) |
| Вверх, вниз | Та же точка предыдущего или следующего ряда |
| Home, End | Первая или последняя метка ряда |
| Escape | Скрывает подсказку |

Наведение, фокус и касание показывают тот же текст в подсказке, которая скрыта от программ чтения с экрана, чтобы ничего не читалось дважды. Без скрипта метки всё равно имеют имена, а таблица остаётся. Мелкие метки (точки по 8 px) меньше целевого размера 24 px; таблица под *Show data* это равноценный путь (WCAG 2.5.8).

## Ключевые показатели, спарклайны и bullet-диаграммы

`Kpi` показывает значение, его изменение по сравнению с прошлым периодом со стрелкой и словами о направлении (зелёный, когда изменение хорошее, красный, когда нет; `.lower_is_better()` для затрат и ожиданий), цель с достигнутой долей и спарклайн последних значений.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
```

В `DataGrid` колонка `Vec<f64>` рисуется как спарклайн, а колонка с числом или деньгами как bullet относительно цели:

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

Оба это один небольшой SVG на ячейку с именем вроде "Trend of 12 values from 40 to 61, lowest 12, highest 99". Бюджет: около 0,65 КБ HTML на строку для обеих ячеек; 10 000 строк с обеими отрисовываются примерно за 0,4 с в отладочной сборке (`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`), а таблицы отрисовывают только видимые строки. `charts.js` добавляет 3,7 КБ исходного (1,4 КБ gzip), а `charts.css` 3,3 КБ (1,1 КБ gzip).
