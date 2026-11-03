# メディア

Zeyn は、HTTP の Range リクエストでメディアファイルを配信し、`Video` と `Audio` コンポーネントで再生します。デコードはネイティブの `<video>` と `<audio>` 要素が行い、Zeyn はその上にアクセシブルな独自のコントロールを描画します。

## ファイルの配信

権限付きで、Feature に `MediaSource` を登録します。

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

ブラウザは `GET /_zeyn/media/{source}/{id}` を取得します。ソースが何かを目にする前に、サーバーは次のことを行います。

1. `A-Z a-z 0-9 - _ .` の 1 から 128 文字でない ID、ドットで始まる ID、`..` を含む ID を拒否します（`404`）。
2. ソースの権限（`403`）と Feature のガードを検査します。
3. インターセプターを実行します（`CallKind::Media`、`screen` にはソース名が入ります）。

`open(ctx, id)` は、アプリ自身のカタログから ID を検索し、`Media` を返します。ディスク上のファイルには `Media::file(path)`（型は拡張子から、エンティティタグはサイズと更新時刻から決まります）、メモリ上のデータには `Media::bytes(data, type)`、任意の `Read + Seek` には `Media::new(reader, len, type)` を使います。ID は名前であり、パスではありません。パスは ID からではなく、自分の検索結果から組み立ててください。

応答:

| リクエスト | 応答 |
|---|---|
| `Range` がない、別の単位、または複数の範囲 | ファイル全体を含む `200` |
| `bytes=a-b`、`bytes=a-`、`bytes=-n` | `Content-Range: bytes a-b/len` を含む `206` |
| 不正な形式、または終端を超えて始まるバイト範囲 | `Content-Range: bytes */len` を含む `416` |
| エンティティタグに一致しない `If-Range` | ファイル全体を含む `200` |
| エンティティタグに一致する `If-None-Match` | `304` |

すべての応答に、`Accept-Ranges: bytes`、`Cache-Control: private, no-cache`、`X-Content-Type-Options: nosniff` が付きます。インラインで配信されるのは、音声、動画、ラスター画像、WebVTT、ストリーミングのプレイリストだけです。それ以外の型（HTML、SVG、スクリプト）は `application/octet-stream` として `Content-Disposition: attachment` で送られるため、ファイルがアプリのページとして実行されることはありません。ファイルはブロッキングプール上で 64 KiB のチャンクで読み取られ、キューに入るのは最大 4 チャンクなので、ファイルのサイズに関係なくメモリは一定に保たれ、離脱したクライアントは読み取りを止めます。レート制限は他のリクエストと同様に適用されます。メディアの読み取りは書き込みではありません。

Content Security Policy を変更する必要はありません。`default-src 'self'` が、アプリからのメディアをすでに許可しているからです。Media Source Extensions を使う適応型ストリーミングライブラリには `media-src` に `blob:` が必要ですが、Zeyn はこれを追加しません。

## 公開ファイル: 画像、ロゴ、ダウンロード

サインインの有無にかかわらず誰でも見てよいファイルは、公開ソースに置きます。

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}` は権限の検査を省略し（ID、ガード、インターセプターは引き続き実行されます）、Cookie を設定せず、`Cache-Control: public, max-age=86400`（最大 `MAX_PUBLIC_MEDIA_CACHE`、1 年）で応答するため、ブラウザとアプリの前段にある CDN がそれを保持します。エンティティタグにより、`304` で再検証することもできます。パスを `Image` の `src` として使ってください。大きな画像が多い場合は、CDN の背後に別の画像ホストを置くほうが、引き続き安価です。それは `App::csp(|c| c.img_src([...]))` で許可します。

- `open` は、すべての訪問者に同じバイト列を返さなければなりません。公開ファイルを、ユーザー、ロール、テナント、言語によって変えてはいけません。CDN やプロキシは最初の応答を保持して、全員に渡します。ユーザーごとに異なるファイルは、`Feature::media` で登録したソースに置いてください。
- 長い `max_age` では、内容に応じて変わる ID（コンテンツハッシュ、たとえば `poster-3f9a2c.jpg`）を付けてください。そうすると、ファイルが変更された場合に新しいアドレスになり、古いコピーが期限切れまでキャッシュに残り続けることがありません。
- 公開ソースの読み取り（`GET` と `HEAD`）は `App::rate_limit` に数えられません。1 つのページが数十枚のポスターを読み込むことがあり、多くのモバイルの訪問者がキャリアの NAT の背後で 1 つのアドレスを共有しているからです。非公開ソースは引き続き数えられ、IP の許可と拒否のルールは両方に引き続き適用されます。帯域の悪用を制限するのは、アプリの前段にある CDN またはリバースプロキシの役割です。

## プレーヤー

```rust
Video::new("films", t!("films.harbor"))
    .subtitle(t!("films.harbor_sub"))
    .file("harbor.webm")
    .file("harbor.mp4")
    .hls("harbor.m3u8")
    .poster("harbor.jpg")
    .captions("harbor.en.vtt", "en", "English")
    .captions("harbor.tr.vtt", "tr", "Türkçe")

