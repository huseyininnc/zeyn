# WASM Islands

Zeyn はページをサーバーでレンダリングします。キャンバスエディタ、ダイアグラム、重いチャートなど、それではインタラクティブ性が足りない画面もあります。**island** は WebAssembly にコンパイルされた Rust コンポーネントで、サーバーがそれ以外はサーバーレンダリングのページに配置します。周囲はすべてふつうの HTML のままです。

island は `zeyn` の Cargo feature `islands` の背後にあります。有効にしないアプリはこれらを一切コンパイルせず、有効にしても island を含まないページには何のコストもかかりません（後述の Performance を参照）。

## 1. 構成要素

| 要素 | 場所 | 役割 |
|---|---|---|
| `zeyn::ui::Island` | サーバー | プレースホルダー node: props、フォールバックの内容、バインドされた action |
| `App::islands(dir)` | サーバー | ビルド済みファイルとローダーを配信します |
| `zeyn-island` | 共有とブラウザ | `Remote`（呼び出し可能な action の契約）、`Island::call`（クライアント） |
| 共有クレート | 両方 | props、メッセージ、remote を素の serde 型として定義します |
| island クレート | ブラウザ | `mount(root)` と、必要に応じて `unmount(root)` をエクスポートする `cdylib` |
| `zeyn build-islands` | CLI | island クレートをハッシュ付きファイルとマニフェストにビルドします |

`examples/showcase` のような典型的な構成です。

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. 共有型

共有クレートは `serde` と、feature なしの `zeyn-island` に依存します。そのためサーバー側にブラウザ用コードはコンパイルされません。remote は island が呼び出せる action の名前を示し、その型を固定します。

```rust
#[derive(Serialize, Deserialize)]
pub struct Count { pub value: i64 }

pub struct SaveCount;

impl zeyn_island::Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
```

## 3. サーバー側

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>` は、action の入力が `R::Input` で出力が `Result<R::Output>` のときだけコンパイルできます。このため両側の型がずれることはありません。island が呼び出せるのは、その node がバインドした action だけです。

`fallback` には任意の node を渡せます。テキストの要約や読み取り専用のグリッドなどです。island が読み込まれる前、JavaScript が無効なとき、読み込みに失敗したときに表示されます。その中のグリッドなどのコンポーネントは通常どおり動作します。

読み込み: `.on_interaction()` はプレースホルダーへの最初のポインター、タッチ、フォーカスを待ちます。`.on_idle()` はブラウザがアイドル状態のときに読み込みます。既定では、プレースホルダーがビューポートから 200 px 以内に入ったときに読み込みます。読み込みがページをブロックすることはありません。

出力フォルダーは一度だけ登録します: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`。そこにマニフェストがない場合、サーバーは警告をログに出し、island はフォールバックを表示します。

## 4. island 側

```rust
#[wasm_bindgen]
pub fn mount(root: web_sys::Element) -> Result<(), JsValue> {
    let island = zeyn_island::Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    // build DOM under root, attach listeners
    zeyn_island::spawn(async move {
        match island.call::<SaveCount>(&Count { value: 3 }).await {
            Ok(saved) => { /* ... */ }
            Err(e) => { /* e.message is localized by the server */ }
        }
    });
    Ok(())
}
```

`Island::call` は `POST /_zeyn/data` に、island の screen、params、node パス、`key` としての remote 名、そして入力を送ります。ページの CSRF トークンを `x-zeyn-csrf` に、セッション cookie とともに付けて送信します。サーバーは、他のすべてのコンポーネントと同様に次を順に検査します。CSRF、screen のアクセス、feature のガード、action の permission。そのあとでインターセプターを実行し、時間制限付きで action を実行します。これ以外の入口はありません。island は、その node がバインドしていない action には到達できず、island に関するどんなものも検査を迂回しません。

エラーは `CallError { status, code, message }` として返ります。`status` は、ローカルでの失敗（`network`、`props`、`input`、`answer`）では `0` です。

