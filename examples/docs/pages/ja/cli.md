# CLI とプロジェクト

`zeyn` コマンドは、プロジェクトの作成、Feature の追加、アプリの検査を行い、エージェント向けの MCP サーバーを起動します。

## コマンド

| コマンド | 内容 |
|---|---|
| `zeyn new <name>` | 標準の Feature 構成でアプリを作成します |
| `zeyn new <name> --lib` | 複数のアプリで共有するモデル、Feature、テキストのライブラリを作成します |
| `zeyn g feature <name>` | `models`、`services`、`actions`、`screens` フォルダーと動作する一覧画面を備えた `src/features/<name>` を追加します |
| `zeyn g screen <feature> <name>` | feature に `screens/<name>.rs` を追加して登録し、タイトルをすべてのロケールファイルに追加します |
| `zeyn routes` | Feature、画面、ルート、API、アクセス権を一覧表示します |
| `zeyn check` | コンパイルし、構造上の問題と翻訳の欠落を報告します |
| `zeyn check --fix` | `src`、`tests`、`examples` 内で名前が変わった Zeyn API を書き換えてから検査します |
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
- `Dockerfile` と `.dockerignore`: 小さなコンテナイメージ。
- 代表的な CI サーバー向けのパイプライン。いずれも同じ検査（`cargo fmt --check`、`cargo clippy -D warnings`、`cargo test`、`cargo deny check`）を実行し、`Cargo.toml` と一致しない `vX.Y.Z` タグを拒否します。
  - `.github/workflows/ci.yml`: GitHub Actions 用。`.github/dependabot.yml`（Cargo とアクションの週次更新）とプルリクエストのチェックリストが付きます。
  - `.gitlab-ci.yml`: GitLab 用。
  - `Jenkinsfile`: Jenkins 用（Docker エージェント、`cargo llvm-cov` によるカバレッジ）。
- `sonar-project.properties`: SonarQube 解析（Clippy の指摘と、`target/lcov.info` の LCOV カバレッジ）。GitHub のワークフローは、リポジトリ変数 `SONAR_HOST_URL` とシークレット `SONAR_TOKEN` が設定されていれば実行し、Jenkins はジョブが `SONAR_ENABLED=true` を設定していれば実行します。
- `SECURITY.md` と `CHANGELOG.md`（Keep a Changelog 形式）。
- git がインストールされていて（Windows、macOS、Linux）、フォルダがすでにリポジトリ内にない場合は、ブランチ `main` のローカル git リポジトリ。何もコミットされないため、最初のコミットにはご自身の名前とメールアドレスが入ります。`--no-git` を付けると作成しません。

各フォルダについては [プロジェクト構造](structure.md) を参照してください。

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

## Docker

生成される `Dockerfile` は 2 段階でビルドし、distroless イメージ上で root 権限のないユーザーとして動作します。バイナリは `<app> health` で自身の稼働状態を確認でき、`HEALTHCHECK` がこれを使います。

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

生成されたアプリのイメージは約 43 MB で、アイドル状態のアプリは数 MB の RAM しか使いません。

## MCP サーバー

`zeyn mcp` は標準入出力で MCP を話します。ツールは `zeyn_docs`（トピック別のフレームワークリファレンス）、`zeyn_describe`（カレントディレクトリにあるアプリの Feature、画面、ルート、問題点）、`zeyn_check`（`fix: true` を付けると `check --fix` を実行）、`zeyn_test`、`zeyn_new_feature` です。[MCP と AI エージェント](mcp.md) を参照してください。
