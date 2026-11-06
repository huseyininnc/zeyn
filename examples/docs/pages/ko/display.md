# 표시

상태를 보여 주는 컴포넌트입니다. 메시지, 배지, 진행 표시, 키와 값, 이력, 자리 표시자가 있습니다. 서버에서 렌더링되며 자체 요청이 필요하지 않습니다.

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info`, `success`, `warning`, `danger` 또는 `Alert::new(tone, text)`를 사용합니다. `.title(text)`는 제목을, `.dismissible()`은 닫기 버튼을 추가합니다. 톤은 메시지 앞에서 음성으로도 전달되므로 색상만으로 전달되는 일이 없습니다.

## Badge

`Badge::new(text).tone(Tone::...)`입니다. 톤은 `Neutral`, `Info`, `Success`, `Warning`, `Danger`, `Brand`입니다. 그리드는 `.badge(|v| tone)`으로 값을 배지로 표시합니다.

## Progress

`Progress::new(label, value, max)`이며 선택적으로 `.tone(...)`을 붙일 수 있습니다. 레이블과 값이 있는 실제 `progressbar`입니다.

## DescriptionList

레코드의 필드에서 만들거나 직접 작성하는 키와 값의 쌍입니다.

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

값은 그리드 셀처럼 서식화됩니다. 금액, 날짜, choice가 사용자의 언어로 표시됩니다.

## Timeline

`Timeline::new().event(moment, title, detail, tone)`은 이벤트를 나열합니다. 시각은 사용자의 시간대와 언어로 표시됩니다.

## EmptyState와 Skeleton

`EmptyState::new(title).text(text)`는 빈 영역을 다음 단계로 채웁니다. `.link(label, screen, params)`는 `LinkButton`을 통해 다른 페이지로 이동하고, `.opens(label, screen, params)`는 앱의 Screen을 창(Window)으로 엽니다. `Skeleton::lines(n)`은 무언가를 불러오는 동안의 자리 표시자입니다.

## LinkButton

어딘가로 가는 것은 링크이고, 무언가를 하는 것은 버튼입니다. `LinkButton::new(label, screen, params)`는 Screen의 정식 경로(예: `/products/42`)로 가는 실제 `<a href>`이며 버튼처럼 보이므로, 가운데 클릭, "새 탭에서 열기", 검색 엔진이 모두 이것을 링크로 취급합니다. `nav.js`를 통해 페이지 새로고침 없이 열리고 Enter로 따라갑니다. 모양은 기본적으로 보조(secondary)이며, `.primary()`를 쓰면 주요 동작의 채워진 모양이 됩니다. Screen에는 route가 있어야 하며, 없으면 코드가 컴파일되지 않습니다. `EmptyState::link`와 `Hero::link`가 이것을 사용합니다.

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)`은 이 브라우저가 앱을 위해 보관하는 모든 클라이언트 환경설정을 지우는 버튼이며, 예를 들어 개인정보 페이지에서 `App::client_prefs_table()`의 목록 옆에 둡니다. 토스트가 이를 확인해 줍니다. 서버가 설정하는 쿠키(테마, 언어, 세션)는 건드리지 않습니다. 스크립트에서는 `zeyn.prefs.clear()`로 같은 일을 할 수 있습니다.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: 링크는 타입이 있는 Screen 참조이므로 Screen의 이름이 바뀌어도 깨진 링크가 남지 않습니다.

## ActionButton

`ActionButton::new(label, action, input)`은 action을 실행합니다. `.confirm(text)`는 먼저 확인을 묻고, `.tone(Tone::Danger)`는 파괴적인 작업임을 표시하며, `.on_success(|result, ui| ...)`는 effect를 실행합니다. 입력은 서버에서 고정되며 클라이언트에서 오지 않습니다. action의 권한은 다른 폼과 마찬가지로 서버에서 검사됩니다.

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

`Markdown::new(text)`는 댓글, 제품 설명, 도움말 같은 서식 있는 텍스트를 보여 줍니다. 텍스트는 문자열 또는 로케일 키입니다. `markdown` 기능(`zeyn = { features = ["markdown"] }`)이 필요하며, 이 기능은 의존성을 추가하지 않습니다.

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- 제목, 문단, 목록, 표, 코드 블록, 인용, 가로줄, 인라인 코드, 링크, 이미지, 굵게, 기울임을 지원합니다.
- 원시 HTML은 절대 통과시키지 않습니다. `<script>`는 텍스트로 표시되며 코드 블록도 이스케이프됩니다.
- 링크는 `http`, `https`, `mailto`, 앵커, 상대 경로만 가리킬 수 있습니다. `javascript:`나 `data:` 같은 다른 것은 레이블만 남습니다. 사용자 링크에는 `rel="noopener noreferrer nofollow ugc"`가 붙습니다.
- 이미지는 앱 자체(상대 경로)에서만 불러오며, 다른 이미지는 대체 텍스트를 표시합니다.
- `.trusted()`는 앱이 작성한 텍스트용입니다. 제목에 링크용 id가 붙고 링크에서 `nofollow ugc`가 빠집니다. 이스케이프와 링크 규칙은 그대로입니다.
- 64 KB(`MAX_MARKDOWN`)를 넘는 텍스트는 잘리고 중첩에도 상한이 있으므로, 악의적인 입력도 렌더링 비용이 작게 유지됩니다.

