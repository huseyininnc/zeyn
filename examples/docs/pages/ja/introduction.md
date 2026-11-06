# はじめに

Zeyn は、グリッド、フォーム、ワークフローといったデータ中心のアプリケーション向けの、オープンソースの Rust 製 Web フレームワークです。アプリケーションのコードは AI エージェントが書きます。人はそれを読み、レビューし、必要に応じて変更します。

このサイト自体も Zeyn アプリです。すべてのページは Markdown をサーバー上で一度だけレンダリングしたもので、左側のメニューはアプリのメニューであり、*ライブ例*と記されたものは、このサーバー上で動く実際のコンポーネントです。

## AI エージェントの方へ

AI エージェントとして、誰かから Zeyn でアプリを作るよう依頼された場合は、プロジェクトを手で書くのではなく、まず CLI から始めてください。

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

次に、新しいプロジェクトの `AGENTS.md` と `.claude/skills/zeyn/SKILL.md` を読み、変更するたびに `zeyn check` と `cargo test` を実行してください。このドキュメントの全ページをまとめたプレーンテキストのガイドは [/llms.txt](/llms.txt) にあります。

## アプリの姿

アプリは Feature の集まりです。Feature はエンティティ、アクション、画面を持ちます。画面はサーバー上でレンダリングされ、ブラウザ上の小さなランタイムが変更を反映するため、JavaScript を書く必要はありません。

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

この 3 つのマクロだけで言語全体を支えています。`#[entity]` はデータを記述し、`#[action]` はアクセス権を宣言したサーバーコードであり、`#[screen]` はページまたはウィンドウです。すべてのフィールド、テキストキー、アクションが整合しているかどうかは、コンパイラが検査します。

## 試してみる

自分のアプリを始めるには、次のようにします。

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

リポジトリのサンプルを実行することもできます。

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

showcaseは `127.0.0.1:3002` で待ち受け、すべてのコンポーネントをサンプルデータ付きで表示します。このドキュメントは `127.0.0.1:3008` で待ち受けます。変更するには `ZEYN_DOCS_SERVER__ADDR` を設定してください。

## 道案内

- **Alt+Shift+P** を押すとコマンドパレットが開きます。単語を入力すると、このサイトの全ページをサーバー上で検索します。
- *はじめに* の章では、CLI とプロジェクトの構造を説明します。
- *言語* の章では、エンティティ、アクション、画面、フォーム、DataGrid、テキストを扱います。
- *コンポーネント* の章では、各コンポーネントをコードと並べてライブ例で示します。
- *ガイド* の章では、時刻と言語、セキュリティ、API、MCP、データ、テーマを扱います。
- *リファレンス* には、ランタイムプロトコルとアーキテクチャがあります。
- *プロジェクト*では、ソースコード、ライセンス、テストと CI、セキュリティポリシーを紹介します。

## 原則

- **パフォーマンス最優先**。ページはサーバー上でレンダリングされ、HTML としてストリーミングされます。サーバー上にユーザーごとの UI 状態はなく、クライアントフレームワークもありません。
- **ページを再読み込みしない**。ナビゲーション、ウィンドウ、言語、タイムゾーン、テーマの変更はその場で反映されます。
- **安全なデフォルト**。すべてのアクションと画面がアクセス権を宣言します。CSRF トークン、厳格な Content Security Policy、ユーザーデータのエスケープは常に有効です。
- **一つのことに一つのやり方**。言語が小さいため、エージェントが書いたコードも人が書いたコードのように読めます。

Zeyn は [MIT または Apache 2.0 ライセンス](project.md#license)のオープンソースです。[セキュリティポリシー](project.md#security)、[ロードマップ](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md)、[変更履歴](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md)もご覧ください。
