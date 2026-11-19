# CLI とプロジェクト

`zeyn` コマンドは、プロジェクトの作成、Feature の追加、アプリの検査を行い、エージェント向けの MCP サーバーを起動します。

## コマンド

| コマンド | 内容 |
|---|---|
| `zeyn new <name>` | 標準の Feature 構成でアプリを作成します |
| `zeyn new <name> --lib` | 複数のアプリで共有するモデル、Feature、テキストのライブラリを作成します |
| `zeyn new <name> --component` | ブラウザスクリプト、スタイルシート、テキスト、サンプルアプリ、テストを備えたコンポーネントクレート `zeyn-<name>` を作成する |
| `zeyn g feature <name>` | `models`、`services`、`actions`、`screens` フォルダーと動作する一覧画面を備えた `src/features/<name>` を追加します |
| `zeyn g screen <feature> <name>` | feature に `screens/<name>.rs` を追加して登録し、タイトルをすべてのロケールファイルに追加します |
| `zeyn routes` | Feature、画面、ルート、API、アクセス権を一覧表示します |
| `zeyn check` | コンパイルし、構造上の問題と翻訳の欠落を報告します |
| `zeyn check --fix` | `src`、`tests`、`examples` 内で名前が変わった Zeyn API を書き換えてから検査します |
| `zeyn check --json` | 安定したコード、理由、修正方法つきでレポートを JSON で出力します |
| `zeyn doctor` | プロジェクトのビルド方法を読み取り、遅さや大きさの原因を、各項目に1行の修正方法を添えて報告します |
| `zeyn mcp` | AI エージェント向けに標準入出力で MCP サーバーを起動します |

crates.io からインストールします。

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

このリポジトリ内では、Cargo 経由で CLI を実行します。

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## 新しいアプリ

`zeyn new` は、ビルドでき、テストに通り、エージェントに与えられたルールに従うプロジェクトを書き出します。

