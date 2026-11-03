# 내장 어시스턴트

상태: `zeyn`의 `assistant` Cargo 기능 뒤에 구현되어 있습니다.

Zeyn 앱은 자체 AI 어시스턴트를 담을 수 있습니다. 모든 페이지에 있는 채팅이 앱의 데이터로 응답하고, 로그인한 사용자로서 앱의 Screen과 API를 다룹니다. Zeyn으로 만든 앱은 자신의 고객을 위해 이를 켜고, 고객이 원하는 모델을 연결하고, 필요한 곳에 채팅 기록을 보관할 수 있습니다.

```toml
zeyn = { version = "0.0.2", features = ["assistant"] }
```

기능이 없으면 이와 관련된 것은 컴파일되지 않습니다. HTTP 클라이언트도, 추가 경로도, 추가 메모리도 없습니다.

## 1. 설정

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

이것이 전부입니다. 모든 페이지에 채팅을 측면 패널로 여는 떠 있는 런처가 생깁니다. Screen이 채팅을 직접 표시할 수도 있습니다.

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()`은 다른 콘텐츠 옆에 들어갑니다. `Assistant::launcher(false)`는 떠 있는 런처를 없애므로 채팅은 Screen이 배치한 곳에만 나타납니다.

| 옵션 | 기본값 |
|---|---|
| `permission("...")` | 로그인한 모든 사용자 |
| `instructions("...")` | 없음. 앱 개발자가 작성하며 사용자는 절대 작성하지 않습니다 |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | 시작 칩 없음 |
| `speech_to_text(p)`, `text_to_speech(p)` | 브라우저의 Web Speech API |
| `no_voice()` | 마이크와 소리 내어 읽기가 켜져 있음 |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | 어시스턴트가 확인 후에 저장도 할 수 있음 |
| `rate_limit(RateLimit)` | 사용자당 분당 요청 20개 |
| `token_limit(Some(RateLimit))` | 사용자당 시간당 토큰 200,000개 |
| `limits(Limits { .. })` | 6절 참고 |

## 2. 모델

`LlmProvider`를 구현하는 모든 타입이 동작합니다. 스트리밍 라운드를 시작하고 `ChatEvent`(텍스트 조각, 도구 호출, 사용량)의 제한된 채널을 반환하는 메서드 하나입니다. 내장된 것은 다음과 같습니다.

| 제공자 | 지원 대상 |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI(`https://api.openai.com/v1`), Azure OpenAI v1 API(`https://NAME.openai.azure.com/openai/v1`, 배포를 모델로 지정, `api-key` 헤더에는 `.azure()`), Ollama(`http://localhost:11434/v1`), vLLM, LiteLLM, 그 밖의 호환 서버. `max_completion_tokens`를 모르는 서버에는 `.legacy_max_tokens()` |
| `Anthropic::new(model)` | Anthropic Messages API (게이트웨이에는 `.base_url(...)`). 추론 블록은 한 턴 안에서 변경 없이 다시 전송됩니다 |
| `ScriptedProvider` | 테스트, 데모, 오프라인 개발: 네트워크 호출 대신 규칙을 사용하며, 단어 단위로 스트리밍하고, 실제 도구 호출을 합니다 |

키는 `Secret` 값(`Config::secret`)에서 가져오고, 서버에 남으며, 오류 메시지에 절대 나타나지 않습니다. 제공자 오류는 서버에 기록되며 사용자에게는 일반적인 메시지가 보입니다.

쇼케이스를 실제 모델로 연결하는 데는 설정만 필요합니다.

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

또는 `NORTHWIND_ASSISTANT__PROVIDER=anthropic`과 `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5`, 또는 키가 없는 Ollama URL을 사용합니다. `NORTHWIND_ASSISTANT__PROVIDER=off`는 이를 끕니다.

## 3. 음성

기본적으로 마이크는 브라우저의 음성 인식을, "소리 내어 읽기"는 브라우저의 음성 합성을 사용합니다. 서버 비용이 없고 앱을 위해 브라우저를 떠나는 오디오도 없습니다. 서버 음성을 사용하려면 다음과 같이 합니다.

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

