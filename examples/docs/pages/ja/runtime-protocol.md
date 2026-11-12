# Runtime Protocol

ステータス: ドラフト。`zeyn::server` と `assets/zeyn/*.js`（`/_zeyn/zeyn.js` として配信）が実装しています。

これは Zeyn サーバーとブラウザランタイムの間の契約です。Screen はサーバー側でレンダリングされます。ブラウザランタイムは小さなスクリプトで、ユーザー操作をサーバーへ送り、返ってきた結果を適用します。アプリケーションコードが JavaScript を書くことはありません。

## 1. Page load

`GET {route}?params={json}` は完全な HTML ドキュメントを返します。`params` は Screen params の URL エンコードされた JSON で、Screen に params がない場合は省略されます。

`/orders/{id}` のようにパスセグメントを持つルートでは、params のうちそのフィールドをパス（`/orders/42`）から、それ以外のフィールドを `params` から読み取ります。解釈できないセグメント値は 404 を返します。値はドットも含めてパーセントエンコードされる（`%2E`）ため、Screen の URL がランタイムにファイルとして扱われることはありません。`Screen::moved_to` を返す Screen には、新しい URL を `Location` に入れた `301 Moved Permanently` が返り、`Screen::redirect` を返す Screen には `Cache-Control: no-store` 付きの `303 See Other` が返ります。`nav.js` はどちらもページを読み込まずにたどり、読み込みが履歴エントリを追加しない場合（戻ると進む）も、遷移先のアドレスを表示します。

失敗したページは、そのステータス（404、403、500 など）と、アプリのエラー Screen（`App::not_found`、`App::server_error`）または組み込みの短いメッセージをシェル内に表示して返します。`x-zeyn-partial: 1` を付けたリクエストには、その Screen だけが `x-zeyn-partial: 1` と `x-zeyn-title` を付けて返るため、ランタイムはページを読み込み直さずに差し替えられます。エラーの詳細がブラウザに届くことはありません。

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

メニューには、現在のユーザーが開ける Screen だけが並びます。現在の項目には `aria-current="page"` が付きます。

head にあるフレームワークのスクリプトとスタイルシートにはすべて、`?v=` とビルド ID（`meta[name=zeyn-build]`。`ZEYN_BUILD_ID` か実行ファイルのハッシュ）が付き、`theme.css` にはテーマ文字列のハッシュが付きます。現在の値を持つリクエストは `Cache-Control: public, max-age=31536000, immutable` を受け取るため、ウォームなページ読み込みではそれらを 1 つも要求しません。それ以外のリクエスト（デプロイ後の古いタブ、`v` なし）は `no-cache` と ETag を受け取ります。ランタイムがあとからインポートするモジュールは、それをインポートするスクリプトの URL から同じ `v` を引き継ぎます。

### 1.1 Browser modules

ページは、その画面が使うブラウザーコードだけを読み込んで実行します。どのページもコアを実行します。`zeyn.js`（リクエスト、エフェクト、ウィンドウ、フォーム、メニュー、ショートカット、イベント委譲、モジュールローダー）、`nav.js`、`enterprise.js`（`fetch` のレスポンスフックとセッションのアイドルタイムアウト）で、合わせて非圧縮 90 KB、gzip で 29 KB です。それ以外はすべてコンポーネントのモジュールで、`/_zeyn/{name}.js` と一部は `/_zeyn/{name}.css` です。`crates/zeyn/src/server/modules.rs` の `MODULES` に、その HTML が持つマーカー（属性名 `data-zeyn-date` とクラス `z-select`、名前全体で一致）とともに宣言されます。