island がタイマー、アニメーションフレーム、ソケットを保持する場合は `unmount(root)` をエクスポートしてください。ローダーは、プレースホルダーがドキュメントから外れたとき（ナビゲーション、閉じられた window）にこれを呼び出します。

翻訳: island が表示するテキストは、サーバーがすでにローカライズした状態で props に渡します。これにより island はカタログを持ち運びません。

## 5. ビルド

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

各クレートについて、次を実行します。

1. `cargo build --lib --release --target wasm32-unknown-unknown`。サイズ設定は `--config` で渡します（`opt-level = "z"`、`lto = true`、`codegen-units = 1`、`panic = "abort"`、デバッグ情報なし）。そのためアプリ自身の release プロファイルは変更されません。
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section`（`wasm-bindgen-cli` のバージョンは `wasm-bindgen` クレートのバージョン `0.2.130` と一致している必要があります）。
3. binaryen が `PATH` にある場合は `wasm-opt -Oz`。
4. `{name}.{hash}.js` と `{name}_bg.{hash}.wasm` を書き出し（ハッシュは内容の 64 ビット FNV ハッシュ）、`manifest.json` を更新します。

`zeyn dev` は、アプリに `islands/` フォルダーがある場合、各アプリのビルドの前に同じ処理を行います。`dist/` は監視しません。

サーバーは、マニフェストに記載されたファイル名だけを `/_zeyn/islands/{file}` で配信します。`Content-Type` は `application/wasm` または `text/javascript`、`Cache-Control: public, max-age=31536000, immutable` を付けます。ファイルはリクエストごとにディスクから読み込まれるため、サーバーのメモリを消費しません。

## 6. Content Security Policy

WebAssembly のコンパイルには、`script-src` に `'wasm-unsafe-eval'` が必要です。Zeyn はこれを、screen に island を含むページ**だけ**に追加します。それ以外のすべてのページは、`docs/runtime-protocol.md` の厳格なポリシーを維持します。`'wasm-unsafe-eval'` が許可するのは `WebAssembly.compile` と `instantiate` だけです。`eval`、`new Function`、インラインスクリプトはブロックされたままで、モジュールもアプリの origin からしか読み込まれません。

ポリシーはドキュメントに属し、あとから緩和することはできません。そのため、`nav.js` が island を含む screen（部分応答に `x-zeyn-islands: 1` が付きます）を、island ポリシーなしで読み込まれたドキュメントに読み込む場合、フルページロードにフォールバックします。一度ドキュメントが island ポリシーを持てば、island のある screen とない screen の間の移動はその場で行われます。

サーバーは、属性 `data-zeyn-island="` によって island を見つけます。エスケープされたユーザーデータは引用符を生成できないため、ユーザーが入力したテキストでポリシーを有効にすることはできません。

## 7. Canvas surface

`zeyn::ui::Canvas::new(name, description)` は描画用の island です。description は必須です。キャンバスは支援技術からは見えないため、ページが何を表示しているか、どのキーで操作するかを説明する必要があります。サーバーはこれを island の上に段落としてレンダリングし、プレースホルダーの `aria-describedby` でそれを指します。

island 側では、`canvas` feature を有効にした `zeyn-island` が `canvas::Surface` を提供します。

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| 関心事 | surface の動作 |
|---|---|
| Graphics | `Prefer::Gl` は WebGL 2 を試し、2D にフォールバックします。`webgpu_available()` は、`surface.canvas()` 上で自分で WebGPU をセットアップするシーン向けに、`navigator.gpu` が存在するかどうかを返します |
| High DPI | バッキングストアは `devicePixelRatio` と要素サイズ（`ResizeObserver`）に追従します。2D 描画は事前にスケールされているため、シーンは CSS ピクセルで動作します |
| Input | ポインターの down、move、up（pointer capture 付き）、wheel（行とページの delta をピクセルに変換）、キーを、CSS ピクセル単位の `Input` 値として渡します |
| Frames | `requestAnimationFrame` は、シーンが使った入力のあと、`update` のあと、リサイズのあと、または `Scene::animating` が `true` を返す間だけ実行されます。静止した画面は CPU を消費しません |
| Defaults | シーンが使う入力は既定の動作が抑止されます（矢印キーでページはスクロールしません）。Tab は `false` を返して、フォーカスが外へ出られるようにします |
| Clean up | `destroy()` はリスナー、オブザーバー、キャンバスを削除します。`unmount` から呼び出してください |

