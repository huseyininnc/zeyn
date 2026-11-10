# デスクトップアプリ

Zeyn アプリは、1 つのデスクトップ実行ファイルとして配布できます。`zeyn-desktop` クレートは、アプリ自身のサーバーをプロセス内で起動し、システムの WebView を通じてネイティブ window に表示します。

| プラットフォーム | WebView | 状況 |
|----------|---------|--------|
| Windows 10 と 11 | WebView2 (Edge) | ビルドし、手動で確認済み |
| macOS | WKWebView | `wry` 経由でコンパイルできる。手動では未確認 |
| Linux | WebKitGTK 4.1 | `libwebkit2gtk-4.1-dev` と `libgtk-3-dev` がインストールされていれば `wry` 経由でコンパイルできる。手動では未確認 |

データがマシンの外に出ることはありません。アプリ自身がリモートサービスを呼び出さない限り、オフラインで動作します。

## 使い方

通常のサーバービルドで WebView がコンパイルされないよう、クレートを feature の背後に追加します。

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.10", optional = true }
```

`App::serve` の代わりに window を起動します。

```rust
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    zeyn_desktop::Desktop::new(app()?)
        .id("my-app")
        .title("My App")
        .size(1280, 800)
        .run()
}
```

- `id` は、ユーザーごとのデータフォルダーの名前になり、id ごとに 1 つのインスタンスを保ちます。既定値は実行ファイル名です。
- `title` は、ページが `document.title` を設定するまで表示されます。その後、window のタイトルはページに追従します。
- `size` は、論理ピクセルでの最初のサイズです。2 回目以降の起動では、前回のサイズと位置を再利用します。

`run` は window が閉じるまでブロックし、サーバーを停止してプロセスを終了します。`ZEYN_DESCRIBE=1` を指定すると、アプリの説明を出力して戻るため、`zeyn check` と `zeyn routes` は引き続き動作します。

Windows のリリースビルドでコンソール window を非表示にするには、`main.rs` の先頭に次を置きます。

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` は、POS（販売時点管理）アプリの完全な例です。

```
cargo run -p zeyn-example-adisyon --features desktop
```

## 実行ファイルをビルドする

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

リンク時最適化、1 つの codegen unit、シンボルの削除を有効にして `cargo build --release` を実行し、各実行ファイルのパスとサイズを表示します。速度は `opt-level = 3` のままです。

### アイコン、バージョン情報、マニフェスト (Windows)

Windows は、これらを exe 内部のリソースから読み取ります。デスクトップビルドの場合にだけ埋め込む build スクリプトを追加してください。

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`:

```rust
fn main() {
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "My App")
            .set("FileDescription", "My App")
            .set_manifest_file("assets/app.exe.manifest");
        res.compile().unwrap();
    }
}
```

アイコンのリソース id は 1 である必要があります（`set_icon` の既定値）。window とタスクバーがこれを使用します。バージョンは `package.version` から取得されます。モニターごとの DPI 対応、UTF-8、common controls 6 を指定したマニフェストについては、`examples/adisyon/build.rs` と `examples/adisyon/assets` を参照してください。MSVC ツールチェーンは Windows SDK から `rc.exe` を見つけます。GNU ツールチェーンは `windres` を使用します。

## WebView2 ランタイム

Windows 11 と最新の Windows 10 には WebView2 ランタイムがプリインストールされており、自動的に更新されます（evergreen ランタイム）。exe はこれを同梱しないため、サイズが小さく保たれます。

古いマシンや制限の厳しいマシンの場合は、次のようにします。

- Microsoft の evergreen ブートストラッパー（`MicrosoftEdgeWebview2Setup.exe`、約 2 MB）をインストーラーに同梱し、ランタイムが存在しない場合に `/silent /install` で実行します。ランタイムは一度だけダウンロードされます。
- インターネットに接続できないマシンには、evergreen スタンドアロンインストーラー（大きい、オフライン用）か、exe の隣に置く固定バージョンランタイムを使用します。固定バージョンは自動更新されないため、セキュリティ更新は自分で管理することになります。

ランタイムがない場合は window を開けず、`run` はエラーを返します。

## セキュリティモデル

サーバーには、アプリ自身の window だけが到達できます。

- **ループバックのみ。** リスナーは、システムが選んだポートで `127.0.0.1` にバインドします。`0.0.0.0` や LAN アドレスにバインドすることはありません。
- **起動トークン。** 起動のたびに、ランダムな 256 ビットのトークンが作られます。window は最初に `/_desktop/start?t=<token>` を開きます。これはトークンを `HttpOnly`、`SameSite=Strict` の cookie として設定し、`/` にリダイレクトします。それ以外のすべてのリクエストは、その cookie か `x-zeyn-desktop` ヘッダーにトークンを持っている必要があり、持っていなければ `403` になります。静的アセットも保護されます。トークンはメモリ内にだけ存在し、定数時間で比較されます。
- **Host と Origin。** `Host` ヘッダーは、ちょうど `127.0.0.1:<port>` でなければなりません。これにより DNS rebinding を防ぎます（名前が 127.0.0.1 に解決されるサイトは、ホストとして自分自身の名前を送ります）。`Origin` ヘッダーは、存在する場合、ちょうど `http://127.0.0.1:<port>` でなければなりません。どちらも、start と focus エンドポイントを含むすべてのパスに適用されます。
- **他のローカルプログラム**はポートに接続できますが、トークンを知りません。他のユーザーアカウントは、ユーザーごとのデータフォルダーにあるインスタンスファイルを読めません。
- **ナビゲーション。** window が表示するのは、自身の origin、その `blob:` URL、`about:blank` だけです。`http`、`https`、`mailto` のリンクはシステムの既定のハンドラーで開かれます。それ以外のスキーム（`file:`、カスタムプロトコルハンドラー、`javascript:`）は破棄されます。`window.open` が 2 つ目の WebView を作ることはありません。
- **Devtools** は、デバッグビルドでは有効、リリースビルドでは無効です。
- **ダウンロード**は、アプリの origin からのものだけが許可され、ユーザーの Downloads フォルダーに保存されます。既存のファイルを上書きすることはありません（`orders.csv`、`orders (1).csv`、...）。連続した複数のダウンロードは、確認なしで許可されます。その他の permission リクエスト（カメラ、マイク、クリップボード）には、WebView の通常のプロンプトが表示されます。

