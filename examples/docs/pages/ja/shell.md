# シェルとコマンド

シェルは、すべての画面を囲むフレームです。メニュー、言語、テーマ、タイムゾーンのピッカー、トースト、コマンドパレットがあります。デスクトップツールのように動くアプリでは、メニューバー、ツールボックス、プロパティインスペクター、ステータスバー、ドッキングパネルが加わります。

## メニュー

メニューは、Feature の `.menu(...)` の呼び出しから作られます。ユーザーが開けない項目は HTML から省かれ、ガードがユーザーを拒否する Feature は何も表示しません。現在の画面のリンクには `aria-current="page"` が付きます。

## サイトシェル

業務画面は、サイドメニューのあるアプリシェルで開きます。ショップや商品カタログのような公開ページは、代わりにサイトシェルで開くことができます。アプリ名をブランドリンクとし、横型のメニュー、狭い画面向けのメニューボタン、任意のアクション、フッターを備えます。どちらを使うかは Feature が選び、他の Feature の画面はアプリシェルのままなので、1 つのアプリに両方が共存します。

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **メニュー**。上部のメニューには、サイトシェル内のすべての Feature のメニューが並びます。項目が 1 つのグループは通常のリンクで、それより大きなグループは、リンクを開くボタンです（W3C APG の disclosure navigation パターン）。Enter、Space、クリックでグループを切り替え、下矢印で開いて最初のリンクへ移動し、上矢印、下矢印、Home、End でリンク間を移動し、Escape で閉じてボタンに戻り、Tab でグループの外へ出るか外側をクリックすると閉じます。左矢印と右矢印は、最上位の項目間を移動します。
- **狭い画面**。800 px 未満では、メニューは `aria-expanded` を持つメニューボタンの陰に折りたたまれ、Escape で閉じます。スクリプトがなければメニューは開いたままになるため、到達できなくなるものはありません。
- **ヘッダースロット**。上部のバーには、論理的な名前を持つアプリバーのように、任意のコンポーネントを受け取る四つのスロットがあります。ブランドの前の `start(node)`、メニューと end の間の `center(node)`、バーの末端の `end(node)`、その下の全幅の行となる `bottom(node)`（chips など）です。各スロットは複数のノードを順に保持し、start と end は右から左に書く言語では左右が入れ替わります。スロット内のコンポーネントは、ユーザーの言語、権限、マスクで描画されます。`SearchBox` のように自身の action でデータを読み込むものはそこで動作し、Feature のガードと action の権限が確認されます。grid と form は画面に置くものです。スマートフォンでは、start と end のスロットはバーに残り、center スロットはアプリが指定した場所に移ります。`center_on_phone(CenterOnPhone::Row)`（デフォルト）はバーの下の行に置き、`CenterOnPhone::Icon` はそれを開く虫眼鏡ボタンの陰に置いて（W3C APG の disclosure）フォーカスを中へ移し、Escape で閉じてボタンに戻ります。`center_button(icon, label)` はそのボタンを変更します。スマートフォンのレイアウトはスクリプトが実行される前に CSS で決まるため、ページの読み込み中にヘッダーが動くことはありません。
- **アクション**。`sign_in()` は、アプリにサインインページがあれば、訪問者にはサインインのリンクを、サインイン済みのユーザーにはサインアウトのリンクを表示します。`language()` と `theme()` は、アプリシェルと同じピッカーを表示します。これらは end スロットの後ろに来て、スマートフォンではメニューに折りたたまれます。
- **ボトムバー**。`bottom_nav([...])` は、スマートフォン（幅 800 px 以下）で画面の下部に固定される最大五つのリンクを追加します。人が最もよく行く場所のためのもので、六つ以上はコンパイルできません。各項目は、ラベル、ルートを持つ画面、任意のアイコンを持つ `BottomNavItem` です。表示中のページのリンクには `aria-current="page"` が付き（`nav.js` が最新に保ちます）、ユーザーが開けない画面へのリンクは除かれ、バーは `env(safe-area-inset-bottom)` でホームインジケーターを避け、バーが何も隠さないようにページの下に余白が確保されます。残りの場所にはトップメニューが残ります。バーにスクリプトは不要です。
- **フッター**。画面へのリンク（`link`。開けないユーザーには省かれます）またはアドレス（`url`: パス、`https:`、`http:`、`mailto:`。`App::check` は他のスキームや、登録されていない画面へのリンクを拒否します）の列と、テキスト 1 行です。