| モジュール | コンポーネント |
|---|---|
| `components.js` | すべてのコントロールモジュールが import する共有部分：ヘルパー、ポップアップとリストボックスのコア、セレクト（アプリシェルの選択肢）、確認ダイアログ、アラート、パンくずリスト |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js`, `menu.js` | パネル、タブ、分割ペイン；日付と時刻のピッカー（グリッドの日付フィルターも）；リストボックス、ルックアップ、複数選択；数値、伸びるテキストエリア、マスク、パスワード；明細行エディター；ステップ形式のフォーム；ツリー；ツールチップとポップオーバー；メニューボタンと分割ボタン；処理中オーバーレイ；サイドメニューの折りたためるグループ |
| `grid.js` | グリッド。`zeyn.js` が自分で import するため `<link rel="modulepreload">` としてリンクされます |
| `palette.js`, `pro.js` | コマンドパレットとコマンドボタン；メニューバー、ツールボックス、ステータスバー、ドッキングパネル、プロパティインスペクター |
| `colors.js`, `length.js`, `inspector.js` | カラーピッカー、単位切り替え付きの長さ、アイランドのプロパティインスペクター |
| `forms.js`, `reveal.js`, `import.js` | フォームの下書きとワークフローの遷移；マスクされた値の表示；インポート |
| `catalog.js`, `media.js`, `files.js`, `charts.js` とそのスタイルシート | カードグリッド、レール、検索ボックス；プレーヤー、埋め込み、画像；アップロード；チャート、スパークライン、KPI カード、ブレットチャート |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | 共有ボタン、フィルターチップ、リアクション、ヒーロースライド、設定ストア、カードのプレビュー（最初のホバー）、アシスタント（チャットパネル、またはランチャーの最初の押下） |
| `topics.js`, `vlist.js` | トピックを listen するノードと、それを drive するコントロール（`data-zeyn-listens`、`data-zeyn-drives`）、仮想リスト（`data-zeyn-vlist`） |
| `inbox.js`, `kanban.js`, `calendar.js` and their stylesheets, `diff.css`, `code.css`, `barcode.css` | 受信箱と通知ベル、Kanban ボード、カレンダー、スクリプトの要らない差分、コード表示、バーコード（コード表示のコピーボタンは `share.js` にあります） |
| `codeedit.js`, `codeedit.css` | コードエディター（`data-zeyn-code`）：色分けしたコピー、行番号、補完候補、エラーマーカー。`/_zeyn/data` への送信は入力が止まった後だけです |
| `range.js`、`quantity.js`、`gallery.js`、`countdown.js`、`commerce.css` | 範囲スライダー、数量ステッパー、商品ギャラリー、カウントダウン。すべてのショップ用コンポーネントのスタイルシート（価格、バリエーション選択、ファセットパネル、レビューの集計） |
| `imgcompare.js` | 画像の比較（`ImageCompare`）：スライダーを表示し、その値を `--z-imgcompare` に書き込みます |
| `tour.js` | ページのツアー（`Tour`）：開始ボタンを表示し、ダイアログをページの各ストップへ順に移動します |
| `thread.js` と `thread.css`, `stream.js`, `composer.js` と `composer.css`, `typing.js` と `typing.css` | メッセージスレッド、スレッドとアシスタントが共有するストリーミングの Markdown と JSON 行、チャットの composer、入力中インジケーター |
| `feed.js`, `lightbox.js` and their stylesheets, `stories.css` | 投稿のフィード、画像と動画のライトボックス、ストーリーリングとその行、スクリプト不要 |
| `drag.js`, `net.js` | 独自のマーカーを持たない共有ヘルパー。インポートするモジュールと一緒にリンクされる(下記) |

2つのヘルパーモジュールは、複数のモジュールが本来コピーするはずのコードをまとめています。`net.js`(`kanban.js`、`files.js`、`vlist.js` がインポート)は `BASE`、`csrf()`、`parse(text)`、`post(path, body)` をエクスポートします。`post` はページのオリジンで `x-zeyn-csrf` ヘッダー付きの JSON を送り、`{ ok, body }` を返します。`drag.js`(列の移動に `grid.js`、レールのドラッグスクロールに `catalog.js` がインポート)は、今後の並べ替えリスト、ボード、サーフェスの土台となるドラッグコアです。`press(e, { threshold, longPress, tolerance, axis, onStart })` は、マウスやペンが 6 px 動いたとき、またはタッチが 5 px 以内で 250 ms 押し続けられたときに `pointerdown` をドラッグに変えます(スワイプはそのままスクロールし、先にスクロールすると押下は取り消されます)。`capture(el, e, move, end)` はポインターキャプチャでドラッグを追い、`move` をアニメーションフレームごとに最大1回、離したとき・キャンセル時・Escape で `end(event, cancelled)` を呼びます。`swallowClick()` は離した後のクリックを取り消します。`announce(text)` はローカライズされたテキストを、ページに1つの polite ライブリージョンで最大 500 ms に1回読み上げます。`.z-drag-handle`(`zeyn.css` 内)はドラッグハンドル上でだけタッチのパンを止めます。ドラッグするすべてのコンポーネントは、キーボードと単一ポインターで移動する方法(移動メニュー、WCAG 2.5.7)も提供します。どちらのモジュールも HTML を書きません。

完全なページは head に、すべてのページが使うスタイルシート（`zeyn.css`、`components.css`、`content.css`、`pro.css`、`enterprise.css`）、HTML にマーカーがあるモジュールのスタイルシート、そしてそれらのスクリプトを、それぞれが import するモジュールの後にリンクします。`zeyn.js` は同じ表（`[name, script selector, style selector]`）付きで配信され、後から届くコンテンツに足りないものを読み込みます。`nav.js` は画面を差し替える前に `{ root, waits }` 付きの `zeyn:need` を発行して新しいスタイルシートを待ち、ウィンドウも開く前に同じように待ちます。`zeyn:content`、グリッドのページ、グリッドのフィルターは、含むもののスクリプトを読み込みます。ブラウザーは URL ごとに 1 つのインスタンスを保つので、モジュールは一度だけ実行されます。モジュールは実行時にドキュメントで見つけたものを設定し、その後は `zeyn:content` を待ち受けます。ページにないコンポーネントを探したり待ち受けたりすることはありません。モジュールは共有部分を自分の URL の `?v=` 付きで import します。`tests/ui_modules.rs` は、ページが使わないモジュールをリンクしたとき、またはモジュールのスタイルシートがマーカー外のクラスにスタイルを付けたときに失敗します。

showcase での計測（非圧縮、gzip での転送量）：テキストだけのページはスクリプトが 269 KB（80 KB）から 132 KB（46 KB）に、グリッドのページは 357 KB（106 KB）から 223 KB（74 KB）に、フォームのページは 269 KB（80 KB）から 186 KB（65 KB）になり、どのページもスタイルは 177 KB（44 KB）から 151 KB（35 KB）になりました。コンポーネントカタログのようにすべてのコンポーネントを持つページは、各モジュールが別ファイルのため、以前より多くのファイルを読み込みます（41 から 68 リクエスト、非圧縮 774 KB から 836 KB）。

## 2. Screen markup

すべての Screen のルートは、名前と params を持ちます。

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

Window は `class="z-window" role="dialog" aria-modal="true"` と `--z-width` スタイルを使います。

body の各項目はノード要素で包まれます。そのパスが、以下のエンドポイントで使うアドレスです。コンテナ（`Row`、`Stack`、`Panel`、`Card`、`Tabs`、`Split`）も各子を同じ方法で包み、子のパスにはインデックスが加わります。body 項目 1 の最初の子の 3 番目の子は `"1.0.2"` です。

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

エンドポイントは `node` を、ドット区切りの文字列、または body 項目の場合は単純な数値（`0`）として受け取ります。パスは数字で最大 16 階層までで、それ以外は `422` で拒否されます。

## 3. Attributes the runtime reads

| 属性 | 付与先 | 意味 |
|---|---|---|
| `data-zeyn-effect` | button、`tr` | クリック時に実行する JSON の effect（行ではダブルクリックまたは Enter） |
| `aria-keyshortcuts` | button | キーボードショートカット（例: `Control+N`） |
| `data-zeyn-export` | button | 同じ Screen 上の grid のエクスポート形式（`csv`、`xlsx`） |
| `data-zeyn-grid` | grid ルート | grid を示します。`data-zeyn-mode`、`data-zeyn-paging`、`data-zeyn-page-size`、`data-zeyn-total`、`data-zeyn-next`、`data-zeyn-offset` も付きます |
| `data-field`、`aria-sort`、`aria-colindex` | `th` | 列のフィールド名、現在のソート、位置 |
| `data-filter`、`data-options` | `th` | フィルター入力の種類（`text`、`number`、`money`、`date`、`datetime`、`time`、`choice`、`bool`）と、選択肢 `[{value,label}]` |
| `data-pin` | `th` | 固定列の `start` または `end`（選択列と行アクション列も含む） |
| `data-edit`、`data-optional` | `th` | 保存できるユーザー向けの、編集可能な列の入力の種類 |
| `data-zeyn-texts`、`data-zeyn-columns`、`data-zeyn-view` | grid ルート | ローカライズされたコントロールのテキスト、全列 `[{field,label,hidden,pin,width}]`、保存されたビュー |
| `data-zeyn-views` | grid ルート | ビューをサーバーに保存する場合は `server` |
| `data-zeyn-version-field`、`data-zeyn-exportable` | grid ルート | インライン編集用の行バージョンのフィールド名。grid をエクスポートできる場合は `csv` |
| `data-zeyn-key`、`data-zeyn-version`、`data-zeyn-menu` | `tr` | 行キー、行バージョン、コンテキストメニューの項目 |
| `data-v` | `td` | 編集可能なセルの入力値 |
| `data-zeyn-detail`、`aria-expanded` | `tr` | `{screen, params}` 形式の行の詳細 Screen、および開いているかどうか |
| `data-zeyn-expand` | 行内の button | 行の詳細、グループ、子を開閉します |
| `data-zeyn-level`、`aria-level`、`data-zeyn-path` | `treegrid` 内の `tr` | グループ化または tree の grid での深さ（最上位が 0）。行を開くときに送る、グループ値または祖先キーのパス |
| `z-group` | `tr` | グループヘッダー行。`z-branch-more` は枝の「もっと読み込む」行 |
| `data-zeyn-run` | 行内の button | 実行する行の項目（`a0` は最初の行アクション） |
| `data-agg` | `tfoot td` | 列が表示する集計（`sum avg min max count`） |
| `data-zeyn-bulk` | grid の前の div | `data-zeyn-bulk-run` ボタンを持つ選択バー |
| `data-zeyn-menu` | Screen ルート | Screen のコンテキストメニュー |
| `data-zeyn-native-menu="off"` | `html` | テキスト入力以外ではブラウザのコンテキストメニューを無効にします |
| `data-zeyn-rows` | `tbody` | 行を入れる場所 |
| `data-zeyn-page` | ページャーの button | `prev` または `next` |
| `data-zeyn-page-info` | span | `1-50 of 230` のようなテキスト。grid ルートのローカライズされたテンプレートから作られます |
| `data-zeyn-text-range`、`data-zeyn-text-range-total`、`data-zeyn-text-loaded` | grid ルート | ローカライズされたテンプレート: `{from}-{to}`、`{from}-{to} of {total}`、`{count} rows loaded` |
| `data-zeyn-more` | div | 無限スクロールのセンチネル。「もっと読み込む」ボタンを含みます |
| `data-zeyn-submit` | form | submit action の名前 |
| `data-zeyn-editor` | input | カスタムフィールド型のクライアント側エディター |

Effects:

| Effect | JSON | ランタイムの動作 |
|---|---|---|
| Open window | `{"effect":"open_window","screen":"product_form","params":{...}}` | Screen（セクション 4.1）をモーダルの `<dialog>` に読み込みます |
| Navigate | `{"effect":"navigate","screen":"...","params":{...}}` | `/_zeyn/route/{screen}?params=` を読み込み、ページへリダイレクトします。ルートにセグメントがある場合はパス形式になります |
| Toast | `{"effect":"toast","message":"Saved"}` | トーストを 4 秒間表示します |
| Close and refresh | `{"effect":"close_and_refresh"}` | 最前面のダイアログを閉じ、その下の Screen にあるすべての grid を再読み込みします |

## 4. Endpoints

すべての `POST` リクエストは JSON を送り、`zeyn-csrf` meta タグのトークンを入れたヘッダー `x-zeyn-csrf` を必ず付けなければならず、付いていない場合は `403` で拒否されます。

セッションなしで開かれた public Screen は、キャッシュできるように、空の `zeyn-csrf` meta タグを持ち、`Set-Cookie` も付きません。最初の `POST` の前に、ランタイムは `GET /_zeyn/csrf` を呼び出します。これは `{ "token": "..." }` を `Cache-Control: no-store` 付きで返し、ブラウザに `zeyn_csrf` Cookie がなければ設定し、`Sec-Fetch-Site` が `same-origin` でないリクエストは拒否します。トークンは meta タグに入ります。呼び出しが失敗した場合、`POST` はトークンなしで送られ、ほかのリクエストと同様に拒否されます。黙って再試行することはありません。

アプリ内ナビゲーション（`nav.js`）は、`x-zeyn-partial: 1` を付けて Screen を要求し、HTML だけを受け取ります。ページタイトルは `x-zeyn-title` に入ります。Screen が head タグ（description、canonical URL、プレビュー画像、robots、JSON-LD）を設定する場合、body はそれらを収めた `<template data-zeyn-head>` で始まります。`nav.js` はドキュメントの head にある `[data-zeyn-head]` 要素をすべて削除し、template のタグをそこへ移し、template を破棄します。head タグのない Screen は template を送らないため、古いタグは削除されるだけです。

### 4.1 Screen fragment

`GET /_zeyn/screen/{screen}?params={json}` は、ページシェルなしで Screen の HTML だけを返します。Window と行の詳細で使われます。`data-zeyn-detail` を持つ行を開くと、その Screen が直下の新しい `tr.z-detail`（行全体にまたがる 1 つのセルで、`role="region"` を持つ）に読み込まれ、閉じるとその行が削除されます。Screen の権限はほかの Screen と同じように確認されます。詳細内の grid は、その詳細 Screen 自身の名前と params でサーバーとやり取りします。

### 4.2 Grid rows

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

フィルターには `contains`、`starts_with`、`equals`、`from`、`to`、`any_of`、`is` を設定できます（`docs/grid.md` を参照）。`view.columns` は、行をレンダリングするときの列レイアウトです。`head: true` の場合、レスポンスにはヘッダー行の `head` も含まれます。

無限スクロールでは、`page` は `{ "type": "after", "cursor": "...", "limit": 100 }` で、最初のバッチでは `cursor` は `null` です。

レスポンス `200`:

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

`foot` は、合計が要求された場合の合計行で、それ以外は `null` です。

`total` と `next` は `null` になることがあります。ランタイムは、ページング方式の grid では行を置き換え、無限スクロールでは行を追加します。

グループ化 grid と tree grid（`<table role="treegrid">`）は、枝を読み込むために `POST /_zeyn/grid` で `path` を送ります。これは、開こうとしている行のグループ値（または祖先キー）で、その行の `data-zeyn-path` にあるものです。レスポンスの形は同じで、`rows` にはグループ行または 1 階層深い行が入り、`total` はそれらの数です。ランタイムはそれらを開いた行の後ろに置き、行が閉じるときには、その後ろに続くより深い行をすべて削除します。grid のグループ階層（tree では 32）より長い path は `404` です。

### 4.2.1 More grid endpoints

| エンドポイント | Body | 動作 |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | ユーザーのビューを保存します（`null` で削除）。`204`。ビューストアがない場合は `404` |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | 行の項目（`a0` は行アクション、`m2.1` はメニューパス）を実行します。サーバーがキーで行を読み込み、入力を組み立てます。`{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | 選択されたキー、または `all` の場合は `query.filters` に一致するすべての行に対して、一括 action を実行します。`{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | インライン編集を保存します。`values` には、form が送るのと同じ形で、編集した列と行バージョンが入ります。`{row}`。編集できない列は `403`、ほかに `409`、`422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | 一致するすべての行を CSV としてストリーミングします |
| `POST /_zeyn/menu` | `{screen, params, item}` | Screen のコンテキストメニューにある action 項目を実行します |

### 4.2.2 Virtual scrolling

`data-zeyn-paging="virtual"` を持つ grid は、行がいくつあっても、すべての行にわたる 1 つのスクロール可能なリストを表示します。サーバーは最初のブロック（`data-zeyn-page-size` 行、オフセット 0）を `data-zeyn-total` 付きでレンダリングします。最初の 64 行はテーブルに、残りはその後ろの `<template data-zeyn-rows-later>` に入り、grid が 40 列以上を表示する場合は、すべての行がテンプレートに入ります。grid ルートには `data-zeyn-row-height`（ピクセル）と CSS 変数 `--z-row-height` もあります。

ランタイムの動作:

- テーブル本体を、内側のスペーサーの高さが `total * rowHeight` ピクセル（上限 8,000,000 px）のスクロールコンテナに入れます。上限を超える場合、スクロール位置は比率で行インデックスに対応付けられる（`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`）ため、1,000,000 行でも末尾に到達できます。
- DOM には、表示中の行とその上下 20 行だけを保持し、`transform: translateY(...)` で配置します。行の高さは固定で、長いテキストは省略記号で切り詰められます。
- `POST /_zeyn/grid` で、`data-zeyn-page-size` 行のブロックを `page: { "type": "offset", "offset": blockStart, "limit": blockSize }` として読み込み、最初の呼び出しの後は `with_total: false` にします。最大 20 ブロックかつ最大 100,000 セルをキャッシュに保持し（最も長く使われていないものから破棄。63 列の grid では 7 ブロックで、3 を下回ることはありません）、`reload` のために保持するブロックも同じ上限に数えられ、最初に破棄され、スクロール方向の次のブロックを先読みし、表示範囲から外れたブロックのリクエストは取り消します（`AbortController`）。
- ブロックの読み込み中、表示中の行を消すことはありません。足りない行は、そのブロックが届くまで `aria-busy="true"` のプレースホルダー行を表示するため、画面がちらつきません。
- ソートやフィルターが変わると、キャッシュを破棄して先頭へスクロールします。
- キーボード: 矢印キー、Page Up、Page Down、Home、End でアクティブな行が移動し、表示範囲にスクロールされます。テーブルには `aria-rowcount` = total が、各行には `aria-rowindex` が付きます。
- 位置を示すテキスト（`data-zeyn-page-info`）は `data-zeyn-text-range-total` を使います。

カードグリッド（`zeyn::ui::CardGrid`。rail や行のリストは含みません）は、カードが 200 枚になると、スクロールコンテナではなくページ自体のスクロールに対してウィンドウ処理を行います。`catalog.js` はリストに `data-zeyn-window` を付け、ビューポートから 800 px 以内の行をその中に保持し、上下の行はパディングに置き換え、各行に最初のカードのうち最も高いものの高さを与え（`grid-auto-rows`）、各カードに `aria-posinset` と `aria-setsize`（まだページが続く可能性がある間は `-1`）を設定します。ウィンドウの外にあるフォーカス中のカードは `z-cards-kept` 付きで DOM に残り、自分のセルの上に配置されます。「もっと見る」や別のページから来たカードも同じウィンドウに加わります。ページが 1 ms あたり 3 px より速くスクロールしている間、カードは画像を保留した状態で入り（`data-zeyn-park`。`src` と `srcset` は別に保持）、スクロールが 120 ms 止まると画像を取り戻します。

### 4.3 Form submit

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

値は、ユーザーが入力したままの形で送られます。input と select は文字列、チェックボックスは真偽値です。サーバーはフィールドの型に従ってそれらを解析します。

サーバーは Screen が読み込んだモデルから始め、form 上にあり読み取り専用でないフィールドだけを上書きします。クライアントからのそれ以外の値は無視されます。キーがクライアントから来ることはありません。

レスポンス:

- `200 { "effects": [ ... ] }`: 保存済み。effects を順に実行します。
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`: 各メッセージを対応するフィールドの下（`#{scope}-{field}-error`）に表示し、入力に `aria-invalid="true"` を設定し、最初の無効な入力にフォーカスします。`field` が空のエラーは form の上部に表示されます。

### 4.4 Errors

どのエンドポイントも次のように応答することがあります。

| ステータス | Body | 意味 |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | 権限がない、ガードに拒否された、または CSRF トークンが不正 |
| `404` | `{ "error": "not_found", "message": "..." }` | 不明な Screen またはノード |
| `409` | `{ "error": "conflict", "message": "..." }` | ほかの誰かがレコードを変更した |
| `422` | 4.3 を参照 | 検証 |
| `504` | `{ "error": "timeout", "message": "..." }` | 呼び出しがサーバーの制限より長くかかった |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | 詳細はサーバーログにのみ残ります |

ランタイムは `message` をエラートーストとして表示します。

### 4.5 Language

`POST /_zeyn/locale` に `{ "locale": "tr" }` を送ると、選択が `zeyn_locale` Cookie に保存され、`204` が返ります。ランタイムはページを再読み込みします。Cookie がない場合、サーバーは `Accept-Language` から選び、次にアプリのデフォルトを使います。`<html lang>` は常に選択されたロケールと一致します。

アプリに複数のロケールがある場合、シェルは言語セレクター（`select[data-zeyn-locale]`）を描画します。

### 4.6 Theme

アプリがセレクターを隠している場合や、テーマが 1 つしかない場合を除き、シェルはテーマセレクター（`select[data-zeyn-theme-picker]`）を描画します。各 option には `data-scheme`（`light`、`dark`、System は空）が付き、アプリのテーマには `data-custom` が付きます。変更時、ランタイムは `<html>` の `data-theme` と `data-zeyn-theme`、および `color-scheme` meta を即座に設定し、その後 `{ "theme": "dark" }` を付けて `POST /_zeyn/theme` を送ります。サーバーは `zeyn_theme` Cookie を付けて `204` を返し、アプリが提供していないテーマには `404` を返します。ページはその Cookie のテーマでレンダリングされるため、再読み込みもちらつきもありません。`docs/theming.md` を参照してください。

### 4.7 Enterprise endpoints

`enterprise.js` と `enterprise.css` はランタイムと並べて読み込まれます。詳細は `docs/enterprise.md` にあります。

| エンドポイント | 目的 |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | マスクされたフィールドの生の値 `{value}`。unmask 権限が必要で、常に監査されます |
| `POST /_zeyn/reason {screen, reason}` | `reason` が指定された Screen の理由。`HttpOnly` Cookie 付きの `204` |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV のプレビュー（`commit: false`）またはインポート |
| `POST /_zeyn/keepalive` | アイドル警告後の「サインインしたままにする」。`204` |
| `GET /_zeyn/health`、`GET /_zeyn/ready` | ライブネスとレディネス。health はレート制限されず、readiness は `probe_ips` からのみ |
| `GET /_zeyn/jobs` | jobs 権限を持つユーザー向けのジョブステータス |

すべてのレスポンスに `x-zeyn-build` が付きます。ランタイムが処理する拒否は、`428 reason_required`（理由を尋ねてリクエストを繰り返す）と `401 reauth_required`（ユーザーに再サインインを求めてリクエストを繰り返す）です。

## 5. Security

- すべてのレスポンスは `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`、`X-Content-Type-Options: nosniff`、`Referrer-Policy: same-origin` を設定します。`App::csp` は、指定した `https` ホストを `img-src`、`media-src`（および `blob:`）、`frame-src`、`connect-src`、`font-src` に追加します。ほかのディレクティブは変わりません。ヘッダーの文字列は起動時に一度だけ作られます。
- インラインスクリプトやインラインイベントハンドラーはありません。インラインの `style` 属性は CSS カスタムプロパティ（`--z-width`、`--z-flex`）と固定の列幅にのみ使われるため、ポリシーに `style-src-attr 'unsafe-inline'` が加わっています。スクリプトは厳格なままです。
- CSRF トークンは `HttpOnly`、`SameSite=Strict` の Cookie に保存され、`x-zeyn-csrf` ヘッダーと定数時間で比較されます。
- ページ、`totp` フォーム、`GET /_zeyn/csrf` は、トークンを Cookie にあるそのままの形では載せません。レスポンスごとに新しいランダムなパッドの後ろに、トークンとパッドの XOR（16 進数 128 桁）を付けるため、gzip でトークンが漏れることはありません（BREACH）。サーバーは比較の前にマスクを外し、通常の 64 桁のトークンも引き続き受け付けます。
- 1 KB 以上の動的レスポンス（ページ、部分的な Screen、grid ブロック、エクスポート）は、ブラウザが受け付ける場合にレベル 2 で gzip 圧縮され、`Vary: Accept-Encoding` が付きます。圧縮しないもの: すでに `Content-Encoding` があるレスポンス（一度だけ圧縮してキャッシュするフレームワークのアセット）、範囲（`Content-Range`、および `Range` を持つすべてのリクエスト）、動画、音声、SVG 以外の画像、`application/octet-stream`、アーカイブと PDF、`text/event-stream`、`application/x-ndjson`（アシスタントのストリーム）、WebSocket のアップグレード。
- 各呼び出しは次の順に確認します。認証、Screen のアクセス、feature のガード、ノードの action のアクセス。その後、インターセプターと action を実行します。

## 6. Runtime behavior

- JavaScript がなくても、ページはすべての grid の最初のページ付きでレンダリングされます。form とページングにはランタイムが必要です。
- キーボード: 矢印キーで grid の行間を移動し、Enter で行を開き、Escape で最前面のダイアログを閉じ、`aria-keyshortcuts` のショートカットはテキスト入力の中を除くどこでも動作します。grid では次のとおりです。Space で行を選択、F2 で編集、Ctrl+Z で最後の編集を元に戻す、Shift+F10 またはコンテキストメニューキーで行またはヘッダーのメニューを開く、Shift+クリックで複数列によるソート、Right と Left（右から左のページでは反転）で行の詳細、グループ、子を開閉し、子の行での Left は親へ移動します。行での Right はそのセルの中へ移動します。矢印、Home、End、Ctrl+Home、Ctrl+End、Tab、Shift+Tab でセルカーソルを動かし、Shift で範囲を広げ（セルに `aria-selected`）、Ctrl+C で範囲をタブ区切りテキストとしてコピーし、Ctrl+V で編集可能なセルへ貼り付け（変更された行ごとに 1 回の `POST /_zeyn/grid/edit`）ます。40 列以上の grid は、表示範囲内の列だけをページ内に保持します（固定幅の `colgroup`、`colspan` を持つ `td.z-col-gap` のスペーサー、セルの `aria-colindex`）。
- 右クリックは、定義されている場所（ヘッダー、`data-zeyn-menu` を持つ行、`data-zeyn-menu` を持つ Screen）では Zeyn のメニューを開きます。それ以外の場所ではブラウザ自身のメニューがそのまま使われます。
- ダイアログが開くとフォーカスがその中へ移動し、閉じると開いた要素に戻ります。
- クリックと同じフレームで待機状態を示します。画面の読み込み中、`nav.js` は移動先のリンクを現在の項目にし、`html[data-zeyn-loading]` を設定します。ウィンドウとフォーム送信も同じ属性を設定し、グリッドはソート、ページ、フィルターのキーが押されるとすぐに行を薄くします（`aria-busy`）（フィルターは次のキーを 150 ms 待ち、新しいリクエストが古いものを取り消します）。ウィンドウ上端の細いバーは、応答が 150 ms より長くかかるときだけ表示されます。固定配置なので何も動かず、`prefers-reduced-motion` では動きません。
- クライアント側のチェック（`required`、`minlength`、`maxlength`、`min`、`max`、`type="email"`）は、送信前にブラウザの制約検証 API で実行されます。サーバーは常に再度検証します。

## 7. Component library

`zeyn::ui` のコンポーネントは、1.1 節のモジュールとスタイルシート `/_zeyn/components.css` で配信されます。ランタイムと同じく、サインインは不要です。

### 7.1 Endpoints

すべて、JSON と CSRF ヘッダーを使う `POST` で、最初に Screen のアクセスと feature のガードを確認します。

| エンドポイント | Body | 応答 |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | 1 つのノードの HTML（その grid の最初のページ付き）。遅延 tabs で使われます。 |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`: 画面を 1 回実行してレンダリングした複数のノード。更新または drive されたトピックのリスナー用です。異なるパスは最大 32 個で、画面にないパスは `404` を返します。画面のアクセス、guard、interceptor は 1 回だけ実行され、params は画面の型付き params として読まれます。 |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | ノードのデータソース `key` からの JSON。lookup の選択肢（`key` はフィールド名）、tree の子（`key` は空）、ステップの確認（`key` は `_step`）。ソースの action 権限が確認され、grid と同様にインターセプターが実行されます。action に合わない入力は `404` で拒否されます。 |
| `screen` = `_site.{feature}` の `/_zeyn/data` | 上と同じ | サイトヘッダーのスロット内のコンポーネント向けの同じ処理。`node` はスロット（0 は start、1 は center、2 は end、3 は bottom）、その中でのノードのインデックス、続いて子のインデックスです。Feature のガードと action の権限が確認されます。 |
| `/_zeyn/parse` | `{ kind, text, label }` | 入力された日付または時刻を、サーバーがユーザーの言語、ゾーン、時計でどう解釈するか。`{ ok, text, iso, utc, zone }` または `{ ok: false, message }`。`kind` は `date`、`time`、`datetime`、`local`、`presets`（今日とクイック範囲）です。 |

### 7.2 Values the forms submit

- Lookup は ID をテキストで送ります。`"7"`、複数の場合は `"3,7,9"` です。
- Lines は `"{field}": "rows"` と、セルごとに 1 つの値を `"{field}.{line}.{column}"` として送り、さらに `"{field}.{line}._i"` も送ります。これは、その行の元になった読み込み済みの行のインデックスです（新しい行では空）。サーバーは各 line を読み込み済みの line から再構築するため、表示されていて編集可能な列だけが変わります。
- line の検証エラーは `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }` の形を使います。

### 7.3 Events

| イベント | 対象 | Detail |
|---|---|---|
| `zeyn:content` | `document` | 新しいコンテンツを持つ要素（ナビゲーション、遅延パネル）。ランタイムが必要なモジュールを読み込み、読み込まれた各モジュールが中身を設定します。 |
| `zeyn:errors` | form | ランタイムが表示した後の `422` エラー。line エディターとステップ形式の form が使います。 |
| `zeyn:effect` | `document` | ランタイムが知らない effect: `{ effect, source, run }`。`run(next)` は別の effect を実行します。`confirm` で使われます。 |
| `zeyn:topics` | `document` | `{ topics }`: ほかのスクリプト（通知を受けた `live.js`）からの、更新するトピック。 |
| `zeyn:reload` | 要素 | その中のグリッドをその場で再読み込みします。現在のページ、または `{ reset: true }` なら最初のページです。 |
| `zeyn:density` | `document` | なし。ユーザーがアカウントメニューで別の密度を選びました（`<html>` の `data-density` はすでに変わっています）。ウィンドウ化されたカードグリッドが行を測り直します。 |

Effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` は、モーダルのアラートダイアログで確認し、ユーザーが承諾した後にのみ `then` を実行します。