この保護が対象としないもの:

- 同じユーザーとして実行されているプログラムは、プロセスのメモリを読んだり、デバッガーをアタッチしたりできます。ローカルアプリがこれに対して防御することはできません。
- WebView2 は環境変数 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` を尊重するため、ユーザーの環境変数を設定できる人は誰でもリモートデバッグを有効にできます。同じユーザーであり、上と同じ限界です。
- アプリ自身の認証は、window 内でも引き続き適用されます。デスクトップビルドの `DevAuth` は、そのマシンの前にいる人に完全なアクセスを与えます。

## 動作

- **単一インスタンス。** 最初の起動は、`instance`（ポートと、別個の focus キー）をデータフォルダーに書き込みます。2 回目の起動は、そのキーを `/_desktop/focus` に POST します。最初の window が前面に出て、2 つ目のプロセスは約 150 ms 以内に終了します。2 つの起動が同じ瞬間に始まった場合は、両方が開くことがあります。
- **window の状態。** サイズ、位置、最大化フラグは、閉じるときに `window.json` に保存されます。保存された位置がどのモニターにも存在しなくなっている場合は無視されます。
- **シャットダウン。** window を閉じると、WebView が破棄され、サーバーが正常に停止し（最大 2 秒）、プロセスが終了します。WebView のプロセスもそれと一緒に終了します。
- **データフォルダー。** Windows では `%LOCALAPPDATA%\<id>`、macOS では `~/Library/Application Support/<id>`、Linux では `~/.local/share/<id>` です。WebView のプロファイル（キャッシュ、local storage）はその中の `webview` にあるため、exe は Program Files のような読み取り専用フォルダーからも動作します。
- **トレイアイコンなし。** 組み込まれていません。

## サイズとメモリ

Windows 11 で、`examples/adisyon` のリリースビルド、WebView2 ランタイム 156 を使って測定しました。

| | |
|---|---|
| `zeyn build-desktop` の exe | 6.72 MB（通常の release プロファイルでは 7.25 MB） |
| アプリプロセス、アイドル時 | プライベート 4.3 MB、ワーキングセット 25.6 MB |
| アプリプロセスと WebView2 プロセス、アイドル時 | 7 プロセス、プライベート約 180 MB、ワーキングセット 370 MB |
| `--disable-gpu` を付けた場合 | プライベート約 114 MB、ワーキングセット 330 MB |
| 2 回目の起動から終了まで | 約 130 ms |

WebView が大部分を占めます。その GPU プロセスだけで約 88 MB を保持します。メモリの少ないマシンでの単純な画面では、`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` を付けて exe を起動すると（たとえばショートカットやランチャースクリプトから）、ソフトウェアレンダリングになる代わりに約 65 MB を節約できます。
