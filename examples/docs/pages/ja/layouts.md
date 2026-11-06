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

## Split

`Split::new(start, end)` は、2 つのノードをハンドルを挟んで表示します。`.ratio(percent)` で開始側のサイズを設定します。ハンドルはポインターまたは矢印キーで動かせます。

## Dock

`Dock` は、中央の領域の周りにパネルを配置します。左、右、下、そして中央のタブがあり、ユーザーが移動、サイズ変更、閉じる、元に戻すことができます。各ユーザーのレイアウトは画面ごとに保存されます。[シェルとコマンド](shell.md) を参照してください。
