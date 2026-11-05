# 画面とパラメータ

画面は、ルートを持つページ、または別の画面の上に開かれるウィンドウです。`#[screen]` は、アクションと同じようにアクセス権を宣言します。

## ページ

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` はページ、`Screen::window(title)` はウィンドウです。
- `.body(...)` を複数回呼ぶと、複数のコンポーネントが上から下へ並びます。
- 画面は `Screen` または `Result<Screen>` を返します。

## パラメータ

画面の最初の引数がそのパラメータです。`#[params]` の構造体か、何もなければ `_: ()` とします。それ以外の引数は、アクションと同様に注入されます。

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

パラメータは、使われるあらゆる場所で型が付きます。`Window::open(product_editor, 7)` はコンパイルできません。エディタは `ProductParams` を受け取るからです。

`#[params]` 構造体のすべてのフィールドが `Option` なら、画面はメニューのリンクや単純なアドレスからパラメーターなしでも開き、すべてのフィールドが `None` になります。省略できないフィールドには引き続き値が必要で、ない場合は 404 を返します。

`route` のない画面には URL がありません。リンク、ツールバーのボタン、メニュー、コマンドからウィンドウとして開きます。

## パス内のパラメータ

ルートでは、params のフィールドをパスのセグメントとして指定できます。それぞれの `{name}` は同名のフィールドに結び付き、そのフィールドの型として読み取られます。

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` は注文 42 を開きます。パスにないフィールドは、従来どおり JSON クエリで運ばれます: `/orders/42?params={"tab":"lines"}`。
- セグメントには、整数、エンティティキー（`OrderId`）、`String`、`Slug` を使えます。値は、フレームワークが書き出す形式でのみ解釈されます（`007` や `+7` ではなく `7`）。それ以外は 404 を返します。
- params のどのフィールドにも対応しないセグメント、params のない画面でセグメントを持つルート、セグメントにできない型のフィールドはコンパイルできず、エラーが何を直すべきかを示します。
- 複数のセグメントも同様に動作します: `slug: Slug` と `n: u32` を使った `#[screen(route = "/catalog/{slug}/{n}", public)]`。
- フレームワークが書き出すリンクはパス形式を使います。`Effect::navigate`、パンくず、コマンド、サインイン後の `return_to` です。セグメントのない画面は `?params={json}` のままです。
- セグメントを持つ画面は、何も埋められないため、メニュー項目やホーム画面にはできません。
- アクセス権は、他の画面と同様にリクエストごとに検査されます。

### スラッグ

`Slug::from_title("Çay Bardağı")` は `cay-bardagi` になります。小文字の ASCII、単語を `-` でつないだ形、トルコ語の文字の変換（`ı` と `İ` は `i`、`ş` は `s`、`ğ` は `g`、`ç` は `c`、`ö` は `o`、`ü` は `u`）、他のラテン文字のアクセント記号の除去が行われ、それ以外は取り除かれます。スラッグは最大 80 バイトです。パスでは正規の形だけが解釈されるため、`/catalog/Cay-Bardagi/1` は 404 を返します。

### 見つからない場合と移転

