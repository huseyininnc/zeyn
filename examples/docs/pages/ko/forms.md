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
| `.settings()` | 설정 레이아웃: 각 컨트롤 옆에 레이블과 도움말, 무언가 바뀌면 아래에 고정되는 저장 바 |
| `.step(title)` | 마법사 단계. 폼은 한 번에 한 단계를 보여 줍니다 |
| `.range(M::from, M::to)` | 두 날짜를 하나의 범위 피커로 |
| `Form::auto(value)` | 선언 순서대로 모든 필드 |

폼의 너비는 최대 720 px이고, 폼만 담은 카드나 패널은 폼과 같은 너비이므로 카드의 절반이 비어 있지 않습니다. 레이블은 36 px 입력 위 6 px에 놓이고, 필드 사이 간격은 20 px입니다. 포커스된 입력은 포커스 색의 테두리와 부드러운 3 px 링을 받고, 잘못된 입력은 danger 테두리를 받습니다. 단위, 통화, `%`는 입력 안에 흐리게 그려지며, 숫자 필드는 빼기와 더하기 버튼(28 px)을 필드 끝 안쪽에 둡니다. 화살표 키로도 여전히 값을 바꿀 수 있습니다.

## 저장

`.submit(label, action)`은 저장하는 action을 지정합니다. action은 폼의 모델에 대한 `Valid<M>`을 받아야 하며, 그렇지 않으면 컴파일되지 않습니다. 런타임이 값을 보내면 서버가 모든 규칙을 실행한 다음 action을 실행합니다.

- 규칙이나 action의 `ValidationErrors`에서 나온 필드 오류는 필드 옆에 표시됩니다.
- `Error::Conflict`(오래된 `#[version]`)는 다른 사람이 레코드를 변경했다는 메시지를 표시합니다.
- `.on_success(|result, ui| ...)`는 저장이 성공한 후 effect를 실행합니다([Effect와 창](effects.md) 참고).

## 컨트롤

각 필드 타입에는 기본 컨트롤이 있습니다. 텍스트 입력, 숫자 입력, 통화가 있는 금액, 날짜 및 시간 피커, `bool`용 체크박스, choice용 선택 목록입니다. `.control(M::f, Control::...)`은 다른 컨트롤을 고르고, `.lookup(M::f, action)`은 서버에서 레코드를 검색합니다. [컨트롤과 피커](controls.md)를 참고하십시오.

## 의존하는 lookup

`.depends_on(field, source)`는 lookup `field`의 선택지가 다른 필드에 따라 달라지게 합니다. 국가의 도시처럼요. 검색할 때마다 `source`의 값이 `LookupQuery::parent`로 전송되고(`q.parent_id()`는 이를 id로 읽습니다), `source`가 바뀌면 `field`가 비워져 새 값에 맞는 선택지가 다시 불러와집니다.

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

이 짝은 브라우저에 맡길 수 없으므로, 제출 시 서버는 선택된 도시를 제출된 국가와 함께 lookup action에 그 action의 권한으로 다시 물어보고, 반환되지 않는 도시는 필드 오류로 거부합니다. `q.apply` 전에 필터링하듯이 id 조회에서도 부모로 필터링하세요. `App::check_page`는 필드가 폼의 lookup이 아니거나 원본이 폼의 다른 필드가 아닌 `depends_on`을 보고합니다.

## 필터 폼