キャンバスには `tabindex="0"`、`role="application"`、ラベル、`aria-describedby` が付きます。

アクセシビリティのパターンは、showcase のダイアグラムエディタ（`examples/showcase/islands/diagram`）にあるとおりです。

- **キャンバス上のキーボード:** `]` と `[` で図形を選択し、矢印キーで移動し、Delete で削除し、`+`、`-`、`0` でズームし、Escape で選択を解除します。キーはキャンバスにフォーカスがあるときだけ動作します。
- **通常のコントロール:** ボタンのツールバーが、キャンバスなしで同じ操作を行います。
- **テキストの要約:** キャンバスの下のリストがすべての図形を説明し、変更のたびに更新されます。`role="status"` の行が、選択と保存の結果を通知します。
- **フォールバック:** サーバーが同じ要約をレンダリングするため、WebAssembly を使えないユーザーもダイアグラムを読めます。

エディタは `SaveDiagram` を通じて保存します。island は決して信頼されないため、サーバーは受け取った内容を制限します（`Diagram::clamped`: 図形は最大 200 個、id は一意、ラベルは 40 文字、座標は範囲内）。

### 7.1 プロパティインスペクター

`PropertyInspector::island(name)` は選択したオブジェクトを編集します。アイランドは自分の任意の要素から `detail: { title, schema, values }` 付きの `zeyn:inspect` を送り、インスペクターは確定した変更ごとに、検証済みの `zeyn:property`（`detail: { name, value }`）をアイランドのルートで返します。アイランドは保存する内容を自分でも検証し、サーバーは保存を `FormSchema::check` で検証します。

## 8. Performance

| 項目 | 数値 |
|---|---|
| 通常のページの HTML、リクエスト、ポリシー | 変更なし: meta なし、ローダーなし、追加リクエストなし |
| `nav.js` | +281 バイト（フルロードのフォールバック） |
| `/_zeyn/islands.js` ローダー、island のあるページのみ | 3.9 KB、gzip で 1.7 KB |
| カウンター island の `.wasm`（wasm-opt なし） | 127.6 KB、gzip で 52.8 KB |
| カウンター island のグルー `.js` | 18.0 KB、gzip で 4.2 KB |
| ダイアグラムエディタの `.wasm`（canvas surface、wasm-opt なし） | 169.4 KB、gzip で 67.8 KB |
| ライブ編集付きのダイアグラムエディタ（`yrs` を追加、`docs/realtime.md` を参照） | 408.4 KB、gzip で 144.3 KB |
| ダイアグラムエディタのグルー `.js` | 30.3 KB、gzip で 6.1 KB |
| island あたりのサーバーメモリ | なし: ファイルはディスクからストリームされ、props はリクエストごとにレンダリングされます |

カウンターのサイズの大部分は `serde_json` と `core` のフォーマット処理が占めています。これらを共有する island は、それぞれが 1 回ずつ負担します。wasm-opt を使うと、通常はさらに 10 から 20 パーセント小さくなります。

## 9. 制限

- window（`open_window`）の中の island は、開く側のページが island ポリシーを持っている必要があります。island を含む window は island のあるページに置くか、screen として開いてください。
- `Closure::forget` でアタッチしたクロージャはページが存在する間ずっと残ります。island が何度もマウントされうる場合は、`unmount` で解放してください。
- island 自体はサーバー上では何もレンダリングしません。検索エンジン、読み込み前のスクリーンリーダー、JavaScript のないユーザーが受け取るのはフォールバックです。意味のある内容にしてください。
