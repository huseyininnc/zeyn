# エフェクトとウィンドウ

エフェクトは、クリックやアクションの成功の後にブラウザが行うことです。ウィンドウを開く、画面へ移動する、トーストを表示する、確認を求める、ウィンドウを閉じてその下の画面を更新する、などがあります。Rust で宣言され、ランタイムが実行するため、JavaScript を書く必要はありません。

## エフェクト一覧

| エフェクト | 構築方法 | 結果 |
|---|---|---|
| ウィンドウを開く | `Window::open(screen, params)` | 画面をモーダルウィンドウで開きます |
| 移動 | `Effect::navigate(screen, params)` | リンクのように、その場で画面へ移動します |
| トースト | `Effect::Toast { message }` | 短いメッセージを表示します |
| トーン付きトースト | `Toast::success(message).action(label, effect).into()` | トーン（info、success、warning、danger）を持つトースト。別のエフェクトを実行するボタンを任意で付けられます |
| 確認 | `Effect::confirm(message, then)` | ユーザーが確認した後に `then` を実行します |
| 閉じて更新 | `Effect::CloseAndRefresh` | ウィンドウを閉じ、それを開いた画面を再読み込みします |
| トピックを更新 | `Effect::refresh("orders")` または `ui.refresh(["orders", "stock"])` | トピックを listen しているページ上のノードをその場で更新します（[トピック](#topics) を参照） |

パラメータには型があります。パラメータが合わない画面へのエフェクトはコンパイルできません。

ルートにパスのセグメントがある画面への `Effect::navigate`（[パス内のパラメータ](screens.md) を参照）は、`/orders/42` のようなパス形式で遷移します。パスにないパラメータは `params` クエリに入ります。

## アクションの後

フォームとアクションボタンは `on_success` ハンドラーを受け取ります。これはアクションの結果と、エフェクトを集める `UiCtx` を受け取ります。

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx` には `toast`、`notify`、`close_and_refresh`、`navigate` があり、確認のようなそれ以外のものには `effect` を使います。

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## ウィンドウ

ウィンドウは、ルートを持たない画面で、`Screen::window(title)` で作ります。ウィンドウは積み重なり、ウィンドウから別のウィンドウを開けます。Escape キーまたは閉じるボタンで最前面のウィンドウが閉じ、フォーカスはそれを開いた要素に戻ります。`.width(px)` で幅を設定します。

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## エフェクトの発生元

- グリッドのリンクと行を開く操作: `.link(|p| Window::open(...))`、`.on_open(|p| ...)`。
- ツールバーのボタン: `.opens(screen, params)`。
- コンテキストメニュー: `.item(label, effect)`。
- コマンド: `Command::open`、`Command::navigate`、`Command::effect`。
- フォームとアクションボタンの `on_success` ハンドラー。

エフェクトがページを再読み込みすることはありません。ナビゲーションは新しい画面だけを読み込み、更新はウィンドウの下の画面だけを再読み込みします。

## トピック

画面の一部が変わると、ほかの部分も同じデータを表示していることがよくあります。保存した注文は合計を変え、フィルターはグリッドとグラフを変えます。トピックは、データをブラウザーに通さずにこれらをつなぎます。データを表示するノードに `.listens(topic)` を付けると、そのトピックが指定されたときにその場で更新されます。

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

トピックは次の 3 つの方法で指定されます。

- action がデータを変えた後、`on_success` ハンドラーの中で `ui.refresh("sales")`（または `Effect::refresh`）。
- `.drives("sales")` を持つフィルターコントロール。フィルターチップ、検索ボックス、送信ボタンのないフォーム（セレクト、日付範囲、テキスト）です。[フィルターフォーム](forms.md#filter-forms) を参照してください。
- ほかの人がデータを変えたとき、realtime チャネル（feature `realtime`）からの `hub.notify(&ctx, "sales")`。`docs/realtime.md` を参照してください。

`.listens` はどのコンポーネントでも使えます。コンポーネント自身の builder メソッドの後、最後に呼び出してください。グリッドを持つリスナーは、グリッドのエンドポイントを通じてグリッドを再読み込みし、ページ、並び順、フィルターを保ちます（フィルターによる変更では最初のページから始まります）。それ以外のリスナーはすべて、1 回の `POST /_zeyn/nodes` でまとめてサーバー上で再レンダリングされるため、KPI とグラフで 1 リクエストです。送られるのはトピック名だけで、サーバーはほかのリクエストと同じくユーザーのアクセス権と params で画面を開くので、更新によってユーザーが見られないものが表示されることはありません。

1 つのアニメーションフレーム内で指定されたトピックはまとめて 1 回処理され、同じ画面で実行中の更新は新しい更新によって取り消され、ページ上にあるノードだけが更新されます。新しい内容に同じ要素（id、test id、またはリストの行キーで判断）があれば、フォーカスと開いている `<details>` はそのまま残ります。

トピック名は 1 から 64 文字の英字、数字、`-`、`_`、`.` です。`App::check_page` は、ページで drive されているのにそのページで誰も listen していないトピックを報告し、`zeyn check` は、更新または drive されているのに誰も listen していないトピックや、listen されているのに更新、drive、通知のどれもされないトピックについて警告します。

@live topics

## トースト

`ui.toast(text)` は info のトーストを表示します。`ui.notify(Toast::success(text))` でトーンを選びます: `Toast::info`、`success`、`warning`、`danger` です。`.action(label, effect)` は、保存したレコードを開くなど、別のエフェクトを実行するボタンを追加します。

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

トーストは 4 秒間（danger は 8 秒間）表示され、ポインターがトースト上にある間やフォーカスを持っている間は時間が止まるため、ボタンに手が届きます。info、success、warning のトーストはステータス領域によって丁寧に読み上げられ、danger のトーストはアラートとして直ちに通知されます。すべてのトーストに閉じるボタンがあり、トーンは色の帯とテキストでも示され、同時に表示されるのは最大 3 つ（`MAX_TOASTS`）です。新しいものが出ると、最も古いものが取り除かれます。

@live toasts