Audio::new("music", t!("songs.dawn")).subtitle(t!("songs.artist")).file("dawn.mp3")
```

コントロール: 再生と一時停止、シークスライダー、時間、ミュート、音量スライダー、再生速度メニュー、字幕ボタン、（字幕ファイルが複数ある場合は）言語メニュー、そして動画ではピクチャーインピクチャー（ブラウザが対応している場合）と全画面です。スライダーは WAI-ARIA の slider パターンに従います。矢印キーで 5 秒（または音量の 5 パーセント）動き、Page Up と Page Down で 10 分の 1 動き、Home と End で両端へ移動します。メニューは menu button のパターンに従います。フォーカスがプレーヤー内にある間、`k` または Space で再生と一時停止、`j` と `l` で 10 秒のジャンプ、`m` でミュート、`c` で字幕の切り替え、`f` で全画面の切り替えができます。これらのキーは、ページ上のそれ以外の場所では何もしません。

字幕は、同じソースから配信される WebVTT ファイルです。プレーヤーは、アクティブなキューをプレーンテキストとして描画し（キューのマークアップは取り除かれます）、動画では映像の上に、音声ではタイトルの下に表示するため、字幕はどのブラウザでも全画面でも同じ見え方になります。

## 再開

プレーヤーは、ユーザーが停止した位置を保存します。再生中は 15 秒ごと、一時停止時、ページが非表示になったときまたは閉じられたとき、そしてメディアが終了したときは `0` です。保存された位置が、どちらの端からも 5 秒を超えて離れている場合に、そこから再開します。

- ソースが `position` と `save_position` を実装している場合、位置はサーバーに送られます。`GET /_zeyn/media/position?source=&id=` は `{"at": seconds, "kept": true}` で応答し、`{source, id, at}` を伴う `POST /_zeyn/media/position` は保存します（`204`）。どちらも ID、権限、ガードを検査し、サインインしたユーザーが必要です。POST には CSRF ヘッダーも必要で、レート制限では書き込みとして数えられます。`MAX_POSITION_SECONDS` を超える位置や、ゼロ未満の位置は `422` です。
- それ以外の場合（ソースの既定のメソッド、または匿名ユーザー）、読み取りは `{"at": null, "kept": false}` で応答し、プレーヤーは位置をブラウザのローカルストレージの `zeyn.media.{source}/{id}` に保持します。ストレージのエラー（プライベートウィンドウ、ブロックされたストレージ）は無視されます。

サインインしたユーザーには、サーバー上の位置のほうが適しています。ユーザーに付いてすべてのデバイスへ移動し、アプリは「続きから視聴」の行のためにそれを読み取れます。ローカルストレージはコードが不要ですが、1 つのブラウザにとどまり、それを使うすべての人に共有されます。

## HLS と DASH

`hls(id)` と `dash(id)` は、適応型ストリームを追加します。Safari は HLS をネイティブに再生するため、プレイリストを使います。他のブラウザには、JavaScript ライブラリ（hls.js や dash.js など）が必要ですが、Zeyn はそれを同梱していません。プログレッシブファイルにフォールバックする前に、プレーヤーはそのルート要素でキャンセル可能な `zeyn:media-adaptive` イベントを発行します。

```js
document.addEventListener('zeyn:media-adaptive', (e) => {
  const { media, kind, url } = e.detail;
  if (kind !== 'hls' || !window.Hls?.isSupported()) return;
  e.preventDefault();
  const hls = new window.Hls();
  hls.loadSource(url);
  hls.attachMedia(media);
});
```

そのスクリプトは、アプリのファイルから読み込んでください（インラインスクリプトは Content Security Policy によってブロックされます）。どのリスナーも `preventDefault()` を呼ばない場合、`file` のエントリが順に再生されます。プレイリストとセグメントのファイルは同じソースから配信されるため、その ID がそこに一覧されている限り、相対 URL は機能します。

## アップロードされたファイル

`Form::file` でアップロードされたファイルは、ストアの権限、Feature のガード、テナントの検査の後に、メディアファイルと同じ応答（範囲リクエスト、エンティティタグ、`nosniff`、インライン表示はラスター画像、音声、動画のみ）で、`GET /_zeyn/files/{store}/{id}` から配信されます。`examples/docs/pages/forms.md` のファイルアップロードの節を参照してください。
