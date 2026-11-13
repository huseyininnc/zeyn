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

`.draggable()` を付けると、葉を island などのドロップ先へドラッグできます。ドラッグは MIME タイプ `application/x-zeyn-tree` で JSON `{ "id", "path", "label" }`（`path` は最上位から葉までの id の並び）を、`text/plain` でラベルを運びます。ドロップ先は `dragover` ハンドラーで `dropEffect = "copy"` を設定します。ドラッグしない場合は、フォーカスのある葉の挿入ボタン（ポインター用）と Control+Enter、Mac では Command+Enter（キーボード用）が、同じオブジェクトを `detail` に持つキャンセル可能な DOM イベント `zeyn:tree-insert` を送ります。自分の現在位置に挿入するドロップ先は `preventDefault()` を呼び、そのときだけツリーは挿入をスクリーンリーダーに知らせます。葉にフォーカスがある間 Control+Enter はツリーのものなので、同じキーのアプリのショートカットはそこでは動きません。どのページでもこのデータを偽造できるため、ドロップ先はこれを信頼できない入力として扱い、サーバーでアクセスを確認します。ドラッグのコード（`treedrag.js`）はドラッグできるツリーがあるページでだけ読み込まれます。

```rust
Tree::of(report_fields).label("Fields").draggable()
```

`.movable(move_item)` で項目を移動できます。マウスやペンで項目を別の行の上部か下部（その前か後ろ）、またはブランチの中央（その中）にドラッグし、落ちる位置は印で示されます。フォーカスのある項目の「移動」ボタンはメニューを開きます（上へ、下へ、ブランチの外へ、上のブランチの中へ）。これは単一ポインターで移動する手段で、タッチもこれを使います。Alt と矢印キーではすぐに移動します。アクションは `POST /_zeyn/data` を通じて CSRF、書き込み制限、その権限の検査を経て `TreeMove`（`id`、`target`、`place`）を受け取ります。どちらの id もブラウザーから来るのでアクションはそれを検索し、`TreeMove::parent` は新しい親を返し、項目を自分自身やその下へ移すことを拒否します（`Error::Conflict`。親の連鎖が `MAX_TREE_DEPTH` より長い場合も同じ）。移動の後、ツリーは変わった階層を読み込み直し、移動した項目にフォーカスを保ちます。移動のコードは `treedrag.js` の一部で、ドラッグ可能か移動可能なツリーと一緒にだけ読み込まれます。

```rust
#[action(permission = "folders.write")]
pub async fn move_folder(m: TreeMove, store: Folders) -> Result<()> {
    let parent = m.parent(|id| store.parent_of(id))?;
    store.place(&m.id, parent, &m.target, m.place)
}

Tree::of(folders).label("Folders").movable(move_folder)
```

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

各チャートには、タイトル、スクリーンリーダー向けのテキストによる要約、系列が 2 つ以上あるときの凡例があり、その数値は *データを表示* の下に表として用意されています。棒、折れ線、面、複合チャートは SVG で図形だけを描き、図形はカードに合わせて伸縮します（`--z-chart-h`、240 px、スマートフォンでは 200 px）。目盛りのラベルはその横の HTML なので、どの幅でも 12 px のままで、1 万以上は短く表記され（`600K`、`1.2M`）、カテゴリのラベルは最大 12 個まで（幅 480 px 未満のチャートではその半分）に間引かれます。棒はその帯の 60% を占めます。折れ線は 2 px で点はなく、最後の値とホバー中のカテゴリにだけ丸い点が付きます。面はフェードします。カテゴリにポインターかキーボードのフォーカスが当たると、十字線と、そのカテゴリのすべての系列を示すツールチップが表示されます。系列の色は `--z-chart-1` から `--z-chart-8` のトークンで、最初の 5 色は色覚の違いがあっても区別できる順序に並んでいます。系列 7 と 8 は線を破線でも描きます。起動時のチェックとテスト（`chart_colors_reach_three_to_one_in_both_themes`）が、すべての系列が `--z-surface` に対して 3:1 を満たすことを確認します（[テーマ](theming.md) を参照）。

すべてのバー、点、スライス、段階、ゲージの値は、ラベルと値を名前に持つマークです。SVG はタイトルと要約で名前が付いたグループで、チャートは 1 つのタブストップです。

