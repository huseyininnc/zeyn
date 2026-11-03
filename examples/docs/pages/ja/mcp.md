# MCP と AI エージェント

Zeyn は 2 つの側面のエージェントを想定して作られています。Zeyn のアプリを書くエージェントと、ユーザーに代わって動作中のアプリを使うエージェントです。

## アプリを書くエージェント

エージェントが作業するプロジェクトには、独自のルールとツールが付属します。

- `AGENTS.md` と `CLAUDE.md` には、コマンド、プロジェクト構造、言語のルール、フレームワークの索引が載っています。
- `.mcp.json` は、標準入出力の MCP サーバーである `zeyn mcp` を登録します。ツールは次のとおりです。

| ツール | 内容 |
|---|---|
| `zeyn_docs` | トピック別のフレームワークリファレンス: `rules`、`language`、`protocol` |
| `zeyn_describe` | アプリの Feature、画面、ルート、アクセス権、API、問題点、欠けている翻訳 |
| `zeyn_check` | コンパイルして問題を報告します。`fix: true` を付けると、先に名前が変わった API を書き換えます |
| `zeyn_test` | `cargo test` を実行し、出力の末尾を返します |
| `zeyn_new_feature` | Feature のフォルダを作成して登録します |

主な防御線はコンパイラです。コンパイラが強制できるすべてのルールには `compile_fail` テストと、エージェントが対処できるように書かれた `#[diagnostic::on_unimplemented]` のメッセージがあります。たとえば次のようなものです。

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run` は、`zeyn_describe` が返すのと同じ記述を出力して終了します。

## アプリを使うエージェント

```rust
App::new(t!("app.name")).mcp(Mcp::read_only())
```

`agents` を指定すると、アプリの画面がツールになります。

- グリッドごとに 1 つの検索ツール。
- フォームごとに 1 つの保存ツール。フォームのフィールドとルールがそのスキーマになります（`Mcp::read_write()` の場合のみ）。

エージェントは 2 つの方法でこれらに到達します。

- **ブラウザ内**では、ブラウザが対応していれば WebMCP（`document.modelContext.registerTool`）を通します。現在の画面のツールは、画面が開くときに登録され、閉じるときに削除されます。
- **サーバー上**では、`POST /_zeyn/mcp`（JSON-RPC）です。`Authorization` ヘッダーが必要なので、ブラウザのセッションだけでこれを操作することはできません。

## 人と同じルール

- エージェントは、代理するユーザーとまったく同じ権限を持ちます。ユーザーが開けない画面やアクションのツールは一覧に出ません。
- 呼び出しでは、通常の呼び出しと同じガード、インターセプター、検証、レート制限が実行されます。
- 監査レコードはエージェントによる呼び出しを区別して記録するため、レビュー担当者は両者を見分けられます。
- `Mcp::read_only()` では、エージェントは検索できますが保存はできません。エージェントがデータを変更してよいアプリでだけ `read_write()` を選んでください。