Effect `{"effect":"refresh","topics":["sales"]}` はトピックのリスナーを更新します。`[data-zeyn-listens="a,b"]` はリスナー（親 `data-zeyn-node` のノード）を示し、`[data-zeyn-drives="a"]` はトピックを drive するコントロールを示します。フィルターフォーム（`data-zeyn-kinds` はどの params が数値または真偽値かを示します）、検索ボックス（フィールド名は `q`）、フィルターチップ（各チップは自分の params を `data-zeyn-params` に持ちます）です。drive すると、コントロールの値が画面ルートの `data-zeyn-params` にマージされ、その画面がメイン画面ならアドレスがそれで置き換えられ、リスナーが更新されます。`topics.js` は 1 つのアニメーションフレームで指定されたトピックをまとめて処理します。グリッドを持つリスナーは `zeyn:reload` を受け取り、画面のほかのリスナーは 1 回の `POST /_zeyn/nodes` にまとめられ、同じ画面への新しいリクエストは古いものを中断してそのノードを引き継ぎます。フォーカスと開いている `<details>` は、id、test id、`data-zeyn-key` の行、位置で保たれます。

仮想リストは `data-zeyn-row-height`、`data-zeyn-block`、`data-zeyn-total` を持つ `section[data-zeyn-vlist]` です。`vlist.js` は `{ page: { type: "offset", offset }, with_total: true }` で `POST /_zeyn/grid` にブロックを要求し、各 `<li>` を `(aria-posinset - 1) * height` の位置に置きます。別のフィールドに依存する lookup は `data-depends-on="field"` を持ち、そのフィールドの値を `/_zeyn/data` の入力の `parent` として送ります。

