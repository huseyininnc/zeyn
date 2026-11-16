# 관리 화면

백오피스용 컴포넌트입니다. 레코드에서 무엇이 바뀌었는지, 읽기 전용 코드와 데이터, 바코드, 알림, 보드, 달력을 다룹니다. 모두 서버에서 렌더링되며, 사용하는 페이지에서만 스타일시트와 스크립트를 불러옵니다.

## DiffView

`DiffView`는 레코드의 두 값 사이에서 바뀐 필드를 보여 줍니다. 이전 값에는 취소선이, 새 값에는 추가 표시가 붙습니다. 감사 기록에서는 `DiffView::changes(entry.changes)`로, 모델의 두 값에서는 `DiffView::of(&before, &after)`로 만듭니다. 후자는 감사 추적과 같은 방식으로 비교합니다(행 버전은 제외).

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()`을 주면 필드는 사용자 언어의 레이블을, 선택 필드는 선택지 레이블을 보여 줍니다. 나란히 보기 레이아웃은 이전 값 열과 새 값 열이 있는 표입니다. `DiffLayout::Inline`은 새 값을 이전 값 뒤에 두며 창처럼 좁은 곳에 맞습니다. 최대 `MAX_DIFF_FIELDS`(200)개 필드를 보여 주고 나머지는 안내 문구로 알립니다.

마스킹된 필드는 계속 마스킹됩니다. 값은 감사 추적이 보관하는 마스킹된 텍스트이고, 모델이 `#[mask]`로 표시한 필드는 보여 주기 전에 다시 마스킹되므로 손으로 만든 변경 목록이라도 원래 값이 페이지에 들어가지 않습니다. 이 컴포넌트에는 스크립트가 필요 없으며 `diff.css`(1.9 KB)는 이 컴포넌트와 함께일 때만 불러옵니다.

## CodeView와 JsonView

`CodeView`는 소스 텍스트를 읽기 전용으로 보여 줍니다. 서버에서 강조 표시되며 줄 번호, 접기, 복사 버튼이 있습니다. `JsonView`는 JSON 값을 같은 방식으로 들여쓰기해 보여 줍니다.

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

강조기(`zeyn::ui::highlight`)는 Rust, JSON, TOML, SQL, JavaScript, 셸용으로 직접 작성되었습니다. `CodeLang::Plain`과 그 밖의 이름은 이스케이프된 텍스트를 보여 줍니다. 키워드, 문자열, 숫자, 주석, 타입, 키를 `z-hl-*` span으로 감싸며 색은 테마 토큰에서 오므로 두 테마 모두 동작하고 브라우저는 아무 일도 하지 않습니다. 여러 줄에 걸친 괄호 블록(`{}`와 `[]`)은 스크립트가 필요 없는 기본 `<details>` 요소로 접힙니다. 첫 줄이 버튼(Enter 또는 Space)이고, 접힌 블록은 말줄임표로 끝납니다.

`JsonView::of(value, &ctx)`는 어떤 값이든 엔드포인트가 브라우저로 보내는 방식대로 직렬화하므로 마스킹된 필드는 계속 마스킹됩니다. 최대 `MAX_CODE_LINES`(10,000)줄과 `MAX_CODE_BYTES`(1 MB)까지 보여 주며, 더 긴 텍스트는 줄 끝에서 잘리고 몇 줄을 보여 주는지 알리는 안내가 붙습니다. 복사 버튼(`Share`도 담당하는 `share.js`)은 보이는 줄을 복사하고 알림으로 확인해 주며, 스크립트가 실행되기 전에는 숨겨져 있습니다. `code.css`(2.7 KB)는 이 컴포넌트와 함께일 때만 불러옵니다.

| 키 | 동작 |
|---|---|
| Tab | 텍스트로 이동(한 번의 탭 정지, 화살표 키로 스크롤)한 뒤 접기 버튼과 복사 버튼을 차례로 지납니다 |
| Enter, Space | 접기 버튼에서: 블록을 접거나 펼칩니다 |

## Barcode와 QrCode