| キー | 動作 |
|---|---|
| Tab | チャートに入り、最後にフォーカスしたマークへ移動します（最初は先頭のマーク） |
| 左、右 | 同じ系列の前または次のマーク（`dir="rtl"` では入れ替わります） |
| 上、下 | 前または次の系列の同じ点 |
| Home、End | 系列の最初または最後のマーク |
| Escape | ツールチップを隠します |

ホバー、フォーカス、タップは同じテキストをツールチップに表示します。ツールチップはスクリーンリーダーからは隠されるため、二重に読み上げられることはありません。スクリプトがなくても、マークには名前が付いたままで、表も残ります。小さなマーク（8 px の点）は 24 px のターゲットサイズを下回りますが、*データを表示* の下の表が同等の手段になります（WCAG 2.5.8）。

## Gantt

プロジェクトの計画のように、タスクを日数の範囲にわたるバーとして表示します。`Gantt::new(label, start, end)` は範囲を設定します。最長 2 年です。`.task(title, from, to)` はタスクを追加し、`.done(percent)` と `.tone(...)` は最後に追加したタスクに適用され、`.today(day)` はその日に線を引きます。各タスクの開始日と終了日をテキストで示す表なので、バーはそれを繰り返しているだけです。バーは範囲の両端で切り詰められます。

```rust
Gantt::new(t!("project.plan"), project.starts, project.ends)
    .task(t!("project.design"), design.from, design.to)
    .done(design.percent)
    .today(ctx.today())
```

@live gantt

## OrgChart

会社の人員や製品の部品のような階層を、いちばん上の 1 つのボックスから線でつないだボックスとして表示します。`OrgNode::new(name)` はボックス、`.role(text)` は名前の下の控えめな行、`.child(node)` はその下のボックスです。入れ子のリストなので、スクリーンリーダーは階層を読み取れます。図は最大 8 階層、200 個のボックスまで描き、幅が広い場合は自身の枠の中でスクロールします。

```rust
OrgChart::new(
    t!("company.chart"),
    OrgNode::new(director.name.clone())
        .role(t!("company.director"))
        .child(OrgNode::new(lead.name.clone())),
)
```

## FlowDiagram

プロセスのステップを、矢印でつないだボックスとして開始側から終了側へ描きます。カートから配達までの注文や、分岐のある承認などです。`.node(id, title)` はステップを追加し、`.tone(...)` は最後に追加したステップに色を付け、`.edge(from, to)` は id で指定した二つのステップの間に矢印を描き、`.edge_with(from, to, label)` は言葉の付いた矢印を描きます。サーバーは最初のステップからの距離に応じてステップを列に配置し、前のステップへ戻る矢印はボックスの下を通ります。ステップは順序付きリストで、それぞれがスクリーンリーダー向けに次のステップを示します。図は最大 40 ステップと 80 本の矢印を描き、幅が広いときは自身のボックス内でスクロールします。

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

訪問、カート、注文のように、段階ごとに一部が減っていくプロセスの各段階です。`Funnel::new(label)` と `.stage(title, value)` は、各段階を、最初の段階に対する割合の幅を持つ中央寄せのバーとして描き、数値とその割合をテキストで示します。ファネルが持てる段階は最大 10 個です。

```rust
Funnel::new(t!("sales.funnel"))
    .stage(t!("sales.visits"), visits)
    .stage(t!("sales.orders"), orders)
```

## ActivityRings

1 日の目標のように、全体に対する割合を最大 4 つ、入れ子のリングとして表示します。`.ring(title, value, max, tone)` は外側から内側へリングを追加し、`.size(px)` は直径を設定します。それぞれの名前とパーセントはリングの横に書かれるので、図そのものはスクリーンリーダーから隠されます。

```rust
ActivityRings::new(t!("day.goals"))
    .ring(t!("day.packed"), packed, 60, Tone::Brand)
    .ring(t!("day.calls"), calls, 20, Tone::Success)
```

@live visuals

## ComparisonTable

プランや製品を横に並べます。それぞれが 1 列、機能ごとに 1 行です。`.plan(name)` は列を追加し、`.featured()` は最後に追加したプランを目立たせます。`.row(feature, cells)` はプランごとにセルを受け取ります。`true` と `false` はチェックと × になり、「含まれる」「含まれない」と読み上げられ、テキストはそのまま書かれます。最大 6 プラン、100 行まで持てます。

```rust
ComparisonTable::new(t!("pricing.plans"))
    .plan(t!("pricing.free"))
    .plan(t!("pricing.team"))
    .featured()
    .row(t!("pricing.audit_log"), [false, true])
```

## Table

