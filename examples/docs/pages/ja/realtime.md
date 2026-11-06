# Realtime Channel

Zeyn のリクエストはステートレスです。サーバーはソケットも、ユーザーごとの screen の状態も保持しません。ただし、1 つのダイアグラムを 2 人で編集する、他の人が保存したときに更新されるべき一覧など、それ以上を必要とする画面もあります。`realtime` Cargo feature は、そのためのオプトインの WebSocket チャネルを追加します。有効にしないアプリはこれを一切コンパイルせず、使わないページは何も読み込みません。

WebSocket は企業ネットワークでブロックされることが多いため、このチャネルは改善として扱ってください。どの screen も、これがなくても動作する必要があります（ダイアグラムエディタは action を通じて保存でき、一覧はユーザーが開き直せば更新されます）。

## 1. 有効にする

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
let realtime = Realtime::new()
    .documents("diagram", "diagram.read", "diagram.write", MyDocStore::new(pool))
    .topic("orders", "sales.order.read")
    .limits(Limits::default());
let hub = realtime.hub();          // clone it into stores that notify
App::new(..).realtime(realtime)    // also registers `Hub` as a service
```

## 2. エンドポイント

| エンドポイント | 目的 |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket: 共同編集ドキュメント |
| `GET /_zeyn/live/topics?t=a,b&csrf=TOKEN` | WebSocket: トピックの変更通知 |
| `GET /_zeyn/live.js` | トピック用スクリプト。購読するページだけがリンクします |

すべてのハンドシェイクは、アップグレードの前に次の順で検査されます。

1. チャネルが有効であること（そうでなければ `404`）。
2. **Origin:** `Origin` ヘッダーは、リクエストが届いたホスト、または `Realtime::allow_origin` で追加した origin を指していなければなりません。ブラウザは WebSocket のハンドシェイクで常にこれを送ります。ないものは拒否されます。これが、他のサイトがユーザーの cookie を使ってソケットを開くのを防ぎます。
3. **CSRF トークン:** ブラウザは WebSocket にヘッダーを設定できないため、`meta[name=zeyn-csrf]` のトークンを `csrf` クエリパラメーターに入れ、`POST` に対する `x-zeyn-csrf` とまったく同じように、`zeyn_csrf` cookie と定数時間で比較します。失敗すると `403` を返します。
4. **Identity:** すべてのリクエストと同じ authenticator です。ユーザーはサインインしている必要があります（`403`）。
5. **ユーザーごとの上限**（`429`）。
6. ドキュメント: 宣言済みの kind と単純な id（英字、数字、`-`、`_`、`.` で 1 から 64 文字）でなければ `404`。その kind の読み取り permission が必要です（`403`）。トピック: すべてのトピックが宣言済みで、その permission を持っている必要があります（`403`）。
7. **インターセプター:** アプリのインターセプターが、kind が `Live` の呼び出しで実行されます（`screen` はドキュメントの kind、トピックでは空です）。そのため、`RequireMfa` や停止中アカウントの検査のようなアプリ全体のルールがライブチャネルにも適用されます。拒否された場合は、そのステータスで応答します。
8. ルームの上限: メモリ上のルーム数とルームあたりのクライアント数（`Retry-After` 付きの `503`）。

書き込み permission は接続ごとに一度だけ検査されます。それを持たないクライアントは更新を受信できますが、更新を送信した瞬間に `1008` で切断されます。

## 3. ドキュメントと Y-CRDT

ドキュメントは [Y-CRDT](https://docs.yjs.dev) ドキュメントで、`yrs` で保持されます。ルームは**テナント、kind、ドキュメント id** をキーにするため、id が同じでも 2 つのテナントがルームを共有することはありません。

フレームはバイナリです。最初のバイトがタグで、`1`（`FRAME_UPDATE`）は v1 エンコーディングの Y 更新を運びます。

- 接続時、サーバーはドキュメント全体を 1 つの更新として送信します。
- クライアントは自分の変更の更新を送信します。サーバーはそれぞれを適用し（有効な更新ではないフレームは `1008` でソケットを閉じます）、同じバイト列をルームの他のすべてのクライアントに渡します。
- 各（再）接続のあと、island は自分の状態全体を送り返すため、オフラインで行った編集もマージされます。Y の更新は冪等かつ可換なので、順序や重複は問題になりません。

永続化は trait を通じて行います。

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load` はルームが開くときに実行されます（エラーの場合は `503` で接続を拒否します。ストアが存在するドキュメントを制限する方法でもあります）。`save` はドキュメント全体を 1 つの更新として受け取り、ドキュメントに変更がある間は `persist_every` ごとに、またアイドル状態のルームが破棄される前に実行されます。保存に失敗した場合は次のラウンドで再試行し、ルームは保持されます。`MemoryDocStore` はテストとデモ用です。

island 側では、`realtime` feature を有効にした `zeyn-island` が `live::Channel` を提供します。

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

1 秒後に再接続し、2 倍ずつ増やして最大 30 秒まで待ちます。`close` または `1008` のあとは停止します。

