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
| `.badge(\|v\| tone)` | 値をトーンのラベルとして、その淡い色の上に表示します |
| `.status(\|v\| tone)` | 値をステータスとして表示します。通常の文字の前にトーンの点を置く、バッジより控えめな形です |
| `.progress(max)` | 0 から `max` までのバー |
| `.check_icon()` | `true` にチェックマークを表示します |
| `.link(\|row\| effect)` | セルを、エフェクトを実行するリンクにします |
| `.pin_start()`, `.hidden()` | 先頭に固定する、またはユーザーが表示するまで非表示にします |
| `.editable()` | その場で編集でき、`.edit_with(action)` で保存されます |
| `.plain()` | 整数を桁区切りなしで表示します。コードや年に |
| `.wrap()` | 長い値を複数行に折り返します。指定しなければセルは 1 行のままで末尾が省略記号になり、すべての行が同じ高さになります |

行の高さは 40 px（コンパクトな密度では 32 px）で、区切りは線だけです。非常に幅の広いグリッドでは `DataGrid::zebra()` で 1 行おきに色を付けられます。セル内のリンクは太さ 500 の文字として見え、ホバーで下線が付きます。

数値と金額は右寄せで、等幅の数字で表示されます。整数はグリッドのセルや `DescriptionList` などの表示値で、ユーザーのロケールに従って桁区切りされます（英語は `1,234,567`、トルコ語は `1.234.567`）。`#[key]` の ID は区切らず、`.plain()` で 1 列だけ区切りをやめられます。

## 行

- `.on_open(|row| effect)`: 行のダブルクリックまたは Enter。
- `.row_action(label, action, |row| input)`: 各行のボタン。
- `.bulk_action(label, action, M::id)`: 選択した行に対して実行します。
- `.row_action` または `.bulk_action` の直後の `.confirm(message)` と `.tone(Tone::Danger)`: その操作の実行前に `ActionButton::confirm` と同じダイアログで確認し（フォーカスは最初にキャンセル）、ボタンを赤で描きます。
- `.row_menu(|row| ContextMenu)`: 行の右クリックメニュー。
- `.row_style(|row| Option<RowStyle>)`: danger、warning、success の行。
- `.exportable()`: フィルター後の行の CSV エクスポート。サーバーがストリーミングします。

## 実行時に決まる列

`DataGrid::dynamic(action)` は、SQL のプレビューのように列が行と一緒に届く結果のためのものです。action は、列(名前、ラベル、`Integer`、`Decimal`、`Date` などの `DynamicKind`)と JSON で表した 1 ページ分の行を持つ `DynamicPage` を返します。

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

ページング、仮想スクロール、キーボード操作、ユーザーの言語での数値と日付の書式は、型付きの grid と同じように動きます。名前、ラベル、セルはエスケープされ、サーバーが列数、行数、テキストの長さを制限します。この grid は並べ替えも絞り込みもせず、行は読み取り専用です。詳しくは [DataGrid リファレンス](grid.md) を参照してください。

## その場での更新

最後に呼び出した `.listens(topic)` により、トピックが指定されたときにグリッドがその場で再読み込みされます（[トピック](effects.md#topics) を参照）。`ui.refresh(topic)` の後は並び順とフィルターを保ったまま現在のページを再読み込みし、`.drives(topic)` を持つフィルターコントロールが変わった後は新しい画面の params で最初のページから始めます。グリッドはすべての permission 検査を伴う `POST /_zeyn/grid` で再読み込みされ、フィルターが変えた画面の param は画面関数を通じて、たとえば scope としてグリッドに届きます。

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

[DataGrid リファレンス](grid.md) では、保存済みビュー、マスター詳細、グルーピング、ツリーデータ、インライン編集、セルのナビゲーション、コピーと貼り付け、列の仮想化、アクセシビリティを扱います。
