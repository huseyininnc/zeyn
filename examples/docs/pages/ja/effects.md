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

## トースト

`ui.toast(text)` は info のトーストを表示します。`ui.notify(Toast::success(text))` でトーンを選びます: `Toast::info`、`success`、`warning`、`danger` です。`.action(label, effect)` は、保存したレコードを開くなど、別のエフェクトを実行するボタンを追加します。

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

トーストは 4 秒間（danger は 8 秒間）表示され、ポインターがトースト上にある間やフォーカスを持っている間は時間が止まるため、ボタンに手が届きます。info、success、warning のトーストはステータス領域によって丁寧に読み上げられ、danger のトーストはアラートとして直ちに通知されます。すべてのトーストに閉じるボタンがあり、トーンは色の帯とテキストでも示され、同時に表示されるのは最大 3 つ（`MAX_TOASTS`）です。新しいものが出ると、最も古いものが取り除かれます。

@live toasts