- `AGENTS.md` と `CLAUDE.md`: プロジェクトのルールと、エージェント向けのフレームワーク索引。
- `.mcp.json`: `zeyn` MCP サーバーを登録します。これによりエージェントは `zeyn_check` と `zeyn_docs` を呼び出せます。
- `config/app.json`: デフォルト値。環境変数 `APP_*` が上書きします。たとえば `server.addr` には `APP_SERVER__ADDR` を使います。
- `locales/en.json` と `locales/tr.json`: ユーザーに見えるすべてのテキスト。
- `src/features/`: Feature ごとに 1 つのフォルダ。サンプルの `notes` Feature が入っています。
- `rustfmt.toml`、`clippy.toml`、`deny.toml`、`rust-toolchain.toml`、および `Cargo.toml` の `[lints]`: CI が強制する標準設定。
  `rustfmt.toml` は `chain_width = 40` を設定しているため、コードは上から下へ読めます。それより長いメソッドチェーンは 1 行に 1 つの呼び出しとなり、平均すると 1 行にドットが 1 つか 2 つになります。エージェントも人も、長い 1 行よりリストのほうが速くレビューでき、差分も変更された 1 つの呼び出しだけを示します。この設定は維持してください。

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  次のような書き方ではありません: `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`。
- `build.rs` と、`Cargo.toml` の `[package.metadata.zeyn]` にある `max_file_lines = 300`: `src/` のファイルの空でない行がそれより多いと `cargo build` が止まります。clippy の `too_many_lines`（60、`clippy.toml`）は関数について同じことをします。[Project structure](structure.md) を参照してください。
- `Cargo.toml` の `[profile.dev]`、`.cargo/config.toml`、`.vscode/settings.json`: 速いビルドの繰り返しのための設定です。依存関係は `opt-level = 1` でデバッグ情報なしにビルドされ、自分のコードは行テーブルだけを持ち（バックトレースにはファイルと行が表示されます。ローカル変数でデバッグするには `CARGO_PROFILE_DEV_DEBUG=full` を設定します）、Windows では `rust-lld` でリンクし、rust-analyzer は専用の target フォルダを使うので `cargo build` を待ちません。
- `Dockerfile` と `.dockerignore`: 小さなコンテナイメージ。
- 代表的な CI サーバー向けのパイプライン。いずれも同じ検査（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`、`cargo deny check`）を実行し、`Cargo.toml` と一致しない `vX.Y.Z` タグを拒否します。
  - `.github/workflows/ci.yml`: GitHub Actions 用。`.github/dependabot.yml`（Cargo とアクションの週次更新）とプルリクエストのチェックリストが付きます。
  - `.gitlab-ci.yml`: GitLab 用。
  - `Jenkinsfile`: Jenkins 用（Docker エージェント、`cargo llvm-cov` によるカバレッジ）。
- `sonar-project.properties`: SonarQube 解析（Clippy の指摘と、`target/lcov.info` の LCOV カバレッジ）。GitHub のワークフローは、リポジトリ変数 `SONAR_HOST_URL` とシークレット `SONAR_TOKEN` が設定されていれば実行し、Jenkins はジョブが `SONAR_ENABLED=true` を設定していれば実行します。
- `SECURITY.md` と `CHANGELOG.md`（Keep a Changelog 形式）。
- git がインストールされていて（Windows、macOS、Linux）、フォルダがすでにリポジトリ内にない場合は、ブランチ `main` のローカル git リポジトリ。何もコミットされないため、最初のコミットにはご自身の名前とメールアドレスが入ります。`--no-git` を付けると作成しません。

各フォルダについては [プロジェクト構造](structure.md) を参照してください。

## 新しいコンポーネントクレート

`zeyn new <name> --component` はクレート `zeyn-<name>` を書き出します(名前の先頭の `zeyn-` は受け付けます。名前は小文字、数字、単独のハイフンで、最大32文字)。[共有ライブラリ](polyrepo.md)の拡張モデルをそのまま変更できる形にしたものです。

- `src/lib.rs`: サンプルのコンポーネント(トグルボタン)、`App::module` でブラウザモジュールを登録するその `Plugin`、定数 `CATALOGS`。
- `assets/<name>.js` と `assets/<name>.css`: モジュール。クラスは `z-<name>-` で、マーカー属性は `data-zeyn-<name>-` で始まります。スタイルシートはテーマトークンだけを使うので、ライトとダークの両方のテーマで動きます。
- `assets/en.json` と `assets/tr.json`: テキスト。キーは `<name>` の下にあります。
- `examples/demo`: コンポーネントのないスタートページと、コンポーネントのあるページを持つアプリ。`cargo run --example demo` で実行します。
- `tests/<name>.rs`: コンポーネントのあるページはモジュールをリンクし、ないページは決して参照しません。ランタイムは後から届くコンテンツ用にモジュールを一覧し、ファイルはURLにビルドを含めて配信され、アプリはテキストの文言を変更できます。
- ライブラリと同じプロジェクトファイル: GitHub、GitLab、Jenkins 用の CI、lint とフォーマットの規則、`AGENTS.md` と `CLAUDE.md`。

```sh
zeyn new rating --component
cd zeyn-rating
cargo test
```

## ジェネレーター

`zeyn g feature` は、モデル、サービス、アクション、画面ごとに 1 ファイルの feature を作ります。

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

短い形 `zeyn g f` と `zeyn g s` も同じ動作をします。新しい feature はすぐに動きます。モデル、メモリ上のデータサービス、検索アクション、メニュー項目付きの一覧画面が入っています。必要のないフォルダーは feature 側で削除します（フォルダーと、feature の `mod.rs` にあるその `mod` 行）。`components/` は生成されません。画面の一部を共有するときや画面ファイルが長くなったときに手で追加し、1 ファイルに `Node` を返す関数を 1 つ置きます（`components/<name>.rs`）。画面には権限 `<feature>.<name>.view` が付き、タイトルが `locales/` のすべてのファイルに入ります。英語以外の言語には英語の文と翻訳を促すメモが入ります。このレイアウトは強制ではありません。feature が 1 つの `screens.rs` を使っている場合、`zeyn g screen` はまずその内容ごと `screens/mod.rs` へ移し、`screens/` が削除されていれば作り直します。ジェネレーターはファイルを上書きせず、snake case でない名前は受け付けません。

## アプリの検査

`zeyn check` はコンパイラを実行し、続いてアプリ自身の検査（`App::check`）を行います。対象は、重複したルート、二重に登録された画面、画面が必要としているのに誰も登録していないサービス、使えないショートカット、WCAG AA のコントラストを下回るテーマトークン、ある言語で欠けている翻訳キーです。問題があればエラーで終了するため、CI で実行できます。

検査を失敗させない警告も出します。どの feature も登録していない `#[screen]` や `#[action]`、そして複数の項目を持ち約 150 行を超えた 1 つの `screens.rs` や `actions.rs` です。

