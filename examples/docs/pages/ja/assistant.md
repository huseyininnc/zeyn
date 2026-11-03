# 組み込みアシスタント

ステータス: `zeyn` の Cargo feature `assistant` の下で実装済みです。

Zeyn アプリは、独自の AI アシスタントを備えることができます。すべてのページにチャットが用意され、アプリのデータに基づいて回答し、サインインしているユーザーとしてアプリの Screen と API を操作します。Zeyn で作られたアプリは、自社の顧客向けにこれを有効にし、顧客が望むモデルを接続し、チャット履歴を必要な場所に保存できます。

```toml
zeyn = { version = "0.0.2", features = ["assistant"] }
```

feature がなければ、アシスタントに関するものは何もコンパイルされません。HTTP クライアントも、追加のルートも、追加のメモリもありません。

## 1. セットアップ

```rust
use zeyn::assistant::{Assistant, OpenAiCompatible};

let model = OpenAiCompatible::new(config.require("assistant.base_url")?, config.require("assistant.model")?)
    .api_key(config.secret("assistant.api_key")?);

App::new("Lifts")
    .assistant(
        Assistant::new(model)
            .permission("lifts.assistant.use")
            .instructions("You help service engineers plan elevator maintenance.")
            .suggest(work_orders, "Which lifts are overdue for service?")
            .suggest_anywhere("What can you do?"),
    )
```

これだけです。すべてのページにフローティングランチャーが表示され、チャットがサイドパネルとして開きます。Screen がチャットそのものを表示することもできます。

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` は、他のコンテンツの隣に配置できます。`Assistant::launcher(false)` はフローティングランチャーを取り除き、チャットは Screen が配置した場所にだけ表示されます。

| オプション | 既定値 |
|---|---|
| `permission("...")` | サインインしているすべてのユーザー |
| `instructions("...")` | なし。アプリの開発者が書くもので、ユーザーが書くことはない |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | 開始用のチップなし |
| `speech_to_text(p)`, `text_to_speech(p)` | ブラウザーの Web Speech API |
| `no_voice()` | マイクと読み上げがオン |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | アシスタントは確認の後に保存もできる |
| `rate_limit(RateLimit)` | ユーザーごとに 1 分あたり 20 リクエスト |
| `token_limit(Some(RateLimit))` | ユーザーごとに 1 時間あたり 200,000 トークン |
| `limits(Limits { .. })` | セクション 6 を参照 |

## 2. モデル

`LlmProvider` を実装する任意の型を使えます。ストリーミングのラウンドを開始し、`ChatEvent` (テキストの断片、ツール呼び出し、使用量) の有界チャネルを返す 1 つのメソッドです。組み込みのプロバイダーは次のとおりです。

| プロバイダー | 対応先 |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI (`https://api.openai.com/v1`)、Azure OpenAI v1 API (`https://NAME.openai.azure.com/openai/v1`。デプロイをモデルとして指定し、`api-key` ヘッダー用に `.azure()` を使う)、Ollama (`http://localhost:11434/v1`)、vLLM、LiteLLM、その他の互換サーバー。`max_completion_tokens` を知らないサーバーには `.legacy_max_tokens()` を使う |
| `Anthropic::new(model)` | Anthropic Messages API (ゲートウェイには `.base_url(...)` を使う)。推論ブロックは、1 ターンの間は変更されずに送り返される |
| `ScriptedProvider` | テスト、デモ、オフライン開発向け。ネットワーク呼び出しの代わりにルールを使い、単語ごとにストリーミングし、実際のツール呼び出しを行う |

キーは `Secret` の値 (`Config::secret`) から取得され、サーバー上にとどまり、エラーメッセージには決して現れません。プロバイダーのエラーはサーバーでログに記録され、ユーザーには汎用的なメッセージが表示されます。

showcase を実際のモデルに向けるには、設定だけで済みます。

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

