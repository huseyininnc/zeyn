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
