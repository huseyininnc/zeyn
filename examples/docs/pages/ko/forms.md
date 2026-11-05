# 폼

`Form::of(value)`는 모델이나 entity를 편집합니다. 필드는 필드 참조에서, 레이블은 로케일 파일에서, 검증은 `#[rule]` 속성에서 가져오므로 브라우저와 서버가 같은 규칙을 검사합니다.

## 폼

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

이름을 비워 두거나 잘못된 이메일을 입력한 뒤 *Send*를 누르십시오. 브라우저가 즉시 오류를 표시하며, 서버도 같은 입력을 같은 메시지로 거부합니다.

## 레이아웃

| 호출 | 결과 |
|---|---|
| `.field(M::f)` | 한 줄에 필드 하나 |
| `.row(\|r\| r.field(a).field(b))` | 필드를 나란히 배치합니다. 좁은 화면에서는 쌓입니다 |
| `.section(title)` | 이후 필드들의 제목 있는 그룹 |
| `.step(title)` | 마법사 단계. 폼은 한 번에 한 단계를 보여 줍니다 |
| `.range(M::from, M::to)` | 두 날짜를 하나의 범위 피커로 |
| `Form::auto(value)` | 선언 순서대로 모든 필드 |

## 저장

`.submit(label, action)`은 저장하는 action을 지정합니다. action은 폼의 모델에 대한 `Valid<M>`을 받아야 하며, 그렇지 않으면 컴파일되지 않습니다. 런타임이 값을 보내면 서버가 모든 규칙을 실행한 다음 action을 실행합니다.

- 규칙이나 action의 `ValidationErrors`에서 나온 필드 오류는 필드 옆에 표시됩니다.
- `Error::Conflict`(오래된 `#[version]`)는 다른 사람이 레코드를 변경했다는 메시지를 표시합니다.
- `.on_success(|result, ui| ...)`는 저장이 성공한 후 effect를 실행합니다([Effect와 창](effects.md) 참고).

## 컨트롤

각 필드 타입에는 기본 컨트롤이 있습니다. 텍스트 입력, 숫자 입력, 통화가 있는 금액, 날짜 및 시간 피커, `bool`용 체크박스, choice용 선택 목록입니다. `.control(M::f, Control::...)`은 다른 컨트롤을 고르고, `.lookup(M::f, action)`은 서버에서 레코드를 검색합니다. [컨트롤과 피커](controls.md)를 참고하십시오.

## 읽기 전용 필드

`#[readonly]` 필드는 폼에 표시되지만 편집할 수 없습니다. 재고 이동만 바꾸는 재고 수준이 그 예입니다.

## 파일 업로드

`File`, `Option<File>`, `Vec<File>` 필드는 `FileStore`가 보관하는 파일을 담습니다. 규칙을 `const`로 한 번 선언하고, 스토어를 권한과 함께 Feature에 등록하며, 폼에서 같은 규칙을 사용합니다.

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

권한 이름 하나는 같은 사용자에게 업로드와 다운로드를 모두 허용합니다. 추가할 수 없는 파일을 읽기 사용자가 열 수 있게 하려면 나누십시오: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`.

컨트롤은 드롭 영역 안의 기본 파일 입력입니다. 각 파일은 진행 막대와 함께 독립적으로 업로드되고, 진행 중에는 취소할 수 있으며, 실패하면 다시 시도할 수 있고, 완료되면 제거할 수 있습니다. 이미지는 선택한 파일의 object URL로 만든 미리 보기를 보여 줍니다. 진행 상황과 결과는 정중한 라이브 리전에서 안내됩니다. 쇼케이스의 `/uploads`에 동작하는 예제가 있습니다. 이 문서 사이트에는 로그인한 사용자가 없으므로 라이브 업로드는 표시되지 않습니다.

| 키 | 동작 |
|---|---|
| Tab | 파일 입력으로 이동한 다음 각 파일의 다운로드 링크와 취소, 다시 시도, 제거 버튼으로 이동합니다 |
| 파일 입력에서 Enter 또는 Space | 시스템 파일 선택기를 엽니다 (드래그 앤 드롭의 키보드 경로) |
| 취소, 다시 시도, 제거에서 Enter 또는 Space | 실행합니다. 취소 후에는 포커스가 다시 시도로, 제거 후에는 다음 파일 또는 입력으로 이동합니다 |

`POST /_zeyn/files/{store}`에서 서버가 하는 일은 다음과 같습니다.

1. CSRF 헤더, 스토어의 권한, Feature의 가드를 검사하고 인터셉터를 실행합니다(`CallKind::Upload`, 새 파일 id와 함께 감사됨). 업로드는 쓰기 제한에 포함됩니다.
2. 선언된 `Content-Length`가 제한을 넘으면 거부하고(`413`), 처음 4 KiB를 읽어 내용으로 타입을 찾습니다. 확장자는 같은 타입에 속해야 하고 타입은 `accept`에 있어야 하며, 그렇지 않으면 `415`입니다. HTML, SVG, XML, 스크립트는 절대 허용되지 않습니다. 브라우저의 콘텐츠 타입은 무시합니다.
3. 나머지를 네 개짜리 채널을 통해 최대 64 KiB 조각으로 스토어에 스트리밍하며 바이트를 셉니다. 제한 초과, 끊긴 연결, 30초 동안 데이터가 없는 경우 업로드를 중단하고 스토어는 쓴 것을 제거합니다.
4. Feature의 `FileScanner`를 실행합니다. 거부된 파일이나 실패한 스캐너의 파일은 삭제되고 응답은 `422`입니다.
5. `{id, name, size, type}`과 함께 `201`로 응답합니다. id는 서버에서 만든 128비트 난수이며, 이름은 경로와 제어 문자가 제거됩니다.

다운로드(`GET /_zeyn/files/{store}/{id}`)는 id의 형태, 권한, 가드, 테넌트를 검사하고 미디어 제공(`docs/media.md` 참고)을 거칩니다. 범위, 엔티티 태그, `nosniff`가 적용됩니다. 래스터 이미지, 오디오, 비디오만 인라인으로 제공되며, 그 밖의 모든 것은 `Content-Security-Policy: sandbox`와 함께 따옴표로 묶은 ASCII 이름과 UTF-8 이름이 있는 `Content-Disposition: attachment`를 붙여 `application/octet-stream`으로 전송됩니다.

`DiskStore`는 `{id}.bin`과 `{id}.json`을 하나의 폴더에 보관하며 업로드를 먼저 `{id}.part`에 씁니다. `MemoryStore`는 바이트 상한이 있는 메모리에 파일을 보관하며 테스트와 데모용입니다. 폼 값은 파일의 이름만 담습니다. 파일이 존재하는지 확실히 해야 하는 저장 action은 쇼케이스처럼 `store.meta(id)`를 호출합니다.

예산: 256 MiB 업로드는 서버 힙을 약 0.4 MiB 늘립니다(`tests/files_memory.rs`). 컨트롤은 DOM 노드 10개에 파일당 5개를 렌더링하며, `files.js`는 원본 12 KB(gzip 4 KB), `files.css`는 원본 3 KB(gzip 1 KB)를 추가합니다.