レコードがない場合は `Err(Error::NotFound)` を返します。ユーザーには 404 ページが表示されます。たとえば商品名が変わってアドレスが変わった場合は、新しいアドレスで応答します。ブラウザには恒久的リダイレクト（301）が返され、アプリ内ナビゲーションではページを読み込まずにこれに従います。

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)` は、アドレス自体は有効なまま、1 つのリクエストを別の Screen へ送ります。たとえばランダムな選択や最新の項目です。ブラウザは、遷移先の正規パスと `Cache-Control: no-store` を伴う `303 See Other` を受け取り、アプリ内ナビゲーションはページを読み込まずにそれをたどり、アドレスバーには遷移先が表示されます。遷移先は常にルートを持つ型付きの Screen なので、リダイレクトがアプリ外のアドレスへ向かうことはなく、アプリが提供していない遷移先には 404 が返ります。

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## アクセス

| オプション | 意味 |
|---|---|
| `permission = "..."` | ID にこの権限が必要です |
| `public` | 誰でも開けます。セッションのない訪問者には、Cookie なしでページが返されます（[公開ページ](public-pages.md) を参照） |
| `cache = 300` | 公開画面のみ。セッションのない訪問者のページを、ブラウザと共有キャッシュがこの秒数（最大 86400）だけ保持できます |
| `sensitive` | 開くことが、個人データの読み取りとして記録されます |
| `reason` | 開く前にユーザーが理由を入力する必要があります（[監査、マスキング、ジョブ](enterprise.md) を参照） |

ページは、`Screen` の `.description(..)`、`.canonical(..)`、`.og_image(..)`、`.json_ld(..)`、`.robots(..)` で、検索エンジンやリンクプレビューにも自身の情報を伝えます。[公開ページ](public-pages.md#search-engines-and-link-previews) を参照してください。

ユーザーが開けない画面を開くメニュー項目、ツールバーのボタン、コマンドは、HTML から省かれます。

サインインしていない訪問者向けの画面は `public` を宣言します。そのアクションにも独自のアクセス権が必要です。データを一覧表示する公開画面は `public` のアクションを呼び出さなければ、訪問者には一覧が空のままになります。

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

`permission` も `public` も書かないとコンパイルエラーになるため、画面が誤って公開されることはありません。

## ツールバーとコンテキストメニュー

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

`Alt+N` のようなショートカットは画面上で動作し、パレットに表示されます。ブラウザが使うショートカット（`Ctrl+P`、`Ctrl+S`、`F5`）は、ボタンが `.override_browser()` を呼ばない限り割り当てられません。ブラウザがページに渡さないもの（`Ctrl+N`、`Ctrl+T`、`Ctrl+W`）は決して割り当てられません。空いている組み合わせは `Alt+Shift+<letter>` と `Ctrl+Alt+<letter>` です。

ボタン、メニューグループ、ツールのアイコンは `Icon` から選びます: `Plus`, `Edit`, `Delete`, `Save`, `Search`, `Box`, `Cart`, `Table`, `Home`, `Chart`, `Users`, `Settings`, `Bell`, `Shield`, `Clock`, `Tag`, `Mail`, `SignOut`, `ArrowUp`, `ArrowDown`。これらは `zeyn.css` のマスクで、24 px のグリッドに 2 px の丸い線で描かれ、文字色を使い、スクリーンリーダーからは隠されます。意味は常にラベルが伝えます。

## ヘッダー

`.subtitle(text)` はタイトルの下に短い 1 行を置き、`.primary(|t| ...)` は画面の主要な操作をプライマリボタンとしてタイトル行の末尾に置きます。この 2 つで、ほとんどの管理画面にあるページヘッダーになります。

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary` は `.toolbar` と同じボタンを、同じショートカットとパレット項目で受け取ります。ツールバーはその他のツール用にヘッダーの下に残ります。サブタイトルはスクリーンリーダー向けの画面の説明です（`aria-describedby`）。行の幅が足りないと操作はタイトルの下に折り返し、スマートフォンでは先頭側にそろいます。サブタイトルも主要な操作もない画面は、これまでどおり普通の見出しで描かれます。

## Feature とメニュー

画面は Feature に属します。Feature のルートプレフィックスがルートの前に付き、そのガードは画面が開く前に実行され、メニューが画面を一覧にします。

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` は、`/` で画面を開くようにします。

## エラーページ

`App::not_found(screen)` と `App::server_error(screen)` で、アプリは 404 とサーバーエラー用の独自ページを持てます。

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- 404 の画面は、未知のパスと、`Err(Error::NotFound)` を返すすべての画面（解釈できないパスの値を含む）に応答します。
- サーバーエラーの画面は、`Error::Internal`、タイムアウト（504）、ハンドラー内のパニックに応答します。エラーの詳細はサーバーログにのみ出力され、画面はパラメータを受け取らず、リクエストについて何も知りません。
- どちらも、適切なステータスコードとともにシェルの中に表示されます。アプリ内ナビゲーションでは画面だけが取得され、ページを読み込まずに差し替えられます。フレームワークのエンドポイントと API は、引き続き JSON で応答します。
- エラー画面はパラメータを取らず、ルートも不要で、Feature にも属しません。`public` にしてください。開く権限のないユーザー、あるいはエラー画面自体が失敗した場合には、組み込みの短いメッセージが表示されます。

## 画面がブラウザに届くまで

最初のリクエストでは、メニュー、画面、アセットを含む完全なページが返されます。その後、アプリ内のリンクは画面だけを読み込んでその場で差し替えるため、メニュー、開いている状態、スクロール位置は保たれます。戻るボタン、新しいタブ、ブックマークは、通常のリンクと同じように動作します。
