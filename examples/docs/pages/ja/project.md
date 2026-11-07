# ソース、ライセンス、セキュリティ

Zeyn はオープンソースです。このページでは、コードの場所、ライセンス、テストの方法、セキュリティ上の問題の報告方法を説明します。

## ソースコード

コードと issue は GitHub にあります: [github.com/huseyininnc/zeyn](https://github.com/huseyininnc/zeyn)。crate は [crates.io](https://crates.io/crates/zeyn) で公開され、API リファレンスは [docs.rs](https://docs.rs/zeyn) にあります。

## ライセンス

Zeyn は [MIT ライセンス](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-MIT)と [Apache License, Version 2.0](https://github.com/huseyininnc/zeyn/blob/main/LICENSE-APACHE) のデュアルライセンスで、どちらかを選べます（`Cargo.toml` では `MIT OR Apache-2.0`）。コントリビューションも同じ 2 つのライセンスで受け付けます。

## 品質

リポジトリには 767 個のテスト関数があります（crate とサンプルの `#[test]` と `#[tokio::test]`、2026-11-06 時点）。これに加えて、ドキュメントコメントのサンプルが doctest として実行されます。

すべての pull request と、`dev` と `main` へのすべての push で CI が動きます:

- 書式を確認する `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`（lint の警告が 1 つでもあればビルドは失敗します）
- すべての crate とサンプルのテストを実行する `cargo test --workspace`
- `assistant`、`oidc`、`totp`、`islands`、`realtime`、`xlsx`、`email`、`accounts` の機能を有効にした `cargo test -p zeyn`
- サーバー機能なしでフレームワークもビルドする `cargo test -p zeyn-cli`
- ライセンス、セキュリティアドバイザリ、禁止された依存関係を確認する `cargo deny`

各実行の結果は [Actions ページ](https://github.com/huseyininnc/zeyn/actions)で公開されています。

## セキュリティ

- **サポート対象のバージョン。** Zeyn は alpha 段階です。セキュリティ修正を受けるのは最新の `0.0.x` リリースだけです。
- **報告方法。** 脆弱性は GitHub Security Advisories で非公開に報告してください。リポジトリの Security タブを開き、"Report a vulnerability" を選びます（[直接リンク](https://github.com/huseyininnc/zeyn/security/advisories/new)）。公開の issue、ディスカッション、pull request では報告しないでください。
- **対応。** 報告には 5 営業日以内に受領の連絡をします。修正のリリース後にセキュリティアドバイザリを公開し、希望しない場合を除いて報告者の名前を記載します。

報告に含める内容と対象範囲を含むポリシーの全文は [SECURITY.md](https://github.com/huseyininnc/zeyn/blob/main/SECURITY.md) にあります。

## その他

- [変更履歴](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md): 各リリースの変更点。
- [ロードマップ](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md): 次のリリースと予定。
- [コントリビューションガイド](https://github.com/huseyininnc/zeyn/blob/main/CONTRIBUTING.md): ビルド、テスト、変更の送り方。
- [行動規範](https://github.com/huseyininnc/zeyn/blob/main/CODE_OF_CONDUCT.md): 一緒に働くためのルール。
- [サポート](https://github.com/huseyininnc/zeyn/blob/main/SUPPORT.md): 質問やバグ報告の窓口。
