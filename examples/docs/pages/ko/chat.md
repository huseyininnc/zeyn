# 채팅

대화를 위한 컴포넌트입니다. 대화의 메시지, 사용자가 글을 쓰는 상자, 누군가 입력 중이라는 표시가 있습니다. 각각은 서버에서 렌더링되며 스타일시트와 스크립트를 그것을 쓰는 페이지에서만 불러옵니다.

## MessageThread

`MessageThread`는 한 대화의 메시지를 채팅 말풍선으로 보여 줍니다. 사용자 자신의 메시지는 끝 쪽에, 다른 사람의 메시지는 시작 쪽에 놓이고(오른쪽에서 왼쪽으로 쓰는 언어에서는 좌우가 뒤집힘), 한 발신자가 5분 안에 연달아 보낸 메시지는 하나의 이름 아래 묶이며, 사용자의 시간대 기준으로 날마다 제목이 붙고, 각 메시지의 시각과 자신의 메시지에는 전달 상태가 표시됩니다.

```rust
#[action(permission = "chat.read")]
pub async fn conversation_messages(q: ThreadQuery, store: Chats, ctx: Ctx) -> Result<Page<Msg>> {
    store.check_member(&q.conversation, &ctx)?;
    Ok(q.query.apply(store.messages(&q.conversation)))
}

MessageThread::of(conversation_messages)
    .conversation(params.id.to_string())
    .author(Msg::author)
    .name(Msg::author_name)
    .text(Msg::body)
    .when(Msg::sent_at)
    .status(Msg::status)
    .attachments(Msg::files, CHAT_FILES)
    .live()
```

기록 action은 `ThreadQuery`를 받습니다. 서버가 컴포넌트에서 가져오는 대화(브라우저는 이를 절대 보내지 않음)와 최신순으로 정렬된 `DataQuery`입니다. 사용자가 그 대화의 참여자인지 action 안에서 확인하십시오. `.author`는 발신자의 사용자 id로, 로그인한 사용자와 비교해 자신의 메시지와 다른 사람의 메시지를 구분합니다. `.name`은 표시되는 이름입니다. 모델에는 `#[key]`가 필요하며, 그래서 두 번 불러온 메시지도 한 번만 표시됩니다.

처음 50개 메시지(`THREAD_PAGE`)는 화면과 함께 렌더링됩니다. 위로 스크롤하면 다음의 더 오래된 페이지를 `POST /_zeyn/grid`로 불러오며, 매번 화면 접근 권한과 action 권한을 확인하고, 독자가 보고 있는 메시지는 제자리에 유지됩니다. 페이지에는 최대 200개 메시지만 남습니다. 더 나아가면 먼 쪽 끝을 버리고, 독자가 돌아오면 다시 불러옵니다. 따라서 수천 개 메시지가 있는 대화도 짧은 대화와 같은 메모리를 씁니다. 5,000개 메시지를 스크롤할 때 페이지에는 약 2,000개 요소가 있고 디스플레이의 프레임 속도로 동작합니다.

`Delivery`에는 `Sending`, `Sent`, `Delivered`, `Read`(읽음 확인), `Failed` 상태가 있으며, 각각 아이콘과 스크린 리더용 텍스트로 표시됩니다. `.attachments(field, rules)`는 메시지의 파일을 보여 줍니다(`Vec<File>` 필드와 `Feature::files`로 등록한 저장소의 규칙). 이미지는 자리 표시자 위에 지연 로딩되는 고정 크기 상자에, 다른 파일은 저장소의 읽기 권한을 거치는 다운로드 링크로 표시됩니다.

새 메시지는 스레드의 topic 중 하나가 호출될 때 가져옵니다. `.topic(..)`으로 따로 지정하지 않으면 `"chat"`입니다. `Composer`가 보낸 뒤, action이 `ui.refresh("chat")`으로 끝난 뒤, 또는 `.live()`와 `realtime` feature가 있을 때 어디서든 `hub.notify(&ctx, "chat")`이 실행될 때입니다. 다른 사람의 메시지는 polite 라이브 영역에서 알려 줍니다.