または、`NORTHWIND_ASSISTANT__PROVIDER=anthropic` と `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5` を指定するか、キーなしで Ollama の URL を指定します。`NORTHWIND_ASSISTANT__PROVIDER=off` で無効になります。

## 3. 音声

既定では、マイクはブラウザーの音声認識を使い、「読み上げ」はブラウザーの音声合成を使います。サーバーのコストはかからず、音声がアプリのためにブラウザーの外へ出ることもありません。サーバー側の音声を使うには次のようにします。

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

どちらも OpenAI 互換の `/audio/transcriptions` と `/audio/speech` エンドポイントを使います。ローカルの Whisper や音声サーバーもこれらを提供しています。録音には上限があります (ブラウザーで 4 MB、60 秒)。読み上げの音声は、届いた分からブラウザーへストリーミングされます。

## 4. 履歴

`ChatStore` は会話をテナントとユーザー (`ChatKey`) をキーとして保持するため、テナント同士が会話を共有することはありません。

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` が既定です (60 メッセージの会話を 1,000 件。最も長く使われていないものから先に追い出され、再起動で失われます)。データベースを使うには、テナント、ユーザー、位置、JSON としてのメッセージを持つテーブルに対して、この 3 つのメソッドを実装します。メッセージは追記のみで、保持期間はストアが決めます。保留中の確認も履歴の中に保存されるため、サーバーにはそれ以外のユーザーごとの状態はありません。

## 5. ツールと action

モデルが見るツールは、MCP および WebMCP と同じです (`docs/runtime-protocol.md` のセクション 9 を参照)。ツールは毎回、サインインしているユーザー向けに構築されます。ユーザーが使えるグリッドの検索、フォームの保存、API の action から、`ai = false` が付いたものをすべて除いたものが対象で、説明は `///` ドキュメントコメントと `ai = "..."` のガイダンスから取られます。さらに 2 つの組み込みツールが追加されます。

- `navigate`: ユーザーが開けるルート付きの Screen を、ページの読み込みなしで開き (`zeyn:navigate`)、そのグリッドをフィルターできます (`zeyn:filter`)。その後グリッドは `POST /_zeyn/grid` を通じて読み込まれ、サーバーは通常の検査を適用します。
- `suggest`: チップとして表示されるフォローアップのプロンプトです。

すべての呼び出しは、コンテキストにエージェントであることを示すマークを付けたうえで、通常の action の経路を通ります。Screen のアクセス、Feature のガード、action のパーミッション、インターセプター、レート制限、時間制限、監査 (アシスタントの呼び出しはすべて `via_agent: true` で記録されます)、マスキングが適用されます。

**確認。** 読み取りは即座に実行されます。書き込みを行うもの (フォームの保存、入力が `DataQuery` ではない API の action) と、`#[action(confirm)]` が付いたすべての action は、ラウンドを停止し、実行される内容とその引数を示す確認カードを表示します。それを 1 回だけ実行できるのは、ユーザーのクリックだけです (`POST /_zeyn/assistant/confirm`。CSRF 付きで、そのユーザー自身の履歴にある保留中の呼び出しが対象です)。「I confirm」のようなメッセージは単なるテキストです。新しいメッセージが送られると、保留中の呼び出しは未実行として閉じられます。

## 6. セキュリティモデル

