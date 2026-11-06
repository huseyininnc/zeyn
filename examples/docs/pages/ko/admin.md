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

일괄 작업 후 받은 편지함은 자신의 토픽(`"notifications"`, `.topic(..)`으로 변경)을 알립니다. 받은 편지함과 그 토픽을 `.listens`하는 모든 노드가 제자리에서 다시 그려지고 종도 개수를 다시 불러옵니다. 종은 `UnreadCount { limit }`을 받아 한도(100)에서 멈추는 액션, 즉 한 번의 제한된 쿼리로 셉니다. 99를 넘으면 `99+`로 보여 주며 링크 이름이 스크린 리더에 개수를 알려 줍니다. 페이지가 보인 뒤 한 번의 요청으로 개수를 불러오고, 받은 편지함이나 `ui.refresh("notifications")`, 또는 `.live()`와 `realtime` 기능을 쓸 때 어떤 액션의 `hub.notify(&ctx, "notifications")`가 토픽을 알리면 다시 불러옵니다(토픽은 `Realtime::topic`으로 선언). `inbox.js`(6.1 KB, gzip 2.3 KB)와 `inbox.css`(3.1 KB)는 이 컴포넌트와 함께일 때만 불러옵니다.