クライアント設定（`zeyn::ClientPref`）はブラウザーのモジュール `/_zeyn/state.js` にあり、ページはこのモジュールを、`data-zeyn-remember` または `data-zeyn-prefs-clear` を持つ要素がある場合にだけインポートします。`GET /_zeyn/prefs` は `{ preferences }` を返します。これは、アプリの同意フックが、設定値をセッションを越えて保持することを許可するかどうかを表し、`Cache-Control: no-store` が付きます。サーバーは保存された値を読み取りません。

### 7.4 Media

`Video` と `Audio` は `/_zeyn/media.js` と `/_zeyn/media.css` で配信され、コンポーネントライブラリの後にリンクされます。`docs/media.md` を参照してください。

プレーヤーは、最初に表示範囲に入ったときに、ページで 1 つの `IntersectionObserver` を通じてソースとポスター（`data-zeyn-poster`、先に `data-zeyn-poster-low`）を設定します。`data-zeyn-preview` を持つルートは、4 分の 1 が見えている間ミュートでループ再生します。ただし `prefers-reduced-motion`、`prefers-reduced-data`、Save-Data がそれを望まない場合は除き、訪問者がコントロールを操作するまでに限ります。読み込みオプションを持つ画像は、スクリプトなしでプレースホルダーを表示するボックス（`.z-img-host`）の中にあり、`media.js` を読み込む目印である `data-zeyn-img` を持ちます。document 上の 1 つのキャプチャリスナーが、それらの画像の `load` と `error` を扱います。読み込みが終わるとボックスに `z-img-done` を付け（低解像度ファイルの上でフェードするときは `z-img-fade` も）、失敗すると `data-zeyn-fallback` を 1 回試し、その後ボックスの `<template data-zeyn-img-error>` か、アイコン付きの代替テキストを表示します。モジュールの実行前に終わっていた画像は、モジュールの開始時に 1 回確認されます。

