# アプリと共有ライブラリを別々のリポジトリで管理する

Zeyn は 1 つのリポジトリでも動作しますが、多くのチームはアプリごとにリポジトリを分け、複数のアプリが使う共有コード (entity、サービス、Feature 全体) をライブラリにまとめています。このガイドでは、その構成方法と、構成を機能させ続けるための 1 つのルールを説明します。

## 何をどこに置くか

| リポジトリ | 作成コマンド | 内容 |
|---|---|---|
| アプリ | `zeyn new billing` | `main.rs`、`app.rs`、アプリの Feature、設定、アプリのカタログ |
| 共有ライブラリ | `zeyn new acme-domain --lib` | entity、サービス、Feature、そのカタログ、`CATALOGS` |

ライブラリの Feature はアプリの Feature と同じものです。model、action、Screen、パーミッション、翻訳が 1 つのフォルダーにまとまっています。ライブラリのどの Feature をマウントするかは、アプリが決めます。

```rust
let i18n = I18n::new("en")
    .catalogs(acme_domain::CATALOGS)?
    .catalog("en", include_str!("../locales/en.json"))?
    .catalog("tr", include_str!("../locales/tr.json"))?;

App::new(t!("app.name"))
    .i18n(i18n)
    .service(acme_domain::customers::CustomerStore::default())
    .feature(acme_domain::customers::feature())
```

ライブラリのカタログが先に読み込まれるため、アプリは同じキーを定義することでライブラリの文字列を上書きできます。2 つのライブラリが衝突しないように、ライブラリのキーには Feature 名を接頭辞として付けてください (`customers.list`)。

`examples/shared-domain` がそのようなライブラリです。`examples/billing` と `examples/crm` はどちらもその customers Feature をマウントしており、crm は自身のカタログで「Customers」を「Accounts」に名前変更しています。

## ライブラリが持つべきもの

Rust のオーファンルールでは、トレイトを実装できるのは、そのトレイトまたは型を定義したクレートの中だけです。アプリは、ライブラリの型に対して `zeyn::Model` や `zeyn::Entity` を実装できません。そのため、次のようにします。

- `#[entity]`、`#[model]`、`#[choice]` は、ライブラリ内の型に、そのルールの隣に付けます。
- 検証ルール (`#[rule(...)]`) は共有する型に付けたままにします。そうすれば、すべてのアプリが同じ方法で検証します。
- 追加の動作が必要なアプリは、その型をラップする (`struct Invoice { customer: Customer, ... }`) か、独自のトレイトを追加します。ライブラリの型を編集してはいけません。

## アプリがライブラリを取得する方法

| 方法 | 長所 | 注意点 |
|---|---|---|
| タグ付きの Git 依存 | 実行するものがなく、どの Git サーバーでも動く | Cargo がリポジトリ全体を取得する。すべてのアプリが URL を列挙する |
| プライベート Cargo レジストリ (Kellnr、GitLab package registry など) | 通常の `version = "0.3"` 依存、semver による解決 | 運用、バックアップ、保護が必要なサービスが増える |
| すべてを 1 つのワークスペースリポジトリにまとめる | 1 回のビルド、1 つの `Cargo.lock`、1 回のコミットでのリファクタリング | すべてのチームがすべてのアプリを見られる。アクセス制御はフォルダー単位ではなくリポジトリ単位 |

まずは Git タグから始めてください。ライブラリが数個を超えたとき、またはバージョンの手動解決が面倒になったときに、レジストリへ移行します。アプリのコードは変わらず、変わるのは `Cargo.toml` の行だけです。

```toml
[dependencies]
zeyn = { git = "https://git.example.com/platform/zeyn.git", tag = "v0.0.2" }
acme-domain = { git = "https://git.example.com/shared/acme-domain.git", tag = "v0.3.0" }
```

`zeyn new` は既定で `zeyn = "<version>"` を書き込みます。Zeyn が公開されるまでは、新しいアプリとライブラリのすべてで、この行を上記の Git の形式に変更してください。

## ルール: どこでも同じ Zeyn のバージョン

すべてのライブラリとすべてのアプリは、まったく同じ Zeyn に依存する必要があります。Git 依存の場合、それは同じ URL と同じタグを意味します。

Cargo は、2 つの異なる Zeyn のソースを 2 つの異なるクレートとして扱います。タグ `v0.0.2` に対してビルドされたライブラリが返す `zeyn::Feature` を、`v0.0.3` に対してビルドされたアプリは受け取れません。コンパイラーは、同じ名前を持つ 2 つのものの間で型の不一致を報告します。レジストリを使うと「同じ semver 互換バージョン」に緩和されますが、1 回のビルドにつき Zeyn は 1 つというルールは変わりません。

Zeyn をアップグレードする手順は次のとおりです。

1. 各ライブラリでタグを更新し、テストを実行し、新しいライブラリのタグをリリースします。
2. 各アプリで、Zeyn とライブラリのタグをまとめて 1 回のコミットで更新します。

`cargo tree -d -i zeyn` で重複を一覧表示できます。Zeyn が複数表示された場合は、どれかの依存がまだ古いタグを指しています。

## 手順

1. ライブラリを作成し、専用のリポジトリにプッシュします。

   ```sh
   zeyn new acme-domain --lib
   cd acme-domain
   cargo test
   git init && git add . && git commit -m "First version"
   git tag v0.1.0 && git push origin main --tags
   ```

2. タグを付けてアプリの `Cargo.toml` に追加し、上で示したようにカタログ、サービス、Feature をマウントします。

3. アプリの CI がライブラリを読み取れるようにします。生成された `.gitlab-ci.yml` は、`https://<gitlab host>/` と `ssh://git@<gitlab host>/` の URL をジョブトークンを使うように書き換えます。GitLab で、ライブラリのプロジェクトを開き、CI/CD 設定、Job token permissions の順に進んで、アプリのプロジェクトを許可リストに追加してください。

4. アプリのテストで `app.check()` を実行します (生成される `tests/app.rs` が実行しています)。マウントされた Feature に構造上の問題がある場合、たとえばある言語だけ翻訳キーが欠けている場合に失敗します。

## ライブラリとアプリを同時に開発する

`Cargo.toml` を編集せずに、アプリをローカルのチェックアウトに向けます。次の内容をアプリの `.cargo/config.toml` に書き、そのファイルは Git の管理対象から外してください。

```toml
[patch."https://git.example.com/shared/acme-domain.git"]
acme-domain = { path = "../acme-domain" }
```

変更が終わったら、この設定を削除し、ライブラリにタグを付け、アプリ側のタグを更新してください。

## ライブラリのリリース

1. `Cargo.toml` の `version` を更新します。
2. `cargo semver-checks` がインストールされていれば実行します。呼び出し側を壊す公開 API の変更を報告します。ライブラリが 1.0 未満の間は、破壊的変更には新しいマイナーバージョンが必要です。
3. コミットし、`v<version>` のタグを付け、タグをプッシュします。タグと `Cargo.toml` が一致しない場合、CI ジョブ `tag-matches-version` が失敗します。