| リスク | フレームワークの対応 |
|---|---|
| API キーの漏えい | キーはサーバー上の `Secret` であり、ブラウザーはアプリとだけ通信する |
| モデルがユーザーの権限を超えて動作する | ツールはユーザーのパーミッションから構築され、各呼び出しはサーバーで再度検査される |
| ユーザーが望まない書き込み | 書き込みと `confirm` の action にはユーザーのクリックが必要で、モデルは確認できない |
| マスクされたデータがモデルに届く | マスクされたフィールドは、リビールが許可されているユーザーであっても、エージェントの呼び出しでは常にマスクされる |
| データ経由のプロンプトインジェクション | ツールの結果は `<tool_result>` 区切りの内側のプレーンテキストで、`<` と `>` はエスケープされる。システムプロンプトはそれらがデータであると述べる。システムプロンプトは、固定のルール、アプリの instructions、Screen 名から、ラウンドごとにサーバーが組み立て直し、ユーザー、ページ、ツールのデータからは決して作られない |
| リンクによる情報流出 | 回答は `textContent` で構築される Markdown の小さなサブセットとして描画される。HTML、リンク、画像はない |
| コストの暴走 | ユーザーごとのリクエストのレート制限、ユーザーごとのトークン制限、出力トークンの上限、最大 6 ラウンド、1 ラウンドあたり最大 4 回のツール呼び出し |
| 大きな入力 | メッセージは 2,000 文字まで、1 ラウンドあたり履歴 30 メッセージ、ツールの結果は 6,000 文字で切り詰め、録音は 4 MB まで |
| クロスサイトリクエスト | すべての `POST` に CSRF ヘッダーが必要で、アプリの書き込み制限に計上される |
| 責任の所在が不明な操作 | アシスタントのすべてのツール呼び出しは `via_agent` 付きで監査される |

フレームワークが行わないのは、モデルの回答が正しいかどうかの判断です。ユーザーにそのことを伝え (パネルが伝えます)、重要な action は `confirm` の背後に置いてください。

## 7. 顧客向けの組み込み

アシスタントを自社の顧客に提供するアプリには、通常、次のものが必要です。

- `permission("app.assistant.use")` と、ガードでのライセンスモジュールの検査。料金を支払っているテナントにだけ表示されます。
- テナントごとのモデル。`LlmProvider` を、`ctx` のテナントに基づいて設定から顧客のプロバイダーを選ぶ小さなルーターとして実装するか、全員に 1 つのモデルを使います。
- 顧客の保持ルールに従う、アプリのデータベース上の `ChatStore`。
- ドメインを説明する `instructions(...)` (エレベーター会社なら、ユニット、サイト、点検間隔)、およびそれが必要な action の `ai = "..."` ガイダンス。
- 顧客が支払っているプランに合わせて設定した `token_limit`。

## 8. エンドポイント

| エンドポイント | 動作 |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | `application/x-ndjson` のイベントをストリーミングする |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | 保留中の呼び出しを実行または拒否し、その後チャットと同様にストリーミングする |
| `POST /_zeyn/assistant/clear` | 会話を削除する。`204` |
| `POST /_zeyn/assistant/stt` (音声ボディ) | `{ text }`。サーバー側の音声認識がない場合は `404` |
| `POST /_zeyn/assistant/tts` `{ text }` | ストリーミングされる音声。サーバー側の音声合成がない場合は `404` |

イベントは 1 行に 1 つの JSON オブジェクトです。`{t:"text",v}`、`{t:"tool",name,id}`、`{t:"navigate",url,screen,filters}`、`{t:"suggest",v}`、`{t:"confirm",id,tool,title,question,details}`、`{t:"resolved",id,approved}`、`{t:"error",message}`、`{t:"done"}`。

`assistant.js` と `assistant.css` は、アシスタントを使えるユーザーにだけ読み込まれます。チャットのログは `role="log"` のライブリージョンで (回答のストリーミング中は `aria-busy`)、Enter で送信し、Shift+Enter で改行し、Escape でサイドパネルを閉じてフォーカスをランチャーに戻します。確認カードでは、最初にキャンセルボタンにフォーカスが置かれます。

## 9. コスト

アシスタントは、上限のある履歴ストアと処理中のストリームを除いて、ユーザーごとの状態をメモリに保持しません。回答は、32 項目のチャネルを通じてエンドツーエンドでストリーミングされます。showcase での測定結果 (デバッグビルド、Windows、ページを 5 回読み込んだ後のアイドル状態): アシスタントがオフの場合はワーキングセットが 14.4 MB、プライベートメモリが 3.13 MB、オンの場合は 14.5 MB と 3.19 MB でした (約 100 KB の増加)。