| エンドポイント | 動作 |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | 登録された media ソースのファイルを range リクエストで返します。`200`、`206`、`304`、`416`。 |
| `GET /_zeyn/media/position?source=&id=` | サインインしているユーザーが止めた位置 `{ at, kept }`。ソースが位置を保持しない場合、`kept` は `false` です（その場合、プレーヤーは local storage を使います）。 |
| `POST /_zeyn/media/position` | CSRF ヘッダー付きの `{ source, id, at }` で保存します。`204`。位置が不正なら `422`。 |

### 7.5 Islands

`islands` feature を有効にすると、`div[data-zeyn-island]` を含む Screen は、`<meta name="zeyn-islands">`（island 名から `{ js, wasm }` の URL への対応）、`/_zeyn/islands.js`、および `'wasm-unsafe-eval'` を含むポリシーとともに配信されます。ほかのページは変わりません。island を含む partial の応答には `x-zeyn-islands: 1` が付き、ドキュメントに island のポリシーがない場合、`nav.js` はそれを完全な読み込みで開きます。

| 属性またはエンドポイント | 意味 |
|---|---|
| `data-zeyn-island`、`data-zeyn-load`、`data-zeyn-props` | island 名、`visible`、`interaction`、`idle` のいずれか、props の JSON |
| `data-zeyn-island-fallback` | フォールバックのコンテンツ。island がマウントされると隠れます |
| `data-zeyn-island-state` | ローダーが設定します: `loading`、`ready`、`failed` |
| `GET /_zeyn/islands/{file}` | マニフェストに載っているビルド済みファイル。1 年間キャッシュされます |
| `key` = リモート名の `POST /_zeyn/data` | island からの型付き呼び出し。7.1 のすべての確認が行われます |