### AI 답장

```rust
#[action(permission = "chat.ai")]
pub async fn ask_model(r: ReplyRequest, store: Chats) -> Result<Reply> {
    let id = store.reserve_id();
    let (tx, reply) = Reply::channel(id.to_string());
    tokio::spawn(async move {
        let mut text = String::new();
        for piece in my_model_stream(&r.text).await {
            text.push_str(&piece);
            if !tx.send(piece).await {
                break;
            }
        }
        store.save_ai_answer(id, &r.conversation, text);
    });
    Ok(reply)
}

MessageThread::of(conversation_messages)
    .conversation(id)
    .ai(Msg::from_ai)
    .reply(ask_model)
```

`.reply(action)`을 쓰면 같은 대화의 `Composer`로 보낸 메시지가 AI 답장을 시작합니다. 브라우저는 텍스트를 `POST /_zeyn/stream`에 보내고(CSRF, 쓰기 제한, 화면 접근 권한, action 권한, 인터셉터), 답장은 내장 어시스턴트와 같은 형식의 JSON 줄로 스트리밍되어 돌아옵니다. 조각들은 크기가 제한된 채널(`REPLY_BUFFER`)을 지나갑니다. 서버는 `MAX_REPLY_CHARS`(32,000)자 또는 `REPLY_SECONDS`(120)초 뒤에 멈추고, 중지 버튼으로 답장을 일찍 끝낼 수 있습니다. 독자가 떠나면 `ReplySender::send`가 `false`를 돌려주며, 이것이 생성기가 멈춰야 한다는 신호입니다.

답장은 텍스트 노드만으로 만든 작은 Markdown 부분 집합으로 표시되므로, 모델이 쓴 어떤 내용도 HTML로 해석되지 않습니다. 이전 메시지는 다시 렌더링되지 않고, 지금 쓰이고 있는 문단만 렌더링됩니다. 2,000개 조각으로 된 답장도 끝에서의 프레임당 비용이 처음과 같고, 위에 있는 내용은 움직이지 않습니다. 라이브 영역은 답장이 끝났을 때 한 번만 읽고, 조각마다 읽지 않습니다. `.ai(field)`로 표시한 메시지도 기록에서 안전한 Markdown으로 불러옵니다(`markdown` feature가 있을 때. 없으면 일반 텍스트).

답장의 키(`Reply::channel(key)`)는 저장된 메시지가 갖게 될 키이므로, 기록은 두 번째 사본 대신 저장된 답변을 그 자리에 보여 줍니다.

## Composer

`Composer`는 사용자가 메시지를 쓰는 상자입니다. 텍스트에 맞춰 최대 높이까지 늘어나는 텍스트 영역, 보내기 버튼, 그리고 요청하면 첨부, 멘션, 유지되는 임시 저장본을 제공합니다.

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, hub: Hub, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    hub.notify(&ctx, "chat");
    Ok(())
}

pub const CHAT_DRAFTS: ClientPref<Draft> =
    ClientPref::draft("chat-draft", Duration::from_secs(7 * 86_400), Purpose::Preference);

Composer::of(send_message)
    .conversation(params.id.to_string())
    .attachments(CHAT_FILES)
    .mentions(search_people)
    .remember(CHAT_DRAFTS)