Zeyn のリリースで API の名前が変わったときは、`zeyn check --fix` が、たとえば `Grid` から `DataGrid`、`GridQuery` から `DataQuery` のように古い名前を書き換え、もう一度検査します。

```sh
zeyn check --fix
```

どの問題にも安定したコードがあります。アプリ、フィーチャー、ルート、サービスは `Z01xx`、セキュリティは `Z02xx`、公開ページは `Z03xx`、メニュー、コマンド、ショートカットは `Z04xx`、データは `Z05xx`、ページ検査は `Z06xx`、警告は `W01xx` です。テキスト出力は各問題を理由 (`why`)、修正方法 (`help`)、このドキュメントへのリンク、コードとともに表示します。`zeyn check --json` は同じレポートをエージェントと CI 向けに 1 つの JSON ドキュメントとして出力し、`ok` が false のときはエラーで終了します。

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

`zeyn check` はアプリのソースも読み、ファイルと行を添えて次を警告します。`W0106` は `src` のない `<script>` や `onclick=` のようなインラインのイベントハンドラーを含む文字列 (CSP はどちらもブロックします)、`W0107` は `escape(..)`、`r.text(..)`、`r.value(..)` を使わずに HTML を書く手書きのコンポーネント、`W0108` は範囲を限るクエリなしで `Vec` 全体を返す action、`W0109` は `.trusted()` を付けた `DocumentViewer` です。`permission` や `public` がない場合は警告するまでもなく、コンパイルできません。

## ビルド速度とディスク使用量

`zeyn doctor` はプロジェクトのビルド方法を読み取り、遅さや大きさの原因を報告します。target フォルダのサイズと最大のフォルダ、リンカー、dev プロファイル（デバッグ情報と依存関係の最適化）、`rust-version` と比べた rustc と cargo、古いツールチェーンが残した成果物、rust-analyzer 用の target フォルダ、そして Windows では target フォルダと `~/.cargo` が Defender のスキャン対象外かどうか（除外設定または Dev Drive。Windows が読み取りを許可しない場合はその旨を伝えます）を調べます。各項目には安定したコードと1行の修正方法があります。`--json` はそれらを1つのドキュメントとして出力し、`--target-limit-gb` は大きいとみなすサイズを決め（既定は 20）、終了コードは常に 0 です。`zeyn doctor --fix` は高速な dev プロファイルを `Cargo.toml` に、`rust-lld` リンカーを `.cargo/config.toml` に追加し、ほかは何も変更しません。2回目の実行では何も変わりません。MCP ツール `zeyn_doctor` はエージェント向けに同じことを行います。

## DevTools

`cargo run`（または `zeyn dev`）で動かすデバッグビルドは、各リクエストが何をしたかを記録します。実行した画面とアクション、ステータスと時間、画面の構築と描画にかかった時間、各 SQL 文とその時間および影響を受けた行数、アクションが返したエフェクトとトピックです。Alt+Shift+D で直近 50 件のリクエストを表示するパネルが開き、各リクエストを開くとその SQL 文が表示されます。`GET /_zeyn/devtools` は同じ一覧を JSON で返し、MCP ツール `zeyn_devtools` がそれをエージェントのために読みます。

SQL 文は引数なしで保存されるため、値がパネルに届くことはありません。リリースビルドには DevTools がなく、そのためのコードもルートもメモリもありません。`App::router` を自分で提供するアプリやテストは、`zeyn::server::devtools(true)` で有効にします。

## Docker

生成される `Dockerfile` は 2 段階でビルドし、distroless イメージ上で root 権限のないユーザーとして動作します。バイナリは `<app> health` で自身の稼働状態を確認でき、`HEALTHCHECK` がこれを使います。

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

生成されたアプリのイメージは約 43 MB で、アイドル状態のアプリは数 MB の RAM しか使いません。

## MCP サーバー

`zeyn mcp` は標準入出力で MCP を話します。ツールは `zeyn_docs`（トピック別のフレームワークリファレンス）、`zeyn_describe`（カレントディレクトリにあるアプリの Feature、画面、ルート、問題点）、`zeyn_inspect`（画面のノードツリーと、各ノードを置いた Rust の行）、`zeyn_devtools`（実行中のデバッグビルドの DevTools リクエストログ: 画面、アクション、時間、SQL 文）、`zeyn_check`（`fix: true` を付けると `check --fix` を実行）、`zeyn_doctor`, `zeyn_test`、`zeyn_new_feature`、`zeyn_new_screen` です。[MCP と AI エージェント](mcp.md) を参照してください。