`docs/islands.md` を参照してください。

### 7.6 Realtime

`realtime` feature を有効にすると、`div[data-zeyn-live="topic"]` は、サーバーがその topic を通知したときに更新されるノードを示します。これを含むページは `/_zeyn/live.js` をリンクします。これを含む partial の応答には `x-zeyn-live: 1` が付き、`nav.js` がそのスクリプトをインポートします。要素上の `zeyn:reload` は、その内部の grid をその場で再読み込みします。

| エンドポイント | 動作 |
|---|---|
| `GET /_zeyn/socket` | WebSocket。テキストフレーム `{"sub":["a"]}` ⇅ `{"s":1,"t":"a"}` |
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=` | WebSocket。双方向でバイナリフレーム `0x01` + Y update (v1) |

ハンドシェイクは、アップグレードの前に、origin、Cookie に対する CSRF トークン、サインインしているユーザー、権限を確認します。`docs/realtime.md` を参照してください。

プレーヤーは、プログレッシブファイルにフォールバックする前に、そのルートでキャンセル可能な `zeyn:media-adaptive` イベント（`{ media, kind, url, hls, dash }`）を発行します。ストリーミングライブラリは `preventDefault()` を呼んで処理を引き継ぎます。

## 8. Desktop class shell

`/_zeyn/pro.js` と `/_zeyn/pro.css` は、コンポーネントライブラリの後に読み込まれます。ランタイムと同様に、サインインは不要です。

### 8.1 Commands and the palette

コマンドは Rust で宣言され（`Command::open`、`Command::navigate`、`Command::effect`、`Command::action`）、`Feature::command` で登録されます。シェルは、ユーザーが実行できる（feature のガードを通り、コマンドの access が許可している）コマンドごとに、`div[data-zeyn-commands]` の中に非表示のボタンを 1 つ描画します。

| 属性 | 意味 |
|---|---|
| `data-zeyn-command` | コマンド ID |
| `data-zeyn-effect` | effect コマンドの effect。クリック時にランタイムが実行します |
| `data-zeyn-command-run` | action コマンドの ID。`POST /_zeyn/command` で実行されます |
| `aria-keyshortcuts` | 正規化されたショートカット（`Control+Alt+N`）。ランタイムのショートカット処理がボタンをクリックします |
| `data-hint`、`data-icon` | ユーザーに表示されるショートカット、アイコン名 |
| コンテナ上の `data-text-*` | ローカライズされたパレットのテキスト |

`App::check` は、次のものを拒否します。読み取れないショートカット、1 文字だけのショートカット（WCAG 2.1.4）、パレット自身のショートカット、2 つのコマンドで使われているショートカットまたは ID、ブラウザがページに渡さないショートカット（`BROWSER_RESERVED`: `Ctrl+N`、`Ctrl+T`、`Ctrl+W`、`Ctrl+Tab`、`Alt+F4` とその macOS 形式）、そして、コマンドが `.override_browser()` を呼ばない限り、ブラウザが使うショートカット（`BROWSER_DEFAULTS`: 印刷、保存、検索、アドレスバー、再読み込み、ズーム、開発者ツール、コピーと貼り付け、`F1` から `F12`）。ツールバーのボタンも実行時に同じルールに従います。そのようなショートカットはバインドされず、理由がログに記録されます。空いている組み合わせは `Alt+Shift+<letter>` と `Ctrl+Alt+<letter>` です。

`Alt+Shift+P`（macOS では Option+Shift+P）がデフォルトでパレットを開きます。`App::palette_shortcut("Ctrl+Shift+P")` で変更でき、意図的にブラウザが使うキーにすることもできます。ショートカットは `data-palette-keys` に入り、文字は物理キーで照合されるため、Option や AltGr で壊れません。パレットはメニュー内の `button[data-zeyn-palette-open]` からも開きます。これは ARIA の combobox と listbox を持つモーダルの `<dialog>` です。アプリのコマンド、現在の Screen（Window が開いている場合は最前面の Window）のツールバーボタン、メニューバーのコマンド、コンテキストメニューの項目、およびメニューにある Screen を一覧にします。照合はあいまい一致で、大文字小文字とアクセントを無視します。直近に実行した 8 つのコマンドが先頭に来ます（`localStorage` のキー `zeyn.palette.recent`）。Escape で閉じ、フォーカスは元の場所に戻ります。

| エンドポイント | Body | 応答 |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | サーバー側で入力が固定された action コマンドを実行します。CSRF、feature のガード、action の権限、インターセプター。`{ effects }`。不明な ID と effect コマンドは `404` |
| `POST /_zeyn/search` | `{ query }` | ユーザーが実行できるすべての `Feature::search` を実行します（CSRF、feature のガード、action の権限、インターセプター）。クエリは 100 文字で切り詰められます。`{ hits: [{ label, group, url }] }`。最大 20 件で、アプリのパスのみ |

コンテナに `data-search` がある場合、パレットは最後のキー入力の 150 ms 後（2 文字以上）にクエリを送り、コマンドの下にヒットを一覧表示します。最新のクエリへの応答だけが描画されます。ヒットを選ぶと、`url` の前にベースパスを付けた `zeyn:navigate` が送られます。その中の `#anchor` は、Screen が差し替えられた後にその要素へスクロールします。