`Barcode::code128(value)`와 `QrCode::new(value)`는 서버에서 심볼을 SVG로 그립니다. 두 인코더 모두 공개 표준(Code 128과 QR Code)을 따라 직접 작성되었으므로 크레이트도 스크립트도 추가되지 않습니다.

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode`는 출력 가능한 ASCII 문자 1~80자(`MAX_BARCODE_CHARS`)를 인코딩하며, 연속된 숫자는 한 심볼에 두 자리씩 묶고(코드 세트 C) 검사 심볼과 양쪽 10모듈의 여백 영역을 붙입니다. `QrCode`는 텍스트를 UTF-8 바이트로, 선택한 수준(`QrLevel::Low`, 기본값 `Medium`, `Quartile`, `High`)에서 담을 수 있는 가장 작은 버전(1~40)으로 인코딩하며, 리드 솔로몬 오류 정정, 벌점이 가장 낮은 마스크, 4모듈의 여백 영역을 씁니다. 최대 `MAX_QR_BYTES`(2,953)바이트입니다. 인코딩할 수 없는 값은 깨진 심볼 대신 짧은 안내를 보여 줍니다.

각 심볼은 figure이며, 스크린 리더는 아래 텍스트로 이름 붙은 하나의 이미지로 읽습니다. 그 텍스트는 값 자체이거나, 지정했다면 `.label(text)`입니다. 스캐너는 밝은 바탕의 어두운 막대가 필요하므로 두 테마 모두에서 막대는 흰 판 위에 검게 유지됩니다. `barcode.css`(1 KB)는 이 컴포넌트와 함께일 때만 불러오며, `.height(px)`와 `.size(px)`로 그리는 크기를 정하고 좁은 컨테이너에서는 심볼이 줄어듭니다.

## Inbox와 NotificationCenter

`Inbox`는 사용자의 알림을 최신순으로, 사용자 시간대의 날짜별로(오늘, 어제, 그다음 날짜) 묶어 보여 주고 읽지 않은 알림을 표시합니다. 행은 DataGrid가 쓰는 것과 같은 종류의 액션(`DataQuery` 입력, `Page` 출력)에서 오며, 받은 편지함은 시간 필드로 정렬하고 최대 `.limit(n)`행(기본 50, 최대 `MAX_INBOX` 200)을 요청합니다. `NotificationCenter`는 상단 바의 종 아이콘으로, 읽지 않은 개수와 함께 받은 편지함 화면으로 가는 링크입니다.

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

`.mark_read(action)`와 `.archive(action)`을 쓰면 각 행에 체크 상자가, 받은 편지함에 "모두 선택"이 있는 도구 모음이 생깁니다. 버튼은 선택한 키를 `InboxSelection`으로 `POST /_zeyn/data`에 보내며, 이 엔드포인트는 CSRF를 확인하고 쓰기 제한에 포함되며 화면 접근 권한과 액션 권한을 확인합니다. 키는 최대 200개, 각 200자까지 받습니다. 키는 브라우저에서 오므로 액션은 `ctx.identity()`의 알림만 바꿔야 합니다. 사용자가 실행할 수 없는 버튼은 체크 상자와 함께 빠집니다.

일괄 작업 후 받은 편지함은 자신의 토픽(`"notifications"`, `.topic(..)`으로 변경)을 알립니다. 받은 편지함과 그 토픽을 `.listens`하는 모든 노드가 제자리에서 다시 그려지고 종도 개수를 다시 불러옵니다. 종은 `UnreadCount { limit }`을 받아 한도(100)에서 멈추는 액션, 즉 한 번의 제한된 쿼리로 셉니다. 99를 넘으면 `99+`로 보여 주며 링크 이름이 스크린 리더에 개수를 알려 줍니다. 페이지가 보인 뒤 한 번의 요청으로 개수를 불러오고, 받은 편지함이나 `ui.refresh("notifications")`, 또는 `.live()`와 `realtime` 기능을 쓸 때 어떤 액션의 `socket.notify(&ctx, "notifications")`가 토픽을 알리면 다시 불러옵니다(토픽은 `ZeynSocket::topic`으로 선언). `inbox.js`(6.1 KB, gzip 2.3 KB)와 `inbox.css`(3.1 KB)는 이 컴포넌트와 함께일 때만 불러옵니다.

## Kanban

`Kanban::of(flow, cards, move_card)`는 액션의 레코드를 열 안의 카드로 보여 주며, 워크플로의 `#[choice]` 열거형의 상태마다 열이 하나씩 열거형 순서대로 놓입니다. 각 열은 그 상태의 카드에 대한 가상 목록입니다. 액션은 상태로 범위가 정해진 `DataQuery`를 받으므로 데이터베이스에서 거르고, 카드는 열을 스크롤할 때 50개씩(`KANBAN_BLOCK`) 불러옵니다. 열 머리에는 카드 수와 WIP 한도가 보이며, 한도를 넘은 열은 표시됩니다.