둘 다 OpenAI 호환 `/audio/transcriptions`와 `/audio/speech` 엔드포인트를 사용하며, 로컬 Whisper와 음성 서버도 이를 제공합니다. 녹음에는 상한이 있습니다(브라우저에서 4 MB, 60초). 음성 오디오는 도착하는 대로 브라우저로 스트리밍됩니다.

## 4. 기록

`ChatStore`는 테넌트와 사용자(`ChatKey`)를 키로 대화를 보관하므로 테넌트가 서로 대화를 공유하지 않습니다.

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)`가 기본값입니다(60개 메시지의 대화 1,000개, 가장 오래 사용하지 않은 것부터 제거, 재시작하면 사라짐). 데이터베이스에는 테넌트, 사용자, 위치, JSON 형태의 메시지가 있는 테이블 위에 세 메서드를 구현하십시오. 메시지는 추가만 가능하며 보존은 스토어가 결정합니다. 대기 중인 확인도 기록에 있으므로 서버는 그 밖의 사용자별 상태를 보관하지 않습니다.

## 5. 도구와 action

모델은 MCP 및 WebMCP와 같은 도구를 봅니다(`docs/runtime-protocol.md` 9절 참고). 도구는 매번 로그인한 사용자를 위해 만들어집니다. 사용자가 사용할 수 있는 그리드 검색, 폼 저장, API action에서 `ai = false`로 표시된 모든 것을 뺀 것이며, 설명은 `///` 문서 주석과 `ai = "..."` 안내에서 가져옵니다. 두 가지 내장 도구가 추가됩니다.

- `navigate`: 사용자가 열 수 있는 경로 Screen을 페이지 로드 없이 열며(`zeyn:navigate`) 그 그리드를 필터링할 수 있습니다(`zeyn:filter`). 그러면 그리드는 `POST /_zeyn/grid`를 통해 불러오고, 서버는 평소의 검사를 적용합니다.
- `suggest`: 칩으로 표시되는 후속 프롬프트.

모든 호출은 컨텍스트가 에이전트로 표시된 채 일반 action 경로를 거칩니다. Screen의 접근 권한, Feature의 가드, action의 권한, 인터셉터, 속도 제한, 시간 제한, 감사(모든 어시스턴트 호출은 `via_agent: true`로 기록됨), 마스킹이 적용됩니다.

**확인.** 읽기는 즉시 실행됩니다. 쓰는 모든 것(폼 저장, 입력이 `DataQuery`가 아닌 API action)과 `#[action(confirm)]`으로 표시된 모든 action은 라운드를 멈추고 무엇이 실행될지와 그 인수를 보여 주는 확인 카드를 표시합니다. 사용자의 클릭(그 사용자 본인의 기록에 대기 중인 호출에 대한 CSRF가 있는 `POST /_zeyn/assistant/confirm`)만이 이를 한 번 실행합니다. "확인합니다" 같은 메시지는 그냥 텍스트이며, 새 메시지는 대기 중인 호출을 실행되지 않은 것으로 닫습니다.

## 6. 보안 모델

| 위험 | 프레임워크가 하는 일 |
|---|---|
| API 키 유출 | 키는 서버의 `Secret`이며, 브라우저는 앱하고만 통신합니다 |
| 사용자의 범위를 넘어서는 모델 | 도구는 사용자의 권한으로 만들어지며, 각 호출은 서버에서 다시 검사됩니다 |
| 사용자가 원하지 않은 쓰기 | 쓰기와 `confirm` action에는 사용자의 클릭이 필요하며, 모델은 확인할 수 없습니다 |
| 마스킹된 데이터가 모델에 도달 | 마스킹된 필드는 공개할 수 있는 사용자에게도 에이전트 호출에서는 항상 마스킹됩니다 |
| 데이터를 통한 프롬프트 인젝션 | 도구 결과는 `<`와 `>`가 이스케이프된 `<tool_result>` 구분자 안의 일반 텍스트이며, 시스템 프롬프트는 이것이 데이터라고 알려 줍니다. 시스템 프롬프트는 서버가 매 라운드마다 고정된 규칙, 앱의 지침, Screen 이름으로 다시 만들며, 사용자, 페이지, 도구 데이터로 만들지 않습니다 |
| 링크를 통한 유출 | 답변은 `textContent`로 만든 작은 Markdown 부분 집합으로 렌더링됩니다. HTML도 링크도 이미지도 없습니다 |
| 통제되지 않는 비용 | 사용자별 요청 속도 제한, 사용자별 토큰 제한, 출력 토큰 상한, 최대 6라운드와 라운드당 도구 호출 4개 |
| 큰 입력 | 메시지는 최대 2,000자, 라운드당 기록 메시지 30개, 도구 결과는 6,000자로 자름, 녹음은 최대 4 MB |
| 교차 사이트 요청 | 모든 `POST`에는 CSRF 헤더가 필요하며 앱의 쓰기 제한에 포함됩니다 |
| 책임 추적이 안 되는 action | 모든 어시스턴트 도구 호출은 `via_agent`와 함께 감사됩니다 |

