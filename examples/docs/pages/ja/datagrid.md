# DataGrid

`DataGrid` は、`DataQuery` を受け取り `Page<E>` を返すアクションの行を表示します。ページング、並べ替え、フィルター、合計はサーバー上で実行されるため、100 万行のグリッドでも、100 行のグリッドと同じ負荷しかブラウザにかかりません。

## グリッド

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

見出しをクリックして並べ替え、フィルターボックスに入力し、見出しをドラッグして列を移動し、その端をドラッグして幅を変えてみてください。合計は、ページだけでなく、フィルター後のすべての行を対象にします。

## アクション

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply` は、メモリ上の `Vec` に対してフィルター、並べ替え、合計、ページングを行います。小さなリストやテストに向いています。データベースを背後に持つストアは、同じ `DataQuery`（ページ、並べ替え、フィルター、合計）を読み取り、SQL で応答します。[データとデータベース](database.md) を参照してください。

## ページング

| ページング | 用途 |
|---|---|
| `Paging::pages(n)` | 番号付きページ。指定がなければ 50 行です |
| `Paging::infinite(n)` | スクロールに合わせて行を追加で読み込みます |
| `Paging::virtual_scroll()` | 非常に大きなリスト。見えている行だけがページに含まれます |
| `Paging::none()` | 1 ページに収まる短いリスト |

クライアントが何を要求しても、サーバーはページサイズに上限を設けます。

## 列

| 呼び出し | 結果 |
|---|---|
| `.width(px)`, `.flex(n)` | 固定幅、または空き幅に対する割合 |
| `.filter()` | 型に合ったフィルターボックス: テキスト、数値範囲、日付範囲、選択肢リスト |
| `.sum()`, `.avg()`, `.min()`, `.max()` | フッターの合計。テキスト列への `sum` はコンパイルできません |
| `.badge(\|v\| tone)` | 値を色付きのバッジとして表示します |
| `.progress(max)` | 0 から `max` までのバー |
| `.check_icon()` | `true` にチェックマークを表示します |
| `.link(\|row\| effect)` | セルを、エフェクトを実行するリンクにします |
| `.pin_start()`, `.hidden()` | 先頭に固定する、またはユーザーが表示するまで非表示にします |
| `.editable()` | その場で編集でき、`.edit_with(action)` で保存されます |

## 行

- `.on_open(|row| effect)`: 行のダブルクリックまたは Enter。
- `.row_action(label, action, |row| input)`: 各行のボタン。
- `.bulk_action(label, action, M::id)`: 選択した行に対して実行します。
- `.row_menu(|row| ContextMenu)`: 行の右クリックメニュー。
- `.row_style(|row| Option<RowStyle>)`: danger、warning、success の行。
- `.exportable()`: フィルター後の行の CSV エクスポート。サーバーがストリーミングします。

[DataGrid リファレンス](grid.md) では、保存済みビュー、マスター詳細、グルーピング、ツリーデータ、インライン編集、セルのナビゲーション、コピーと貼り付け、列の仮想化、アクセシビリティを扱います。