## 4. トピック: その場で更新される screen

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, hub: Hub, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    hub.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live` は `div[data-zeyn-live="orders"]` をレンダリングします。それを含む screen のページは `/_zeyn/live.js` をリンクします（`nav.js` 経由で届く screen には `x-zeyn-live: 1` が付き、そのときスクリプトがインポートされます）。スクリプトは、ページ上のすべてのトピックに対して 1 つのソケットを開きます。通知を受けると、購読している各要素は自分のグリッドをその場で再読み込みします（`zeyn:reload`。グリッドの現在のページ、ソート、フィルターを維持します）。グリッドがない場合は screen 全体をその場で更新します（`zeyn:refresh`）。300 ms 以内の通知は 1 回にまとめて処理されます。

通知にはトピック名だけが含まれ、データは決して含まれません。更新は、すべての permission 検査を伴う通常のグリッドエンドポイントを通じて行われます。`notify` は呼び出し元のテナントにだけ届きます。`notify_tenant(Some(id), topic)` は特定のテナントを指定し、`None` はテナントを使わないアプリを指します。

同じトピック名はチャネルなしでも使えます。`.listens("orders")` を付けたノードは、通知がそのトピックを指定したとき（`live.js` がそれを `zeyn:topics` として渡します）にその場で更新され、action が `ui.refresh("orders")` で終わったときや、`.drives("orders")` を持つフィルターが変わったときにも更新されます。グリッドは現在のページを再読み込みし、ほかのリスナーは画面ごとに 1 回の `POST /_zeyn/nodes` で再レンダリングされます（`examples/docs/pages/effects.md` の Topics を参照）。ソケットが購読するのは、`Realtime::topic` で宣言が必要な `Live` ノードのトピックだけです。グリッドではない部分には、その中か隣で `.listens` を使ってください。

## 5. メモリとバックプレッシャー

すべての状態に上限があります（`Limits`。括弧内が既定値です）。

| 上限 | 既定値 | 到達したとき |
|---|---|---|
| `per_user` ソケット | 8 | ハンドシェイクで `429` |
| `per_room` クライアント | 32 | ハンドシェイクで `503` |
| メモリ上の `rooms` | 256 | ハンドシェイクで `503` |
| クライアントからの `message_bytes` | 64 KB | ソケットを閉じる |
| ドキュメントあたりの `doc_bytes` | 1 MB | 更新を拒否してソケットを閉じる（`1009`） |
| クライアントあたりの `queue` | 32 メッセージ | クライアントをルームから外し、`1013` で閉じる |
| クライアントのいないルームの `idle` | 30 秒 | 永続化してメモリから破棄する |
| `heartbeat` | 20 秒 | ping を送る。3 回分の間隔にわたって無応答のクライアントは閉じる |
| `persist_every` | 2 秒 | 変更のあったドキュメントを保存する |

ブロードキャストは決して待機しません。更新は `try_send` で各クライアントの上限付きキューに渡されます。キューがいっぱいのクライアントは直ちにルームから外されるため、遅い読み取り側が 1 つあってもメモリを占有したり他のクライアントを遅らせたりしません。そのクライアントは再接続してドキュメント全体を受け取ります。キューがいっぱいのトピック購読者は、その通知を逃すだけです（次の通知でどのみち更新されます）。

したがって、最悪の場合のメモリは、ドキュメントでおよそ `rooms * doc_bytes`、キューで `clients * queue * message_bytes` です（更新はキュー間で共有され、コピーされません）。ダイアグラムで 2 つのライブタブを開いた showcase サーバーは、合計で約 19 MB を使用します（デバッグビルド）。`Hub::stats()` は、監視用にルーム、クライアント、購読者、ドキュメントのバイト数を報告します。

## 6. セキュリティ上の注意

- このチャネルは action を実行しません。ドキュメントは、アプリが `DocStore` を通じて読むデータであり、そこで検証されます（showcase は保存されるすべてのダイアグラムを `Diagram::clamped` で制限します）。
- ドキュメントは、テナント内でその kind の読み取り permission を持つ誰もが読めます。読者が異なるドキュメントには、別々の kind（またはストアが拒否する id）を使ってください。
- すべてのページのポリシーは、`default-src 'self'` によってアプリ自身の origin への WebSocket を許可しています。このチャネルのために、Content Security Policy は何も緩和されません。

## 7. 例

`examples/showcase`: Islands の Diagram editor を 2 つのタブで開きます。一方のタブで図形を移動または追加すると、もう一方のタブにも反映されます。ドキュメントが `DocStore` を通じて保存され、`diagram` トピックが通知されると、下にあるグリッド「Shapes stored on the server」も約 2 秒以内にその場で更新されます。

現在のバージョンの制限: ルームは 1 つのプロセス内に存在します（複数のサーバーインスタンスではスティッキーセッションか共有ブローカーが必要です）。プレゼンス（他に誰がいるか、カーソル）は組み込まれていません。ドキュメントは接続のたびに全体が送信されます。