하지 않는 일: 모델의 답변이 맞는지 판단하는 것. 사용자에게 이를 알리고(패널이 알립니다) 중요한 action은 `confirm` 뒤에 두십시오.

## 7. 고객을 위한 임베딩

어시스턴트를 자신의 고객에게 판매하는 앱은 보통 다음을 원합니다.

- 비용을 지불하는 테넌트만 볼 수 있도록 `permission("app.assistant.use")`와 가드의 라이선스 모듈 검사.
- 테넌트별 모델: `LlmProvider`를 `ctx` 테넌트로 설정에서 고객의 제공자를 고르는 작은 라우터로 구현하거나, 모두를 위한 모델 하나를 실행합니다.
- 고객의 보존 규칙이 적용된, 앱의 데이터베이스 기반 `ChatStore`.
- 도메인(엘리베이터 회사라면 설비, 현장, 점검 주기)을 설명하는 `instructions(...)`와, 필요한 action의 `ai = "..."` 안내.
- 고객이 지불하는 요금제에 맞게 설정한 `token_limit`.

## 8. 엔드포인트

| 엔드포인트 | 기능 |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | `application/x-ndjson` 이벤트를 스트리밍합니다 |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | 대기 중인 호출을 실행하거나 거절한 다음 채팅처럼 스트리밍합니다 |
| `POST /_zeyn/assistant/clear` | 대화를 삭제합니다. `204` |
| `POST /_zeyn/assistant/stt` (오디오 본문) | `{ text }`. 서버 음성 텍스트 변환이 없으면 `404` |
| `POST /_zeyn/assistant/tts` `{ text }` | 스트리밍 오디오. 서버 텍스트 음성 변환이 없으면 `404` |

이벤트는 한 줄에 JSON 객체 하나입니다. `{t:"text",v}`, `{t:"tool",name,id}`, `{t:"navigate",url,screen,filters}`, `{t:"suggest",v}`, `{t:"confirm",id,tool,title,question,details}`, `{t:"resolved",id,approved}`, `{t:"error",message}`, `{t:"done"}`.

`assistant.js`와 `assistant.css`는 어시스턴트를 사용할 수 있는 사용자에게만 로드됩니다. 채팅 로그는 `role="log"` 라이브 리전이며(답변이 스트리밍되는 동안 `aria-busy`), Enter는 전송하고, Shift+Enter는 줄을 바꾸며, Escape는 측면 패널을 닫고 포커스를 런처로 돌려보내며, 확인 카드는 먼저 취소에 포커스를 둡니다.

## 9. 비용

어시스턴트는 제한된 기록 스토어와 진행 중인 스트림 외에는 사용자별 상태를 메모리에 보관하지 않습니다. 답변은 32개 항목의 채널을 통해 끝에서 끝까지 스트리밍됩니다. 쇼케이스(디버그 빌드, Windows, 페이지를 다섯 번 로드한 후 유휴)에서 측정한 결과, 어시스턴트가 꺼져 있으면 작업 집합 14.4 MB와 전용 메모리 3.13 MB, 켜져 있으면 14.5 MB와 3.19 MB입니다(약 100 KB 증가).