제출 버튼이 없는 폼으로 Screen을 필터링할 수 있습니다. `.drives(topic)`은 폼의 필드를 Screen params로 만듭니다. 필드가 바뀌면(선택 상자, 날짜 범위, Enter를 누른 텍스트 필드) 그 값이 같은 이름의 Screen param에 들어가고, 주소도 맞춰지며, `.listens(topic)`을 가진 노드가 제자리에서 새로 고쳐져 자신의 params에서 값을 읽습니다([토픽](effects.md#topics) 참고). 숫자와 불리언은 그대로, 빈 필드는 `null`로 보내지며, 서버는 params를 Screen의 타입이 있는 params로 읽으므로 맞지 않는 값은 거부됩니다.

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

`submit`이 있는 폼은 drive할 수 없습니다. 대신 저장한 뒤 `on_success`에서 `ui.refresh(topic)`으로 새로 고치세요.

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

놓은 폴더는 그 안과 하위 폴더의 파일을 추가하며, 폴더가 끝날 때까지 최대 100개 항목씩 읽습니다. 필드가 받지 않는 형식의 파일은 건너뛰고 그 수를 라이브 영역에서 알립니다. 필드에 포커스가 있을 때 붙여넣은 파일도 업로드됩니다. 확장자나 형식이 `accept`에 없는 파일은 한 바이트도 보내기 전에 브라우저에서 거부되며, 서버는 내용을 다시 확인합니다. 한 필드에서 동시에 업로드되는 파일은 최대 세 개이고, 나머지는 사용 중 진행률 막대와 함께 대기합니다.

| 키 | 동작 |
|---|---|
| Tab | 파일 입력으로 이동한 다음 각 파일의 다운로드 링크와 취소, 다시 시도, 제거 버튼으로 이동합니다 |
| 파일 입력에서 Enter 또는 Space | 시스템 파일 선택기를 엽니다 (드래그 앤 드롭의 키보드 경로) |
| 파일 입력에서 Ctrl+V 또는 Cmd+V | 스크린샷 같은 클립보드의 파일을 업로드합니다 |
| 취소, 다시 시도, 제거에서 Enter 또는 Space | 실행합니다. 취소 후에는 포커스가 다시 시도로, 제거 후에는 다음 파일 또는 입력으로 이동합니다 |

`POST /_zeyn/files/{store}`에서 서버가 하는 일은 다음과 같습니다.

1. CSRF 헤더, 스토어의 권한, Feature의 가드를 검사하고 인터셉터를 실행합니다(`CallKind::Upload`, 새 파일 id와 함께 감사됨). 업로드는 쓰기 제한에 포함됩니다.
2. 선언된 `Content-Length`가 제한을 넘으면 거부하고(`413`), 처음 4 KiB를 읽어 내용으로 타입을 찾습니다. 확장자는 같은 타입에 속해야 하고 타입은 `accept`에 있어야 하며, 그렇지 않으면 `415`입니다. HTML, SVG, XML, 스크립트는 절대 허용되지 않습니다. 브라우저의 콘텐츠 타입은 무시합니다.
3. 나머지를 네 개짜리 채널을 통해 최대 64 KiB 조각으로 스토어에 스트리밍하며 바이트를 셉니다. 제한 초과, 끊긴 연결, 30초 동안 데이터가 없는 경우 업로드를 중단하고 스토어는 쓴 것을 제거합니다.
4. Feature의 `FileScanner`를 실행합니다. 거부된 파일이나 실패한 스캐너의 파일은 삭제되고 응답은 `422`입니다.
5. `{id, name, size, type}`과 함께 `201`로 응답합니다. id는 서버에서 만든 128비트 난수이며, 이름은 경로와 제어 문자가 제거됩니다.

`image` 기능을 켜면 `FileRules::clean_images()`가 저장소에 파일이 들어간 뒤 한 단계를 더합니다. PNG, JPEG, WebP 업로드는 디코딩된 뒤 같은 형식으로 다시 인코딩되므로 EXIF(GPS 포함), XMP, ICC 프로필, 텍스트 청크, 이미지 데이터 뒤에 숨긴 바이트가 서버에 남지 않습니다. EXIF 방향은 먼저 픽셀에 적용되고, JPEG는 품질 90으로, PNG와 WebP는 무손실로 기록됩니다. 디코딩은 한 변 `MAX_IMAGE_SIDE`(12,000) 픽셀과 `MAX_IMAGE_MEMORY`(256 MiB)에서 멈추며, 디코딩되지 않는 이미지는 삭제되고 응답은 `422`입니다. 두 한도는 픽셀을 디코딩하기 전에 이미지 헤더의 크기로 확인되므로, 거대한 이미지라고 주장하는 작은 파일은 그 메모리를 쓰지 않고 거부되며, 애니메이션 이미지는 첫 프레임만 남습니다. 정리된 파일은 새 id로 원본을 대신하고, 그 뒤 스캐너가 정리된 파일을 검사합니다. 기능이 없으면 이 메서드가 없으므로, 저장소가 정리하지 않으면서 정리한다고 여길 수 없습니다.

다운로드(`GET /_zeyn/files/{store}/{id}`)는 id의 형태, 권한, 가드, 테넌트를 검사하고 미디어 제공(`docs/media.md` 참고)을 거칩니다. 범위, 엔티티 태그, `nosniff`가 적용됩니다. 래스터 이미지, 오디오, 비디오만 인라인으로 제공되며, 그 밖의 모든 것은 `Content-Security-Policy: sandbox`와 함께 따옴표로 묶은 ASCII 이름과 UTF-8 이름이 있는 `Content-Disposition: attachment`를 붙여 `application/octet-stream`으로 전송됩니다.

`DiskStore`는 `{id}.bin`과 `{id}.json`을 하나의 폴더에 보관하며 업로드를 먼저 `{id}.part`에 씁니다. `MemoryStore`는 바이트 상한이 있는 메모리에 파일을 보관하며 테스트와 데모용입니다. 폼 값은 파일의 이름만 담습니다. 파일이 존재하는지 확실히 해야 하는 저장 action은 쇼케이스처럼 `store.meta(id)`를 호출합니다.

`FileRules::quota(bytes, window)`는 한 사용자가 창(window)마다 저장소에 업로드할 수 있는 양을 제한하며 앱의 rate store로 셉니다. 한도를 넘긴 업로드는 그대로 저장되고 다음 업로드가 `429`를 받습니다. `DiskStore`는 한 시간보다 오래된 `.part` 파일(충돌로 멈춘 업로드)을 한 시간에 최대 한 번 지웁니다. `ZipGuard`는 ZIP 기반 업로드(ZIP, Word, Excel, PowerPoint)를 위한 스캐너로, 중앙 디렉터리가 10,000개를 넘는 항목, 합계 1 GiB 초과 또는 파일 크기의 200배 초과를 선언하는 아카이브와 ZIP64 아카이브를 거부합니다. 받아들인 아카이브를 푸는 코드도 이 한도에서 멈춰야 합니다.

예산: 256 MiB 업로드는 서버 힙을 약 0.4 MiB 늘립니다(`tests/files_memory.rs`). 컨트롤은 DOM 노드 10개에 파일당 5개를 렌더링하며, `files.js`는 원본 21 KB(gzip 7 KB), `files.css`는 원본 3 KB(gzip 1 KB)를 추가합니다.

### 이어 올리기

Cargo 기능 `resumable`을 켜면 업로드 필드와 `FileDrop`은 tus 1.0 프로토콜의 일부로 파일을 보냅니다. 그래서 네트워크가 끊긴 업로드는 처음부터 다시 시작하지 않고 멈춘 곳에서 이어집니다. 컨트롤은 바꿀 필요가 없습니다.

```toml
zeyn = { version = "0.0", features = ["resumable"] }
```

```rust
App::new("Support").resumable_uploads(
    ResumableUploads::new("data/uploads")
        .expires(Duration::from_secs(6 * 3600))
        .max_per_user(3)
        .max_open(500),
)
```

`resumable_uploads`를 부르지 않으면 끝나지 않은 업로드는 시스템 임시 폴더에 저장되고, 마지막 바이트 이후 24시간 동안 보관되며, 사용자당 최대 5개, 전체 최대 1000개까지 열어 둘 수 있습니다.

| 요청 | 응답 |
|---|---|
| `Upload-Length`와 `Upload-Metadata: filename <base64>`가 있는 `POST /_zeyn/uploads/{store}` | `Location`, `Upload-Offset: 0`, `Upload-Expires`가 있는 `201`; 저장소 한도를 넘으면 `413`, 빈 파일은 `422`, 할당량이나 열린 업로드 한도를 넘으면 `429` |
| 해당 위치에 `HEAD` | `Upload-Offset`과 `Upload-Length`가 있는 `200` |
| `Content-Type: application/offset+octet-stream`과 `Upload-Offset`이 있는 `PATCH` | 새 `Upload-Offset`이 있는 `204`; 오프셋이 서버와 다르면 `409`, 같은 업로드의 다른 `PATCH`가 아직 실행 중이면 `423`, 선언보다 많은 바이트는 `413` |
| 마지막 바이트를 가져오는 `PATCH` | 일반 업로드처럼 `{id, name, size, type}`가 있는 `200` |
| 해당 위치에 `DELETE` | `204`; 업로드와 그 바이트가 삭제됩니다 |

모든 요청에는 `Tus-Resumable: 1.0.0`(없으면 `412`), 저장소의 쓰기 권한, 기능의 가드가 필요하고, `POST`, `PATCH`, `DELETE`에는 CSRF 헤더도 필요합니다. 업로드는 그것을 만든 사용자와 테넌트만 찾을 수 있으며, 다른 사람은 `404`를 받습니다. 업로드 생성은 일반 업로드처럼 쓰기 한도에 포함되지만, 그 `PATCH`는 선언한 길이보다 많이 실을 수 없으므로 포함되지 않습니다. 브라우저는 파일을 흔한 리버스 프록시의 기본 본문 한도인 1 MiB 단위 `PATCH`로 보내므로, 연결이 끊겨도 다시 보내는 양은 최대 그만큼입니다.

바이트는 도착하는 대로 업로드 폴더의 `{id}.part`에 기록되므로, 연결이 끊기기 전에 서버에 도착한 바이트는 보존되고 `HEAD`가 이를 알려 줍니다. 마지막 바이트가 도착하면 파일은 `POST /_zeyn/files/{store}`와 같은 단계를 거칩니다. 내용으로 찾은 형식, 크기 한도, 할당량, 저장소, 스캐너, 인터셉터(`CallKind::Upload`)입니다. 그다음 부분 파일이 삭제되며, 검사가 파일을 거부할 때도 마찬가지입니다. 만료 시간 동안 새 바이트가 없는 업로드는 다음에 조회되거나 다음 업로드가 만들어질 때 삭제되고, 재시작으로 남은 부분 파일은 만료 시간마다 한 번 정리됩니다.

브라우저에서 실패한 요청(네트워크 끊김, `409`, `423`, `429`, 서버 오류)은 브라우저가 다시 온라인이 된 뒤 1, 2, 4, 8초, 그다음 15초마다 연속 최대 12번 다시 시도됩니다. 각 시도는 먼저 오프셋을 묻고 나머지만 보냅니다. 취소는 요청을 멈추고 서버의 업로드를 삭제합니다. 최종 실패 뒤의 다시 시도는 서버가 아직 가지고 있으면 같은 업로드를 이어 갑니다.

tus 1.0과의 차이: creation, expiration, termination 확장만 있고, `OPTIONS` 탐색, 체크섬, 지연 길이, 연결은 없습니다. 또한 마지막 `PATCH`는 `204` 대신 저장된 파일과 함께 `200`을 응답하므로 필드는 추가 요청 없이 파일 id를 얻습니다. 열린 업로드는 그것을 만든 프로세스만 알기 때문에, 인스턴스가 여러 개인 로드 밸런서 뒤에서는 `/_zeyn/uploads/` 경로에 고정 세션(sticky session)이 필요합니다.

## File drop

`FileDrop::new(label, rules)`는 같은 업로드 컨트롤을 폼 밖에서 단독으로 쓰는 것으로, 문서 받은편지함처럼 파일이 작업의 전부인 화면을 위한 것입니다. 폼 필드처럼 폴더, 붙여넣은 파일, accept 목록을 받고, `rules`의 저장소에 업로드합니다. 저장소는 `Feature::files`가 권한과 함께 등록합니다. `.single()`은 한 번에 파일 하나를 받습니다. `.on_upload(action)`은 저장된 각 `File`로 액션을 실행합니다. 서버는 먼저 id가 자신이 만드는 형식인지 확인하고, 이름을 다시 정리하며, 규칙에 있는 콘텐츠 형식만 남기므로 액션은 브라우저가 지어낸 것을 받지 않습니다. 액션이 거부하면 항목에 이유가 표시됩니다. 영역 안의 파일 선택기가 키보드와 단일 포인터 경로입니다.

```rust
#[action(permission = "support.ticket.files")]
pub async fn attach(file: File, tickets: Tickets) -> Result<()> {
    tickets.attach(file).await
}

FileDrop::new(t!("ticket.attachments"), ATTACHMENTS).on_upload(attach)
```