```rust
#[action(permission = "tasks.work")]
pub async fn move_task(r: TransitionRequest<TaskId>, store: Tasks, ctx: Ctx) -> Result<TaskId> {
    let mut task = store.get(r.id)?;
    task.state = task_flow().apply(&ctx, task.state, &r.transition, r.comment.as_deref())?;
    store.save(task)
}

Kanban::of(task_flow(), search_tasks, move_task)
    .state(Task::state)
    .card(|t: &Task| Stack::new().add(Paragraph::new(t.title.clone())).add(Badge::new(t.owner.clone())))
    .wip(TaskState::Doing, 5)
    .confirm("reopen", t!("tasks.reopen_confirm"))
```

카드를 한 열에서 다른 열로 옮기면 `Transitions`처럼 `TransitionRequest`로 두 상태 사이의 전이를 실행합니다. 보드는 사용자가 할 수 있는 이동만 보여 줍니다. 서버는 전이를 찾아 그 권한과 이동 액션의 권한을 확인하고(`POST /_zeyn/data`를 통한 CSRF와 쓰기 제한), 액션은 `Workflow::apply`로 전이를 적용하는데, 이미 다른 상태가 된 카드는 거부됩니다. `.confirm(name, message)`는 공용 대화 상자에서 먼저 묻습니다. 카드는 바로 옮겨지며, 서버가 거부하면 이유와 함께 원래 열로 돌아가고, 받아들이면 두 열이 한 번의 요청으로 다시 그려집니다.

마우스나 펜은 6 px 움직인 뒤, 터치는 250 ms 길게 누른 뒤 카드를 끌기 때문에 스와이프는 계속 열을 스크롤합니다(공용 끌기 코어 `drag.js`). 갈 수 있는 열은 테두리로 표시되고 나머지는 흐려지며, 카드를 보드 가장자리 가까이 두면 보드가 옆으로 스크롤됩니다. 각 카드의 버튼은 키보드와 같은 메뉴를 열며, 단일 포인터로 옮기는 방법입니다(WCAG 2.5.7). `kanban.js`(19.7 KB, gzip 6 KB), `drag.js`, `kanban.css`(4.5 KB)는 보드와 함께만, `vlist.js`는 그 열과 함께 불러옵니다.

`.sortable(Task::rank, reorder_tasks)`는 각 열의 카드를 `Rank` 필드 순서로 유지하며, 정렬 가능한 목록의 순서 변경 액션(`ReorderRequest`)을 씁니다. 각 카드에는 열 안에서 순서를 바꾸는 핸들이 생깁니다(끌기, 키, 이동 메뉴). 다른 카드 위에 놓은 카드는 그 앞이나 뒤에 들어가고, 선이 위치를 보여 줍니다. 같은 열 안에서는 순서 변경을 보내고, 다른 열로는 먼저 전환을 실행한 뒤 카드의 새 이웃으로 순서 변경을 보냅니다. 두 요청 모두 서버의 검사를 거칩니다. 열의 노드, CSRF, 쓰기 한도, 각 액션의 권한입니다. `sortable.js`는 정렬 가능한 보드와 함께만 불러옵니다.

| 키 | 동작 |
|---|---|
| Tab | 보드로, 마지막으로 포커스를 받은 카드로 이동합니다(보드 전체에 탭 정지 하나) |
| Up, Down | 열의 이전 또는 다음 카드 |
| Left, Right | 이전 또는 다음 열의 같은 자리에 있는 카드(`dir="rtl"`에서는 서로 바뀜) |
| Home, End | 열에서 보이는 첫 번째 또는 마지막 카드 |
| Enter, Space | 이동 메뉴를 엽니다: Up과 Down으로 열을 고르고 Enter로 옮기며 Escape로 닫습니다 |
| Shift + Left, Shift + Right | 워크플로가 허용하면 카드를 이전 또는 다음 열로 옮깁니다 |
| Alt + 위, Alt + 아래 | 정렬 가능한 보드에서 카드를 열 안에서 위나 아래로 옮김 |