サイトシェルの画面とアプリシェルの画面の間の移動は、ページを読み込まずに `nav.js` を通して行われます。部分応答は自身がどのシェルに属するかを示し（`x-zeyn-shell`）、それが現在のフレームと異なる場合、ランタイムは言語変更のときと同じ方法でフレーム全体を差し替え、もう一方のシェルのスタイルシートとスクリプトを読み込みます。`/_zeyn/route/{screen}` へのリンク（パンくずのもの）も、同じように辿られます。

## 画面内のテーマ選択

`ThemePicker::new()` は、設定ページの「外観」セクションのように、画面の中にテーマ選択を置きます。`App::themes` のテーマ（アプリが端末に従う場合は「システム」も）を提示し、使用中のテーマを表示します。

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

シェルのテーマ選択と同じコントロールです。選択はページを再読み込みせずにすぐ適用され、`zeyn_theme` クッキー（サインイン中のユーザーはビューストアにも）に保存され、ページ上のもう一方の選択にも反映されます。アプリが選択肢を提供しない場合（`theme_picker(false)` やテーマが 1 つだけの場合）は何も描画しません。

## コマンド

コマンドは、ラベル、ショートカット、動作とともに Rust で一度だけ宣言します。Feature に登録するとパレットに一覧表示され、そのショートカットはすべてのページで動作します。

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| コンストラクター | 実行内容 |
|---|---|
| `Command::open(id, label, screen, params)` | 画面をウィンドウとして開きます |
| `Command::navigate(id, label, screen, params)` | 画面へ移動します |
| `Command::effect(id, label, effect)` | トーストなど、ブラウザ内のエフェクト |
| `Command::action(id, label, action, input)` | サーバー上のアクション。入力がブラウザに送られることはありません |

`App::check` は、読み取れないショートカット、2 つのコマンドが共有しているショートカット、パレットが自分用に確保しているショートカットを報告します。

## コマンドパレット

**Alt+Shift+P** は、既定でパレットを開きます（`App::palette_shortcut` で変更でき、意図的に `Ctrl+Shift+P` のようなブラウザのキーにすることもできます）。これは ARIA の combobox で、アプリのコマンド、現在の画面のツールバーとコンテキストメニュー、メニューの画面を検索します。最近使ったコマンドが先頭に来ます。

Feature は、これにサーバー検索を追加できます。ユーザーが入力するとパレットはアクションを呼び出し、その結果をコマンドの下に一覧表示します。1 つを選ぶと、そのページがその場で開きます。このサイトは、この方法で自身のページを検索しています。

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

サーバーは、ユーザーが実行できるすべての検索を、Feature のガード、アクションの権限、インターセプターとともに実行し、クエリを 100 文字で切り詰め、アプリの外へ出るヒットを破棄し、最大 20 件を返します。

## メニューバー、ツールボックス、ステータスバー

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar` は ARIA の menubar パターンに従い、パレットと同じコマンドを持ちます。
- `Toolbox` は、トグルボタンの縦のグループです。選んだツールは `zeyn:tool` イベントとして送られます。
- `PropertyInspector::new(title, form)` は、選択したオブジェクトを、`.section(...)` のグループに分けたフォームで編集します。
- `StatusBar` にはライブリージョンがあり、そこに送られたメッセージはスクリーンリーダーによって読み上げられます。

## ドッキングパネル

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

ユーザーは、タブをエリア間でドラッグし、ポインターやキーボードでエリアのサイズを変更し、パネルを閉じられます。メニューの `MenuList::panels` で元に戻せます。レイアウトは、アプリのビューストアにユーザーと画面ごとに保存されます。アプリにビューストアがない場合は、ブラウザに保存されます。