画面がすでに持っているテキストによる小さな表です。料金表のプラン、店舗の営業時間、レポートの合計などに使います。`Table::new(caption)` は表の名前になるキャプションを受け取り、`.head([...])` で列見出しを、`.row([...])` で各行を指定します。`.row_headers()` は各行の最初のセルをその行の見出しにするので、スクリーンリーダーは各セルの列と行を読み上げます。`.numeric(index)` は数値の列を末尾側にそろえ、`.striped()` は 1 行おきに背景色を付けます。見出しより短い行は空のセルで埋められ、長い行は切り詰められます。表が持てるのは最大 200 行、12 列です。ページ送り、並べ替え、フィルターはなく、リクエストも送りません。アクションから取得する行や、ユーザーが操作する行は `DataGrid` に置きます。狭い画面では自身の枠の中で横にスクロールし、その枠がフォーカスを受け取るので矢印キーでスクロールできます。

```rust
Table::new(t!("shop.opening_hours"))
    .head([t!("shop.day"), t!("shop.opens"), t!("shop.closes")])
    .row([t!("shop.weekdays"), "09:00".into(), "18:00".into()])
    .row([t!("shop.saturday"), "10:00".into(), "14:00".into()])
    .row_headers()
```

@live table

## Heatmap

受けた注文数や記録した作業時間など、直近の数週間の各日の数値を、四角形のカレンダーとして表示します。1 列が 1 週間、1 行が曜日で、数値が大きいほど濃くなります。`Heatmap::ending(label, end)` は `end`（通常は今日）を含む週で終わり、それより後の日は空のままです。`.weeks(n)` は表示する週の数を設定します。指定しなければ 17 週で、最大 53 週です。`.value(day, n)` または `.values(days)` で数値を、`.tone(...)` で色を指定します。表示されている中で最大の数値が、4 段階のうち最も濃い色になります。週は、ユーザーの言語で週が始まる曜日から始まります。

```rust
Heatmap::ending(t!("sales.orders_a_day"), ctx.today())
    .weeks(12)
    .values(orders.iter().map(|o| (o.day, o.count)))
    .tone(Tone::Success)
```

@live heatmap

サーバーが描く表で、スクリプトは不要です。スクリーンリーダーは行の曜日と、各日の日付と数値をユーザーの言語で読み上げるので、濃淡はテキストが伝える内容を繰り返しているだけです。四角形にポインターを合わせると同じテキストが表示されます。狭い画面では表が自身の枠の中で横にスクロールし、その枠がフォーカスを受け取るので矢印キーでスクロールできます。1 日は 2 つの要素です。既定の 17 週はコンポーネント 1 つあたり 300 要素という予算に収まりますが、1 年分は収まりません。

## 主要指標、スパークライン、ブレット

`Kpi` は、値をカードで最も大きな文字で表示します（30 px、等幅の数字、通貨記号や単位は小さく淡色）。前の期間に対する変化は `+48.8%` のような小さなチップで示し、横に「前の期間比」と添えます（変化が良ければ緑、そうでなければ赤、0.5% 未満なら灰色。コストや待ち時間には `.lower_is_better()`。スクリーンリーダーには文全体が読み上げられます）。さらに、達成率付きの目標と、最近の値の推移をチップと同じ色でフェードする面の上に全幅で表示します。7 桁以上の値は短く表記され（`$1.23M`、トルコ語では `1,23 Mn`）、完全な値はツールチップとスクリーンリーダーで伝えられます。

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` は金額をユーザーのロケールで通貨付きで表示します（英語では `₺184,250.50`、トルコ語では `184.250,50 ₺`、記号は小さく表示）。目標も同じ通貨で扱います。

同じ `Row` や `AutoGrid` に並ぶ Kpi は、トレンドの有無にかかわらず一番高いものと同じ高さになり、スパークラインは各カードの下端に置かれます。Kpi の `Row` は各 Kpi に幅の 4 分の 1 を割り当てます。デスクトップでは 4 つで行が埋まり、約 830 px 未満では 1 行に 2 つに折り返し、360 px 未満ではカードがトレンドを省きます（行に対するコンテナクエリなので、狭い列の中の Kpi 行はより早く折り返します）。各カードは、トレンドの有無にかかわらず 132 px 以上を保ちます。チャートはコンテナの幅いっぱいに広がります。ドーナツ、ゲージ、ファネル、散布図は縦横比を保ち、ドーナツとゲージは高さ 18rem までで、中央に配置されます。

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
