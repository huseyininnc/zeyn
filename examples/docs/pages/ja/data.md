# ツリーとチャート

## Tree

`Tree::of(action)` は、必要に応じて読み込まれる階層を表示します。アクションは `TreeQuery` を受け取り、`q.parent` の子を返します（最上位では `None`）。ブランチは最初に開かれたときに子を読み込むため、大きな階層に対するツリーでも、開いたブランチごとに小さなリクエスト 1 回で済みます。

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

ツリーは ARIA の tree パターンに従います。矢印キーで項目間を移動し、ブランチを開閉します。サーバーは、ブランチごとの子の数に上限を設けます（`MAX_TREE_CHILDREN`）。

## チャート

`Chart::bar`、`stacked_bar`、`line`、`area`、`combo`、`scatter`、`donut`、`gauge`、`funnel` は、サーバー上で SVG を描画します。ブラウザにチャートライブラリはなく、チャートはページの HTML の一部です。

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

各チャートには、タイトル、スクリーンリーダー向けのテキストによる要約、凡例があり、その数値は *データを表示* の下に表として用意されています。系列の色は `--z-chart-1` から `--z-chart-6` のトークンです。テスト（`chart_colors_reach_three_to_one_in_both_themes`）が、ライトテーマと 2 つのダークテーマで、`--z-surface` と `--z-surface-2` に対して 3:1 を満たすことを検査します（[テーマ](theming.md) を参照）。

すべてのバー、点、スライス、段階、ゲージの値は、ラベルと値を名前に持つマークです。SVG はタイトルと要約で名前が付いたグループで、チャートは 1 つのタブストップです。

| キー | 動作 |
|---|---|
| Tab | チャートに入り、最後にフォーカスしたマークへ移動します（最初は先頭のマーク） |
| 左、右 | 同じ系列の前または次のマーク（`dir="rtl"` では入れ替わります） |
| 上、下 | 前または次の系列の同じ点 |
| Home、End | 系列の最初または最後のマーク |
| Escape | ツールチップを隠します |

ホバー、フォーカス、タップは同じテキストをツールチップに表示します。ツールチップはスクリーンリーダーからは隠されるため、二重に読み上げられることはありません。スクリプトがなくても、マークには名前が付いたままで、表も残ります。小さなマーク（8 px の点）は 24 px のターゲットサイズを下回りますが、*データを表示* の下の表が同等の手段になります（WCAG 2.5.8）。

## 主要指標、スパークライン、ブレット

`Kpi` は、値、前の期間に対する変化（矢印と方向を表す言葉付き。変化が良ければ緑、そうでなければ赤。コストや待ち時間には `.lower_is_better()`）、達成率付きの目標、最近の値のスパークラインを表示します。

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` は金額をユーザーのロケールで通貨付きで表示します（英語では `₺184,250.50`、トルコ語では `184.250,50 ₺`）。目標も同じ通貨で扱います。

同じ `Row` や `AutoGrid` に並ぶ Kpi は、トレンドの有無にかかわらず一番高いものと同じ高さになり、スパークラインは各カードの下端に置かれます。Kpi の `Row` は各 Kpi に幅の 4 分の 1 を割り当てます。デスクトップでは 4 つで行が埋まり、約 830 px 未満では 1 行に 2 つ、480 px 未満では 1 つに折り返します（行に対するコンテナクエリなので、狭い列の中の Kpi 行はより早く折り返します）。チャートはコンテナの幅いっぱいに広がり、縦横比を保ちます。ドーナツとゲージは高さ 18rem までで、中央に配置されます。

```rust
Row::new()
    .col(Kpi::money("Revenue", revenue).delta(12.4).trend(weekly))
    .col(Kpi::new("Orders", 1_284.0).delta(3.1))
    .col(Kpi::new("New customers", 312.0).delta(-2.0))
    .col(Kpi::money("Average order", average).delta(0.0))
```

`DataGrid` では、`Vec<f64>` の列はスパークラインとして、数値または金額の列は目標に対するブレットとして描画されます。

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

どちらも、セルごとに小さな SVG 1 つで、「Trend of 12 values from 40 to 61, lowest 12, highest 99」のような名前が付きます。予算: 両方のセルで 1 行あたり約 0.65 KB の HTML です。両方を持つ 10,000 行は、デバッグビルドで約 0.4 秒でレンダリングされ（`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`）、グリッドは表示範囲の行だけをレンダリングします。`charts.js` は生で 3.7 KB（gzip で 1.4 KB）、`charts.css` は 4.1 KB（gzip で 1.3 KB）追加します。