```

보내기 action은 `ChatSend`를 받습니다. 서버가 컴포넌트에서 가져오는 대화, 앞뒤 공백을 제거한 텍스트, 첨부 파일, 멘션된 사람들의 id입니다. 보내기는 CSRF 토큰과 함께 `POST /_zeyn/data`를 거치고, 쓰기 제한에 계산되며, action 권한이 필요합니다. 사용자가 보낼 수 없는 composer는 페이지에서 빠집니다. 서버는 빈 메시지, `.max_chars(n)`(지정하지 않으면 4,000)보다 긴 메시지, 저장소 규칙이 허용하는 것보다 많은 파일, 형식이 잘못된 파일 id, 20개를 넘는 멘션을 거부합니다. 나머지도 신뢰할 수 없는 것으로 다루십시오. 사용자가 대화에 참여하고 있고 멘션된 사람을 지정해도 되는지 확인하십시오.

기본값인 `SendKey::Enter`는 Enter로 보내고 Shift+Enter로 줄을 바꿉니다. `.send_key(SendKey::CtrlEnter)`는 Ctrl+Enter(Mac에서는 Cmd+Enter)로 보내고 Enter는 줄 바꿈에 남겨 둡니다. 메시지가 비어 있거나 너무 길면 보내기 버튼이 비활성화되고, 한도에 가까워지면 카운터가 나타납니다. 보낸 뒤 composer는 자신의 topic을 호출합니다(`.topic(..)`으로 따로 지정하지 않으면 `"chat"`). 그래서 같은 대화의 `MessageThread`가 메시지를 곧바로 보여 주고, `.reply(..)`가 있으면 AI 답장을 스트리밍합니다.

`.attachments(rules)`는 `Feature::files`로 등록한 저장소의 업로드 필드를 추가합니다. 파일은 진행 표시줄과 함께 하나씩 `POST /_zeyn/files/{store}`에 업로드되고, 저장소의 쓰기 권한과 형식 검사를 거치며, 그 id가 메시지와 함께 전달됩니다.

`.mentions(action)`은 `@` 뒤에서 lookup action으로 사람을 검색합니다(입력은 `LookupQuery`, 출력은 `LookupItem` 페이지). 검색은 마지막 키 입력 200 ms 뒤에 시작되고, 새 검색은 이전 검색을 취소하며, 서버는 최대 `MAX_MENTIONS`(8)명을 요청하고 그 수까지만 돌려줍니다. 목록은 텍스트 영역에서 조작하는 listbox입니다. 화살표 키로 이동하고, Enter나 Tab으로 고르고, Escape로 닫습니다.

`.remember(key)`는 각 대화의 보내지 않은 텍스트를 보낼 때까지 클라이언트 설정 저장소에 `App::client_prefs`로 등록한 `Draft` 키 아래 보관합니다. 임시 저장본은 최대 `MAX_DRAFT_CHARS`(4,000)자이고, 최대 `MAX_DRAFTS`(20)개 대화가 보관되며 가장 오래된 것부터 지워집니다. 대화 id는 해시로만 저장됩니다. `Purpose::Preference`를 쓰면 사용자가 더 많은 저장에 동의할 때까지 임시 저장본은 세션 동안만 남습니다.

## TypingIndicator

`TypingIndicator`는 대화에서 다른 누가 입력 중인지 polite 라이브 영역에서 보여 줍니다. "Ada가 입력 중", "Ada와 Grace가 입력 중", "3명이 입력 중"처럼 표시됩니다.

```rust
let realtime = Realtime::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

이 컴포넌트는 실시간 채널(`realtime` feature, `realtime.md` 참고) 위에서 `Realtime::topic`으로 선언한 topic을 통해 동작합니다. 사용자가 같은 대화의 `Composer`에서 입력하는 동안 브라우저는 페이지의 라이브 소켓으로 작은 신호를 최대 3초에 한 번 보냅니다. 서버는 사용자별로 그보다 빠른 신호를 버리고, 신호를 테넌트 안의 topic의 다른 구독자에게 전달하며, 같은 사용자에게는 절대 되돌려 보내지 않습니다. 신호에는 대화 id와 이 서버만 만들 수 있는 사용자 키가 들어 있고, 사용자 id나 텍스트는 절대 들어 있지 않습니다. 이름은 마지막 신호 뒤 6초 동안 표시되며, `.person(user, name)`으로 지정하지 않은 사람은 "누군가"로 표시됩니다.

`realtime` feature가 없거나 앱에 실시간 채널이 없으면 표시기는 아무것도 렌더링하지 않고, 스크립트도 불러오지 않으며, 아무것도 보내지 않습니다. 신호는 테넌트 안의 topic의 모든 구독자에게 전달되므로 대화 id가 그들에게 보입니다. 불투명한 id를 쓰거나 대상마다 topic을 따로 두십시오.