## Embed

`Embed`는 동영상 플레이어나 지도처럼 다른 사이트의 페이지를 잠긴 프레임 안에 보여 줍니다. 제목은 필수입니다(스크린 리더가 제목으로 프레임을 알려 줍니다). `.title(...)`이 없으면 embed를 배치할 수 없으며 컴파일러가 그렇게 알려 줍니다.

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- 호스트는 앱의 `frame-src`에 있어야 합니다. `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` ([보안](security.md#csrf-csp-and-headers) 참고). 주소가 `https`가 아니거나 호스트가 목록에 없으면 프레임이 전혀 렌더링되지 않으며, 콘텐츠를 표시할 수 없다는 경고가 나타납니다.
- 모든 프레임에는 `sandbox="allow-scripts allow-same-origin allow-presentation"`, `referrerpolicy="no-referrer"`, `loading="lazy"`가 붙습니다. 팝업, 최상위 탐색, 폼, 다운로드는 절대 허용할 수 없습니다. `.allow(...)`는 브라우저 기능을 추가합니다. `Fullscreen`, `PictureInPicture`, `Autoplay`, `EncryptedMedia`가 있습니다.
- `.click_to_load(poster)`는 먼저 포스터 이미지(앱의 경로 또는 `img-src`의 호스트, 없으면 `""`)와 다른 사이트의 이름이 있는 버튼을 보여 줍니다. 버튼을 누르기 전에는 다른 사이트에 접속하지 않으며, 누르면 프레임이 그 자리를 차지하고 포커스를 받습니다. 스크립트는 `media.js`의 일부이므로 추가 요청이 들지 않습니다.
- `.notice(t!(...))`는 콘텐츠 경고처럼 앱의 텍스트를 그 버튼에 표시합니다. 이스케이프되고, 다른 모든 텍스트처럼 번역되며, 버튼의 접근 가능한 이름의 일부이므로 스크린 리더가 사용자가 결정하기 전에 이를 읽습니다. `.click_to_load` 없이 사용하면 포스터 없는 버튼이 켜집니다.
- `.remember(EXTERNAL_VIDEO)`는 페이지마다가 아니라 키의 수명 동안 한 번만 묻습니다. 키는 `ClientPref::consent`로 선언하고 `App::client_prefs`에 등록한 동의 키입니다([클라이언트 환경설정](security.md#client-preferences) 참고). 같은 키에 바인딩된 embed는 응답을 공유합니다. 이후의 페이지, 앱 내 탐색, 다른 탭은 그 프레임을 포커스를 옮기지 않고 스스로 불러옵니다(WCAG 2.2 3.2.1 On Focus, 2.4.3 Focus Order). 포커스는 실제로 누른 후에만 프레임으로 이동합니다. 기록이 끝났거나 검사에 실패할 때, 앱이 키를 등록하지 않았을 때, 저장소가 차단되었고 페이지가 새것일 때, JavaScript가 없을 때 버튼이 다시 나타납니다. 일반 문자열이나 choice 키는 컴파일되지 않습니다.
- embed할 주소를 담는 필드에는 `#[rule(embed)]`를 붙입니다. 폼은 URL 또는 붙여 넣은 embed 코드를 받아 `src`만 남기고, `https`와 목록에 있는 호스트를 요구하며, 추적 매개변수(`utm_*`, `fbclid`, `gclid` 등)를 제거하고, HTML이 아니라 정규 주소를 저장합니다. API 입력은 정규 주소를 보내야 합니다.

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

`Image`에는 대체 텍스트와 크기가 필요합니다. `.alt(...)`와 `.size(width, height)`가 없으면 배치할 수 없으며 컴파일러가 그렇게 알려 줍니다. 크기는 파일을 불러오는 동안 공간을 유지하므로 페이지가 움직이지 않습니다.

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src`는 앱의 경로(`/static/logo.png`) 또는 앱의 `img-src` 호스트에 있는 `https` 주소입니다(`App::csp(|c| c.img_src([...]))`). 그 밖의 이미지는 렌더링되지 않고 대체 텍스트가 표시되므로 브라우저가 차단된 요청을 기록하지 않습니다.
- 모든 이미지에는 `decoding="async"`가 붙으며, `.lazy()`는 화면 아래쪽 이미지에 `loading="lazy"`를 추가합니다. `srcset`은 밀도(`2x`) 또는 너비(`800w`) 설명자를 받으며, 다른 호스트의 항목이나 다른 설명자를 쓴 항목은 제외됩니다. 너비 설명자는 이미지 너비로 `sizes`를 추가합니다.
- 순전히 장식용인 이미지는 빈 대체 텍스트를 사용합니다. `.alt(Text::from(""))`.

### 사진이 많은 긴 목록

제품 사진이 수천 장인 카탈로그에서는 이미지가 로드되는 동안 무언가를 보여 주고, 실패하면 복구할 수 있습니다. 모두 브라우저의 기본 기능을 사용하며, 미디어 모듈의 리스너 하나가 페이지의 모든 이미지를 처리합니다.

```rust
Image::new(p.photo_800.clone())
    .alt(p.name.clone())
    .size(400, 500)
    .lazy()
    .srcset([(p.photo_400.as_str(), "400w"), (p.photo_800.as_str(), "800w")])
    .sizes("(min-width: 800px) 25vw, 50vw")
    .placeholder(Placeholder::color("#d9dde3"))
    .low(p.photo_40.clone())
    .fallback("/static/no-photo.webp")
    .on_error(t!("shop.no_photo"))
    .fit(Fit::Cover)
```

- `.placeholder(...)`는 파일이 로드되는 동안 스크립트 없이 상자를 채웁니다. `Placeholder::color("#d9dde3")` 또는 테마 토큰(`"--z-surface-2"`), 앱이 만든 아주 작은 이미지를 위한 `Placeholder::blur(data_uri)`(최대 600바이트의 base64 `data:image/webp`, `avif`, `jpeg` 또는 `png` URI이며 SVG는 허용되지 않음), 모든 이미지가 함께 쓰는 파일 하나를 위한 `Placeholder::image(src)`가 있습니다. 규칙을 어기는 자리 표시자는 제외됩니다.
- `.low(src)`는 작은 저해상도 파일을 전체 파일 아래에 즉시 보여 줍니다. 전체 파일은 동시에 로드되며(이미지가 지연 로드이면 지연 로드됨) 준비되면 서서히 나타납니다. 동작 줄이기 설정에서는 페이드 없이 나타납니다.
- `.fallback(src)`는 이미지가 실패하면 다른 파일을 한 번 시도합니다. 그 후에는 `.on_error(text or component)`가 표시되며, 이것이 없으면 자리 표시자 위에 아이콘과 함께 대체 텍스트가 표시됩니다. 다시 시도하지는 않습니다.
- `.priority()`는 방문자가 처음 보는 이미지(가장 큰 페인트)를 표시합니다. `loading="eager"`와 `fetchpriority="high"`가 붙으며 지연 로드되지 않습니다. 페이지당 한 번만 사용하십시오.
- `.sizes(...)`는 레이아웃에서 이미지의 너비를 알려 주므로 브라우저가 알맞은 `srcset` 너비를 고릅니다. 이것이 없으면 지연 로드 이미지는 `sizes="auto"`를 받고 브라우저가 자리의 크기를 잽니다.
- `.fit(Fit::Cover)`는 상자를 채우고 넘치는 부분을 잘라 냅니다. `Fit::Contain`은 파일 전체를 보여 줍니다.
- `low`, `fallback`, `Placeholder::image`는 `src`와 같은 주소 규칙을 따르며, 그 밖의 주소는 제외됩니다. 상자는 항상 이미지의 크기를 유지하므로 파일이 로드되거나 실패하는 동안 아무것도 움직이지 않습니다.

## ExternalLink

`ExternalLink::new(url, text)`는 `rel="noopener noreferrer"`로 다른 사이트에 연결하므로, 다른 사이트는 페이지 주소도 창에 대한 핸들도 얻지 못합니다. 스크린 리더는 텍스트 뒤에 "external site"를 읽고, 시각적으로는 화살표가 이를 표시합니다.

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()`는 사용자가 입력한 주소에 `nofollow ugc`를 추가합니다. `.new_tab()`은 새 탭에서 열고 이를 스크린 리더에 알립니다. 설정하지 않으면 링크는 같은 탭에 머뭅니다. `https`와 `http` 주소만 링크가 되며, `javascript:` 같은 다른 것은 일반 텍스트로 표시됩니다.

## 툴팁, 팝오버, 사용 중 오버레이

툴팁은 컨트롤 하나에 대한 짧은 설명입니다. `ActionButton::tip(text)`, Screen 툴바의 `ToolButton::tip(text)`, 필드 레이블 뒤에 정보 버튼을 추가하는 `Form::tip(field, text)`가 있습니다. 텍스트는 컨트롤의 `aria-describedby`이므로 스크린 리더가 컨트롤과 함께 읽습니다. 잠깐 마우스를 올리면 표시되고 포커스를 받으면 즉시 표시되며, 포인터가 그 위에 있는 동안 유지되고(WCAG 1.4.13), Escape로 포커스를 옮기지 않고 숨길 수 있으며, 터치에서는 길게 누르면(정보 버튼을 탭하면) 표시됩니다. 툴팁은 사용자가 반드시 알아야 하는 정보를 담지 않습니다. 그런 정보는 레이블, 필드의 규칙, 페이지에 넣으십시오.

`Popover::new(label).title(title).add(node)`는 작은 세부 정보를 위해 옆에 비모달 대화 상자를 여는 버튼입니다. 포커스가 그 안으로 이동하며, Escape, 닫기 버튼, 바깥 클릭으로 닫히고 포커스는 버튼으로 돌아옵니다. 좁은 화면에서는 하단 시트로 열립니다. 자식은 일반 노드이므로 그 안의 폼은 모든 서버 검사와 함께 자신의 노드 경로로 전송됩니다.

`Busy::new().add(node)`는 그 안에서 요청이 실행되는 동안 내용을 덮습니다. 폼 저장, action 버튼, 그리드나 트리 로딩이 그 예입니다. 컨테이너에는 즉시 `aria-busy`가 붙고, 스피너와 "Working"이라는 텍스트(상태 메시지)는 400 ms 후 또는 `.delay_ms(n)` 후에만 표시되므로 빠른 응답에서는 깜박이지 않으며, 한 번 표시되면 최소 300 ms 동안 유지됩니다. 오버레이는 포인터를 막지만 키보드는 막지 않으며 포커스는 있던 자리에 남습니다.

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| 키 | 위치 | 동작 |
|---|---|---|
| Tab | 툴팁이 있는 컨트롤 | 포커스가 툴팁을 표시하고, 벗어나면 숨깁니다 |
| Escape | 툴팁이 표시된 상태의 어디서나 | 툴팁을 숨깁니다. 포커스는 유지됩니다 |
| Enter, Space | 팝오버 버튼 | 팝오버를 엽니다. 포커스가 첫 번째 컨트롤로 이동합니다 |
| Escape | 팝오버 안 | 닫습니다. 포커스가 버튼으로 돌아옵니다 |
| Tab | 팝오버 안 | 컨트롤 사이를 이동합니다. 벗어나면 닫힙니다 |

세 가지를 합쳐 gzip 기준 약 2.2 KB의 스크립트가 필요하고 자체 요청은 없습니다. 툴팁은 노드 1개, 팝오버는 6개와 그 내용, 사용 중 오버레이는 3개입니다.

## 드롭다운과 분할 버튼

`MenuButton::new(label)`은 명령 메뉴를 엽니다. `MenuButton::split(label, action, input)`은 주 부분에서 action 하나를 실행하고 화살표에서 메뉴를 엽니다. 명령은 `.action(label, action, input)`(또는 성공 후 effect를 실행하는 `.action_then(...)`), 창 열기 같은 effect를 위한 `.item(label, effect)`, `.separator()`입니다. `.confirm(text)`는 마지막 action 명령을 실행하기 전에 확인을 묻습니다.

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

action 명령은 자체 경로를 가진 노드이며 `ActionButton`처럼 폼 엔드포인트로 전송됩니다. Screen의 접근 권한, Feature의 가드, action의 권한, 인터셉터, CSRF, 쓰기 속도 제한이 호출할 때마다 검사되고 입력은 서버에 남습니다. 사용자가 실행할 수 없는 명령은 메뉴에서 빠지며(분할 버튼의 주 action은 대신 비활성화됩니다), 호출되더라도 서버가 거부합니다.

@live menus

| 키 | 위치 | 동작 |
|---|---|---|
| Enter, Space, 아래쪽 화살표 | 버튼 | 첫 번째 명령에서 메뉴를 엽니다 |
| 위쪽 화살표 | 버튼 | 마지막 명령에서 메뉴를 엽니다 |
| 아래쪽 화살표, 위쪽 화살표 | 메뉴 | 다음 명령과 이전 명령으로 이동하며 순환합니다 |
| Home, End | 메뉴 | 첫 번째와 마지막 명령 |
| 문자 | 메뉴 | 그 문자로 시작하는 다음 명령 |
| Enter, Space | 명령 | 명령을 실행하고 메뉴를 닫습니다 |
| Escape | 메뉴 | 닫습니다. 포커스가 버튼으로 돌아옵니다 |
| Tab | 메뉴 | 닫고 다음으로 이동합니다 |

메뉴에는 gzip 기준 약 0.8 KB의 스크립트가 필요하며 명령이 실행되기 전까지 요청이 없습니다. 노드는 4개에 명령당 2개입니다.