ランタイムは `document` 上の `zeyn:run` を待ち受けるため、ほかのスクリプトはそれを通して処理を実行できます。`{ effects, source }` は effects を実行し、`{ screen, item }` は Screen のコンテキストメニュー項目を実行し、`{ error }` はエラートーストを表示します。

### 8.2 Menu bar, toolbox, property inspector, status bar

| マークアップ | 動作 |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG の menubar。トップ項目は、ローヴィングタブストップを持つ `button[role=menuitem][aria-haspopup=menu]` で、それぞれが閉じている間は `hidden` の `ul[role=menu]` を制御します。項目には 8.1 のコマンド属性が付きます。Left と Right でメニュー間を移動し、Down、Enter、Space で開き、Up で最後の項目を開き、Home と End でジャンプし、文字キーでその文字で始まる次の項目へ移動し、Escape で閉じてバーに戻り、Tab と外側のクリックで閉じます。ユーザーが実行できないメニューや項目は描画されません。 |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | `button.z-tool[aria-pressed]` のトグルグループ。矢印、Home、End でローヴィングタブストップが移動します。クリック、Enter、Space でツールを 1 つオンにし、`zeyn:tool`（`detail.tool`）を送り、ステータスバーに `data-text-selected` を書き込みます。 |
| `section.z-inspector` | `Form::section` のグループ（`role=group` の本体を持つ `details.z-form-section`）を持つ form で、inspector のノードパスの `POST /_zeyn/form` で保存されます。失敗したフィールドを含むセクションは、ブラウザのチェックの前、および `422` の後に開きます。 |
| `section.z-inspector[data-zeyn-inspector=name]` | フィールドの種類ごとに `template[data-kind]` を 1 つ持つアイランドインスペクター。`[data-zeyn-island=name]` の内側から来た `zeyn:inspect`（`detail: { title, schema, values }`）は、`innerHTML` を使わず DOM 呼び出しでスキーマを描きます。`data-limits` を超えるスキーマは無視され、以前に見たスキーマはフィールドを再利用します。有効で変更された値は `zeyn:property`（`detail: { name, value }`）としてアイランドのルートへ送られ、無効な値はフィールドのエラー行を埋めて `aria-invalid` を付けます。 |
| `div[data-zeyn-statusbar]` | 先頭と末尾の項目、および `[data-zeyn-status-live]`（`role=status`、`aria-live=polite`）。Screen 内のどの要素からでも `zeyn:status`（`detail.text`）を送ると、ライブテキストが置き換わります。 |

