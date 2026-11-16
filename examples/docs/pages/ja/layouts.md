# レイアウト

レイアウトコンポーネントは、他のコンポーネントを配置します。データを持たず、リクエストも増やしません。これらで作られた画面は、1 つの HTML 応答になります。

```rust
Stack::new()
    .add(Row::new().col(Card::new().title("Orders")).col(Card::new().title("Stock")).col(Card::new().title("People")))
    .add(
        Panel::new("Products").collapsible().add(
            Tabs::new("Views")
                .tab("Card", Card::new().title("A card inside a tab"))
                .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
        ),
    )
```

@live layout

## Stack

ノードを上から下へ並べます。`.gap(units)` は、ノード間の余白を間隔の単位（標準の密度では 4 px）で 0 から 12 の範囲で設定します。

## Row

列を横に並べます。`.col(node)` はそれぞれ幅の一部を占め、列の後に `.weight(n)` を付けるとその列が広くなります。狭い画面では列が縦に積まれます。`.compact()` はバッジのような小さな項目を詰めて配置します。

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card と Panel

`Card` は、任意のタイトルを持つ単純な面です。余白は 20 px、枠線は 1 本で、影はありません。`.action(node)` はタイトル行の末尾に操作を 1 つ置き（リンクボタンやメニュー）、`.footer(node)` は区切り線の下に「すべて表示」リンクのようなフッターを追加します。カードの中身がグリッドや一覧のときは端から端まで広がるため、枠線は 2 本ではなく 1 本になります。カードの中のカードは自分の箱を描きません。`Row` に並ぶカードは一番高いものに合わせて伸びます。`Panel` は、任意のツールバーを持つタイトル付きのセクションで、`.collapsible()` でユーザーが折りたためるようになり、`.collapsed()` で折りたたんだ状態から始まります。

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` に `.tab(title, node)` を付けて使います。矢印キーでタブ間を移動できます。これは ARIA の tabs パターンに沿っています。閉じているタブの内容は、最初に開かれるまでレンダリングされないため、タブが多い画面でも軽いままです。

`.remember(KEY)` は、ユーザーが開いたタブを choice キーの下に保持するため（[クライアント設定](security.md#client-preferences) を参照）、以降のページでも同じタブが開きます。URL のハッシュで指定されたタブが優先されます。

## Accordion

`Accordion::new()` と `.section(title, node)` は、セクションを縦に並べます。それぞれタイトルで開閉します。ヘルプページの回答や、長いレコードの各部分に使います。セクションは閉じた状態で始まります。セクションの後の `.open()` はそれを開きます。`.single()` は同時に開くセクションをひとつにします。その場合、開くよう指定された最初のセクションだけが開いた状態で始まります。accordion が持てるセクションは最大 50 個です（`MAX_ACCORDION_SECTIONS`）。

```rust
Accordion::new()
    .section(t!("help.password"), Paragraph::new(t!("help.password_answer")))
    .open()
    .section(t!("help.invoices"), Paragraph::new(t!("help.invoices_answer")))
    .single()
```

@live accordion

各セクションはネイティブの開閉要素（`<details>` と `<summary>`）で、見出しとボタンによる ARIA の accordion パターンではありません。ネイティブ要素はスクリプトを必要とせず、閉じたコンテンツを読み上げ順序の外に保ち、ブラウザーのページ内検索が一致を含むセクションを開けるからです。単一モードではセクションが `name` を共有し、別のセクションが開くとブラウザーが開いていたものを閉じます。`Tabs` と違い、閉じたセクションのコンテンツは最初からページ内にあります。

| キー | 動作 |
|---|---|
| `Tab` | 次のセクションのタイトルに移動します |
| `Enter` または `Space` | セクションを開閉します。フォーカスはタイトルに残ります |

## Split

`Split::new(start, end)` は、2 つのノードをハンドルを挟んで表示します。`.ratio(percent)` で開始側のサイズを設定します。ハンドルはポインターまたは矢印キーで動かせます。

## Dock

`Dock` は、中央の領域の周りにパネルを配置します。左、右、下、そして中央のタブがあり、ユーザーが移動、サイズ変更、閉じる、元に戻すことができます。各ユーザーのレイアウトは画面ごとに保存されます。[シェルとコマンド](shell.md) を参照してください。

## Surface

`Surface::new(label, width, height)` は固定サイズの領域で、フロアプラン上のテーブルのように項目を自由に配置できます。`.item(Draggable::new(id, node).at(x, y).size(w, h))` で項目を追加し、`.axis(Axis::X)` または `Axis::Y` で一方向だけに動かし、`.snap(step)` ですべての位置をグリッドに合わせます。`.on_move(action)` は移動を保存します。サーバーが id がこの領域のものかを確認し、位置を境界内、グリッド上、軸上に収めたあと、アクションは項目の id と新しい `x` と `y` を持つ `SurfaceMove` を受け取ります。

`.resizable()` は項目の右下隅にハンドルを付けます。`.min_size(w, h)`、`.max_size(w, h)`、`.aspect(w, h)` はサイズを制限し、項目をリサイズ可能にもします。アクションは新しい `w` と `h` も受け取り、それらは制限、サーフェス、グリッドの内側に保たれます。縦横比を指定すると、高さは幅に従います。

```rust
Surface::new(t!("floor.plan"), 800, 480)
    .snap(20)
    .on_move(move_table)
    .item(
        Draggable::new("t1", Paragraph::new(t!("floor.table")))
            .at(40, 60)
            .size(120, 80),
    )
```

マウスやペンで項目をドラッグできます。タッチでは短く長押ししてからドラッグします。ポインター一つで操作する場合は、クリックで項目を選び、領域をクリックするとそこへ置かれます。隅のハンドルをクリックするとサイズを選び、サーフェスをクリックするとそこに隅を置きます。

| キー | 動作 |
|---|---|
| 矢印キー | フォーカスのある項目をグリッド一つ分動かします（グリッドがない場合は 10 ピクセル） |
| `Shift` + 矢印キー | リサイズ可能な項目のサイズをグリッド 1 ステップ分変える |
| `Escape` | 選んだ項目を解除するか、ドラッグ中の項目を元に戻します |