## Calendar

`Calendar::of(action)`은 사용자 시간대에서 월, 주, 일 단위로 일정을 보여 줍니다. 액션은 DataGrid가 쓰는 종류이며 보이는 범위만 받습니다. 한 필터는 마지막으로 보이는 날까지 시작하는 일정을, 다른 필터는 첫날부터 끝나는 일정을 남깁니다(날짜 경계는 사용자 시간대의 하루 전체이므로 데이터베이스가 일을 합니다). 결과는 시작 시각순으로 정렬되며 최대 `MAX_CALENDAR_EVENTS`(500)개입니다. 서버가 모든 일정을 배치하고 보기 전체를 그립니다. 시각은 `Ctx`의 시간대(`TimePolicy`)로 표시되므로 UTC 23:30의 일정은 베를린에서는 다음 날입니다.

```rust
#[action(permission = "events.write")]
pub async fn new_event(slot: CalendarSlot) -> Result<Option<Effect>> {
    Ok(Some(Window::open(event_form, EventFormParams { start: slot.start, end: slot.end })))
}

#[screen(route = "/planner", permission = "events.read")]
pub fn planner(p: CalendarParams) -> Screen {
    Screen::new(t!("planner.title")).body(
        Calendar::of(search_events)
            .start(Event::starts)
            .end(Event::ends)
            .label(|e: &Event| e.title.clone())
            .open(|e: &Event| Window::open(event, EventParams { id: e.id }))
            .create(new_event)
            .at(p),
    )
}
```

보기와 날짜는 화면 매개변수입니다(`CalendarParams`, 또는 `view`와 `date`라는 필드). 도구 모음의 링크(이전, 오늘, 다음, 월, 주, 일)는 달력의 토픽(`"calendar"`, `.topic(..)`)을 통해 이를 제자리에서 바꿉니다. 한 번의 `POST /_zeyn/nodes`로 달력이 다시 그려지고 주소도 따라 바뀌므로 보기를 공유할 수 있으며 스크립트 없이도 동작합니다. `.open(|e| effect)`은 각 일정을 효과를 실행하는 버튼으로 만듭니다. 예를 들어 창을 여는데, 그 화면은 자체 접근 권한을 확인합니다. `.create(action)`을 쓰면 사용자가 시간대를 고릅니다. 날짜를 클릭하거나 시간 위를 끌거나 Shift와 화살표 키를 쓴 뒤 Enter나 N을 누릅니다. 시간대는 현지 벽시계 시각으로 `POST /_zeyn/data`를 통해 전송되며(CSRF, 쓰기 제한, 액션 권한), 서버는 이를 사용자 시간대에서 읽고 시계가 건너뛰는 시각과 31일보다 긴 구간을 거부한 뒤 `CalendarSlot`을 액션에 넘깁니다. 액션의 효과는 달력이 다시 그려지기 전에 실행됩니다. 권한이 없는 사용자는 시간대를 고를 수 없습니다. `.hours(from, to)`는 주 보기와 일 보기의 행을 정합니다(7시부터 21시).

컴포넌트 카탈로그에서 측정(debug 서버): 월, 주, 일 보기 사이를 전환하면 클릭 후 새 보기가 전체 CPU에서 21~34 ms, 4배 느린 CPU에서 60~163 ms 뒤에 보입니다. `calendar.js`(9.6 KB, gzip 3.3 KB)와 `calendar.css`(4.4 KB)는 달력과 함께일 때만 불러옵니다.

| 키 | 동작 |
|---|---|
| Tab | 그리드로, 선택한 날짜로 이동합니다(그리드 전체에 탭 정지 하나) |
| 화살표 키 | 다음 날 또는 다음 시간; `dir="rtl"`에서는 Left와 Right가 바뀝니다 |
| Home, End | 행의 첫 번째 또는 마지막 셀 |
| Page Up, Page Down | 이전 또는 다음 월, 주, 일 |
| Enter | 셀의 일정으로 들어갑니다(Up과 Down으로 이동, Escape로 돌아옴). 빈 셀에서는 일정을 만듭니다 |
| Shift + 화살표 키 | 달력이 만들 수 있을 때 선택한 시간대를 넓힙니다 |
| N | 달력이 만들 수 있을 때 선택한 시간대에 일정을 만듭니다 |