### 8.3 Docking panels

`div.z-dock[data-zeyn-dock]` は、4 つの `section.z-dock-area[data-area]`（`left`、`center`、`bottom`、`right`）と、3 つの `div.z-dock-bar[role=separator][data-edge]` の区切りからなる grid です。サイズはカスタムプロパティ `--z-dock-left`、`--z-dock-right`（幅に対するパーセント）、`--z-dock-bottom`（高さに対するパーセント）で、10 から 60 です。各エリアには、ドラッグ可能な `button.z-dock-tab[data-panel]` の `role=tablist`、オプションボタン、子ノードを収める `role=tabpanel` のパネルがあります（パスはどのコンテナとも同じで、パネルが移動しても変わりません）。開いているパネルがない側面のエリアは隠れます。

キーボード: Left と Right（右から左のページでは反転）、Home、End でエリアの開いているタブ間を移動します。Delete でフォーカスされたパネルを閉じます。Shift+F10 またはコンテキストメニューキーでエリアのオプション（別のエリアへ移動、閉じる、閉じたパネルを表示）を開きます。区切りの上の矢印キーは 5 パーセントずつサイズを変更し、Home と End は限界へ移動します。`MenuList::panels` で作ったメニューバーのエントリは、すべてのパネルを `menuitemcheckbox` として一覧表示します。変更は `[data-zeyn-dock-live]` で通知されます。

| エンドポイント | Body | 応答 |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: サインインしているユーザーの保存済みレイアウト、または `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`。`layout: null` で削除します |

`layout` は、タブ順に並んだ `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }` です。サーバーは CSRF、Screen のアクセス、feature のガードを確認し、dock 自身のパネルだけを（それぞれ一度ずつ）残してサイズを範囲内に収め、アプリのビューストア（`App::datagrid_views`）に、テナント、Screen、ノードをキーとして保存します。ストアがない場合、エンドポイントは `404` を、ユーザーがいない場合は `403` を返し、その場合ブラウザはレイアウトを `localStorage` に保持します。

## 9. AI agents: MCP and WebMCP

外部の AI エージェントは、2 つの入口を通してアプリにアクセスします。どちらもデフォルトでは無効で、`App::mcp` で有効にします。

| 設定 | MCP エンドポイント | WebMCP ツール | 書き込み |
|---|---|---|---|
| `Mcp::off()`（デフォルト） | `404` | `404` | なし |
| `Mcp::read_only()` | 有効 | 有効 | なし |
| `Mcp::on()` | 有効 | 有効 | form の保存と、書き込みを行う API action |

`.no_server()` と `.no_webmcp()` で、片方の入口を無効にできます。`zeyn new` で作られる新しいプロジェクトは `Mcp::read_only()` で始まります。

無効がデフォルトの理由: このエンドポイントでは、有効な `Authorization` ヘッダーを持つ誰もが、モデルを通してアプリを操作できてしまい、モデルは読み込んだデータによって誘導されうるからです。フレームワークはすべてのアプリをエージェント対応にします（ツールはコードから生成され、書くものはありません）が、アプリがその入口を開くかどうか、そしてエージェントに書き込みを許すかどうかは、運用する人々が決めることです。読み取り専用は 1 行で有効にできます。

### 9.1 Tools

ツールは、サインインしているユーザー向けにアプリから作られます。

- grid ごとに 1 つの検索ツール（`{screen}_{node}_search`、入力: `DataQuery`）。書き込みが有効な場合は、params なしで開く、ルーティングされたすべての Screen の form ごとに 1 つの保存ツール（`{screen}_{node}_save`、入力: 入力されたままの form の値）。
- 公開された API action ごとに 1 つのツール（`api_{feature}_{action}`）。入力が `DataQuery` の action は読み取りで、それ以外の API action はすべて書き込みとして扱われます。

説明は、Screen のタイトル、Screen と action の `///` ドキュメントコメントの最初の段落、およびそれらの `ai = "..."` ガイダンスから作られます。ユーザーが使えない grid と form、および `ai = false` が付いた Screen と action は、一覧に出ることはなく、呼び出されても拒否されます。

`#[action(confirm)]` は、ユーザーがアプリ内で承諾した後にのみモデルが実行できる action を示します。`/_zeyn/mcp` はそのようなツールを一覧にも実行もしません（そのチャネルには承諾できる人がいないため）。WebMCP ツールはモーダルダイアログ（`zeyn:confirm`）で確認し、その後 form と一緒に `confirmed: true` を送ります。組み込みのアシスタントはチャットで確認します（`docs/assistant.md` を参照）。

### 9.2 Endpoints

| エンドポイント | 動作 |
|---|---|
| `GET /_zeyn/tools?screen&params` | 表示中の Screen 向けの `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }`。`zeyn.js` がそれらを `document.modelContext` に登録します |
| `POST /_zeyn/mcp` | JSON-RPC: `initialize`、`tools/list`、`tools/call`。`Authorization` ヘッダーとサインインしているユーザーが必要です |
| `via_agent: true` 付きの `POST /_zeyn/form` | WebMCP の form 保存。WebMCP 付きの `Mcp::on()` が必要です。`confirmed: true` はユーザーが承諾した後にのみ |

エージェントの呼び出しはすべてユーザーとして実行されます。Screen のアクセス、ガード、action の権限、インターセプター、レート制限、時間制限が適用されます。エージェントの呼び出しは常に監査され（`via_agent: true`）、マスクされたフィールドは、ユーザーが表示できる場合でも、エージェントに対しては常にマスクされたままです。OpenAPI は各操作に `x-zeyn-ai` を付けます。`ai = false` の場合は `false`、それ以外は `{ hint, confirm, writes }` です。
