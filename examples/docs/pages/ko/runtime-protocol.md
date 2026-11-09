# 런타임 프로토콜

상태: 초안이며 `zeyn::server`와 `assets/zeyn/*.js`(`/_zeyn/zeyn.js`로 제공됨)로 구현되어 있습니다.

이것은 Zeyn 서버와 브라우저 런타임 사이의 계약입니다. Screen은 서버에서 렌더링됩니다. 브라우저 런타임은 사용자의 동작을 서버로 보내고 돌아온 것을 적용하는 작은 스크립트입니다. 애플리케이션 코드는 JavaScript를 작성하지 않습니다.

## 1. 페이지 로드

`GET {route}?params={json}`은 전체 HTML 문서를 반환합니다. `params`는 Screen 매개변수의 URL 인코딩된 JSON이며, Screen에 매개변수가 없으면 생략됩니다.

`/orders/{id}`처럼 경로 세그먼트가 있는 경로는 해당 params 필드를 경로(`/orders/42`)에서, 나머지 필드를 `params`에서 읽습니다. 파싱되지 않는 세그먼트 값은 404로 응답합니다. 값은 점도 포함하여 퍼센트 인코딩되므로(`%2E`) Screen URL이 런타임에 파일처럼 보이는 일이 없습니다. `Screen::moved_to`로 응답하는 Screen은 `Location`에 새 URL이 있는 `301 Moved Permanently`를 받고, `Screen::redirect`로 응답하는 Screen은 `Cache-Control: no-store`가 있는 `303 See Other`를 받습니다. `nav.js`는 둘 다 페이지 로드 없이 따라가며, 로드가 기록 항목을 추가하지 않는 경우(뒤로 가기와 앞으로 가기)에도 대상의 주소를 표시합니다.

실패한 페이지는 해당 상태(404, 403, 500, ...)와 셸 안의 앱 오류 Screen(`App::not_found`, `App::server_error`) 또는 짧은 내장 메시지로 응답합니다. `x-zeyn-partial: 1`이 있는 요청은 `x-zeyn-partial: 1`과 `x-zeyn-title`이 있는 그 Screen만 받으므로 런타임이 페이지 로드 없이 이를 교체합니다. 오류의 세부 정보는 브라우저에 도달하지 않습니다.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

메뉴에는 현재 사용자가 열 수 있는 Screen만 나열됩니다. 현재 항목에는 `aria-current="page"`가 있습니다.

head의 모든 프레임워크 스크립트와 스타일시트에는 `?v=`와 빌드 id(`meta[name=zeyn-build]`, `ZEYN_BUILD_ID` 또는 실행 파일의 해시)가 붙고, `theme.css`에는 테마 텍스트의 해시가 붙습니다. 현재 값을 가진 요청은 `Cache-Control: public, max-age=31536000, immutable`을 받으므로 웜 페이지 로드는 그중 어느 것도 요청하지 않습니다. 그 밖의 요청(배포 후의 오래된 탭, `v` 없음)은 `no-cache`와 ETag를 받습니다. 런타임이 나중에 가져오는 모듈은 자신을 가져오는 스크립트의 URL에서 같은 `v`를 이어받습니다.

### 1.1 Browser modules

페이지는 화면이 쓰는 브라우저 코드만 불러와 실행합니다. 모든 페이지는 코어를 실행합니다: `zeyn.js`(요청, 효과, 창, 폼, 메뉴, 단축키, 이벤트 위임, 모듈 로더), `nav.js`, `enterprise.js`(`fetch`의 응답 훅과 세션 유휴 시간 제한)로, 합쳐서 원본 90 KB, gzip 29 KB입니다. 나머지는 모두 컴포넌트의 모듈로, `/_zeyn/{name}.js`와 일부는 `/_zeyn/{name}.css`입니다. `crates/zeyn/src/server/modules.rs`의 `MODULES`에 HTML이 지닌 표식과 함께 선언됩니다: 속성 이름(`data-zeyn-date`)과 클래스(`z-select`)이며, 이름 전체로 비교합니다.

| 모듈 | 컴포넌트 |
|---|---|
| `components.js` | 모든 컨트롤 모듈이 import하는 공유 부분: 도우미, 팝업과 리스트박스 코어, 셀렉트(앱 셸의 선택기), 확인 대화상자, 알림, 이동 경로 |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js` | 패널, 탭, 분할 창; 날짜와 시간 선택기(grid 날짜 필터 포함); 리스트박스, 조회, 다중 선택; 숫자, 늘어나는 텍스트 영역, 마스크, 비밀번호; 줄 편집기; 단계별 폼; 트리; 툴팁과 팝오버; 메뉴 버튼과 분할 버튼; 처리 중 오버레이 |
| `grid.js` | grid. `zeyn.js`가 직접 import하므로 `<link rel="modulepreload">`로 링크됩니다 |
| `palette.js`, `pro.js` | 명령 팔레트와 명령 버튼; 메뉴 막대, 도구 상자, 상태 표시줄, 도킹 패널, 속성 검사기 |
| `forms.js`, `reveal.js`, `import.js` | 폼의 초안과 워크플로 전환; 가려진 값 보기; 가져오기 |
| `catalog.js`, `media.js`, `files.js`, `charts.js` 및 스타일시트 | 카드 그리드, 레일, 검색 상자; 플레이어, 임베드, 이미지; 업로드; 차트, 스파크라인, KPI 카드, 불릿 차트 |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | 공유 버튼, 필터 칩, 반응, 히어로 슬라이드, 환경 설정 저장소, 카드 미리보기(처음 마우스를 올릴 때), 어시스턴트(채팅 패널 또는 런처를 처음 누를 때) |
| `topics.js`, `vlist.js` | 토픽을 listen하는 노드와 이를 drive하는 컨트롤(`data-zeyn-listens`, `data-zeyn-drives`), 가상 목록(`data-zeyn-vlist`) |
| `inbox.js`, `kanban.js`, `calendar.js` and their stylesheets, `diff.css`, `code.css`, `barcode.css` | 받은 편지함과 알림 종; Kanban 보드; 달력; 스크립트가 필요 없는 비교, 코드 보기, 바코드(코드 보기의 복사 버튼은 `share.js`에 있음) |
| `range.js`, `quantity.js`, `gallery.js`, `countdown.js`, `commerce.css` | 범위 슬라이더, 수량 선택기, 상품 갤러리, 카운트다운; 모든 쇼핑 컴포넌트의 스타일시트(가격, 옵션 선택기, 패싯 패널, 리뷰 요약) |
| `thread.js` 및 `thread.css`, `stream.js`, `composer.js` 및 `composer.css`, `typing.js` 및 `typing.css` | 메시지 스레드; 스레드와 어시스턴트가 함께 쓰는 스트리밍 Markdown과 JSON 줄; 채팅 composer; 입력 중 표시기 |
| `feed.js`, `lightbox.js` and their stylesheets, `stories.css` | 게시물 피드; 이미지와 동영상 라이트박스; 스토리 링과 그 행, 스크립트 필요 없음 |

전체 페이지는 head에 모든 페이지가 쓰는 스타일시트(`zeyn.css`, `components.css`, `content.css`, `pro.css`, `enterprise.css`), HTML에 표식이 있는 모듈의 스타일시트, 그리고 그 스크립트를 import하는 모듈 뒤에 링크합니다. `zeyn.js`는 같은 표(`[name, script selector, style selector]`)와 함께 제공되며 나중에 도착하는 콘텐츠에 빠진 것을 불러옵니다: `nav.js`는 화면을 바꾸기 전에 `{ root, waits }`와 함께 `zeyn:need`를 보내고 새 스타일시트를 기다리며, 창도 열리기 전에 같은 방식으로 기다리고, `zeyn:content`, grid 페이지, grid 필터는 담고 있는 것의 스크립트를 불러옵니다. 브라우저는 URL마다 인스턴스를 하나만 유지하므로 모듈은 한 번만 실행됩니다. 모듈은 실행될 때 문서에서 찾은 것을 설정하고 그 뒤에는 `zeyn:content`를 듣습니다. 페이지에 없는 컴포넌트를 찾거나 듣지 않습니다. 모듈은 자신의 URL에 있는 `?v=`로 공유 부분을 import합니다. `tests/ui_modules.rs`는 페이지가 쓰지 않는 모듈을 링크하거나 모듈 스타일시트가 표식 밖의 클래스를 꾸밀 때 실패합니다.

showcase에서 측정(원본, gzip 전송량): 텍스트만 있는 페이지는 스크립트를 269 KB(80 KB) 대신 132 KB(46 KB), grid 페이지는 357 KB(106 KB) 대신 223 KB(74 KB), 폼 페이지는 269 KB(80 KB) 대신 186 KB(65 KB) 불러오고, 모든 페이지의 스타일은 177 KB(44 KB) 대신 151 KB(35 KB)입니다. 컴포넌트 카탈로그처럼 모든 컴포넌트를 담은 페이지는 모듈마다 파일이 따로 있으므로 이전보다 많은 파일을 불러옵니다(요청 41개 대신 68개, 원본 774 KB 대신 836 KB).

## 2. Screen 마크업

모든 Screen 루트는 자신의 이름과 매개변수를 가집니다.

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

창은 `class="z-window" role="dialog" aria-modal="true"`와 `--z-width` 스타일을 사용합니다.

각 본문 항목은 노드 요소로 감싸집니다. 그 경로는 아래 엔드포인트가 사용하는 주소입니다. 컨테이너(`Row`, `Stack`, `Panel`, `Card`, `Tabs`, `Split`)는 각 자식을 같은 방식으로 감싸며, 자식의 경로에는 그 인덱스가 추가됩니다. 본문 항목 1의 첫 번째 자식의 세 번째 자식은 `"1.0.2"`입니다.

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

엔드포인트는 `node`를 점으로 구분한 문자열로, 본문 항목에는 일반 숫자(`0`)로 받습니다. 경로는 숫자 최대 16단계이며, 그 밖의 것은 `422`로 거부됩니다.

## 3. 런타임이 읽는 속성

| 속성 | 대상 | 의미 |
|---|---|---|
| `data-zeyn-effect` | button, `tr` | 클릭 시 실행할 JSON effect (행: 더블 클릭 또는 Enter) |
| `aria-keyshortcuts` | button | 키보드 단축키, 예: `Control+N` |
| `data-zeyn-export` | button | 같은 Screen에 있는 그리드의 내보내기 형식 (`csv`, `xlsx`) |
| `data-zeyn-grid` | 그리드 루트 | 그리드를 표시합니다. `data-zeyn-mode`, `data-zeyn-paging`, `data-zeyn-page-size`, `data-zeyn-total`, `data-zeyn-next`, `data-zeyn-offset`도 있습니다 |
| `data-field`, `aria-sort`, `aria-colindex` | `th` | 열 필드 이름, 현재 정렬, 위치 |
| `data-filter`, `data-options` | `th` | 필터 입력 종류(`text`, `number`, `money`, `date`, `datetime`, `time`, `choice`, `bool`)와 choice 옵션 `[{value,label}]` |
| `data-pin` | `th` | 고정된 열의 `start` 또는 `end` (선택 열과 행 action 열도 포함) |
| `data-edit`, `data-optional` | `th` | 저장할 수 있는 사용자를 위한 편집 가능한 열의 입력 종류 |
| `data-zeyn-texts`, `data-zeyn-columns`, `data-zeyn-view` | 그리드 루트 | 현지화된 컨트롤 텍스트, 모든 열 `[{field,label,hidden,pin,width}]`, 저장된 뷰 |
| `data-zeyn-views` | 그리드 루트 | 뷰가 서버에 저장될 때 `server` |
| `data-zeyn-version-field`, `data-zeyn-exportable` | 그리드 루트 | 인라인 편집을 위한 행 버전 필드 이름. 그리드를 내보낼 수 있으면 `csv` |
| `data-zeyn-key`, `data-zeyn-version`, `data-zeyn-menu` | `tr` | 행 키, 행 버전, 컨텍스트 메뉴 항목 |
| `data-v` | `td` | 편집 가능한 셀의 입력 값 |
| `data-zeyn-detail`, `aria-expanded` | `tr` | `{screen, params}` 형태의 행의 디테일 Screen. 열려 있는지 여부 |
| `data-zeyn-expand` | 행 안의 button | 행의 디테일, 그룹, 자식을 열거나 닫습니다 |
| `data-zeyn-level`, `aria-level`, `data-zeyn-path` | `treegrid`의 `tr` | 그룹화되거나 트리 그리드에서의 깊이(맨 위가 0). 행이 열릴 때 보낼 그룹 값 또는 상위 키의 경로 |
| `z-group` | `tr` | 그룹 헤더 행. `z-branch-more`는 가지의 "Load more" 행 |
| `data-zeyn-run` | 행 안의 button | 실행할 행 항목 (`a0`은 첫 번째 행 action) |
| `data-agg` | `tfoot td` | 열이 표시하는 합계 (`sum avg min max count`) |
| `data-zeyn-bulk` | 그리드 앞의 div | `data-zeyn-bulk-run` 버튼이 있는 선택 막대 |
| `data-zeyn-menu` | Screen 루트 | Screen의 컨텍스트 메뉴 |
| `data-zeyn-native-menu="off"` | `html` | 텍스트 입력을 제외하고 브라우저의 컨텍스트 메뉴가 꺼집니다 |
| `data-zeyn-rows` | `tbody` | 행이 들어가는 곳 |
| `data-zeyn-page` | 페이저 버튼 | `prev` 또는 `next` |
| `data-zeyn-page-info` | span | 그리드 루트의 현지화된 템플릿으로 만든 `1-50 of 230` 같은 텍스트 |
| `data-zeyn-text-range`, `data-zeyn-text-range-total`, `data-zeyn-text-loaded` | 그리드 루트 | 현지화된 템플릿: `{from}-{to}`, `{from}-{to} of {total}`, `{count} rows loaded` |
| `data-zeyn-more` | div | 무한 스크롤 센티널이며 "Load more" 버튼을 담습니다 |
| `data-zeyn-submit` | form | 제출 action의 이름 |
| `data-zeyn-editor` | input | 사용자 지정 필드 타입의 클라이언트 측 편집기 |

Effect:

| Effect | JSON | 런타임이 하는 일 |
|---|---|---|
| 창 열기 | `{"effect":"open_window","screen":"product_form","params":{...}}` | Screen(4.1절)을 모달 `<dialog>`에 불러옵니다 |
| 이동 | `{"effect":"navigate","screen":"...","params":{...}}` | `/_zeyn/route/{screen}?params=`를 불러오며, 이것이 페이지로 리다이렉트합니다. 경로에 세그먼트가 있으면 경로 형태로 리다이렉트됩니다 |
| 토스트 | `{"effect":"toast","message":"Saved"}` | 토스트를 4초 동안 표시합니다 |
| 닫고 새로 고침 | `{"effect":"close_and_refresh"}` | 맨 위 대화 상자를 닫고 그 아래 Screen의 모든 그리드를 다시 불러옵니다 |

## 4. 엔드포인트

모든 `POST` 요청은 JSON을 보내며, `zeyn-csrf` meta 태그의 토큰이 담긴 `x-zeyn-csrf` 헤더가 있어야 하고, 그렇지 않으면 `403`으로 거부됩니다.

세션 없이 열린 공개 Screen은 캐시할 수 있도록 비어 있는 `zeyn-csrf` meta 태그와 `Set-Cookie` 없이 제공됩니다. 첫 `POST` 전에 런타임은 `GET /_zeyn/csrf`를 호출하며, 이는 `Cache-Control: no-store`와 함께 `{ "token": "..." }`로 응답하고, 브라우저에 쿠키가 없으면 `zeyn_csrf` 쿠키를 설정하며, `Sec-Fetch-Site`가 `same-origin`이 아닌 요청을 거부합니다. 토큰은 meta 태그에 들어갑니다. 호출이 실패하면 `POST`는 토큰 없이 나가며 다른 요청처럼 거부됩니다. 조용히 재시도하는 일은 없습니다.

앱 내 탐색(`nav.js`)은 `x-zeyn-partial: 1`과 함께 Screen을 요청하고 `x-zeyn-title`에 페이지 제목이 있는 HTML만 받습니다. Screen이 head 태그(설명, 정규 URL, 미리 보기 이미지, robots, JSON-LD)를 설정하면 본문은 이를 담은 `<template data-zeyn-head>`로 시작합니다. `nav.js`는 문서 head의 모든 `[data-zeyn-head]` 요소를 제거하고, 템플릿의 태그를 그곳으로 옮기고, 템플릿을 버립니다. head 태그가 없는 Screen은 템플릿을 보내지 않으므로 이전 태그만 제거됩니다.

### 4.1 Screen 조각

`GET /_zeyn/screen/{screen}?params={json}`은 페이지 셸 없이 Screen HTML만 반환합니다. 창과 행 디테일에 사용됩니다. `data-zeyn-detail`이 있는 행을 열면 그 Screen이 바로 아래의 새 `tr.z-detail`(행 전체에 걸친 셀 하나이며 `role="region"`을 담음)에 불러와지고, 닫으면 그 행이 제거됩니다. Screen의 권한은 다른 Screen처럼 검사되며, 디테일 안의 그리드는 디테일 Screen 자체의 이름과 매개변수로 서버와 통신합니다.

### 4.2 그리드 행

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

필터는 `contains`, `starts_with`, `equals`, `from`, `to`, `any_of`, `is`를 설정할 수 있습니다(`docs/grid.md` 참고). `view.columns`는 행을 렌더링할 열 레이아웃이며, `head: true`이면 응답에 헤더 행인 `head`도 있습니다.

무한 스크롤에서 `page`는 `{ "type": "after", "cursor": "...", "limit": 100 }`이고 첫 번째 묶음에서 `cursor`는 `null`입니다.

응답 `200`:

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

`foot`는 합계를 요청했을 때의 합계 행이며, 그렇지 않으면 `null`입니다.

`total`과 `next`는 `null`일 수 있습니다. 런타임은 페이지 방식 그리드에서는 행을 교체하고 무한 스크롤에서는 추가합니다.

그룹화된 그리드와 트리 그리드(`<table role="treegrid">`)는 가지를 불러오기 위해 `POST /_zeyn/grid`와 함께 `path`를 보냅니다. 열리는 행의 `data-zeyn-path`에 있는 그룹 값(또는 상위 키)입니다. 응답은 같은 형태이며, `rows`에는 그룹 행 또는 한 단계 더 깊은 행이 들어 있고 `total`은 이를 셉니다. 런타임은 이를 열린 행 뒤에 넣고, 행이 닫히면 그 뒤에 오는 더 깊은 행을 모두 제거합니다. 그리드의 그룹 수준(트리에서는 32)보다 긴 경로는 `404`입니다.

### 4.2.1 그 밖의 그리드 엔드포인트

| 엔드포인트 | 본문 | 기능 |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | 사용자의 뷰를 저장합니다(`null`은 삭제). `204`. 뷰 스토어가 없으면 `404` |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | 행 항목을 실행합니다(`a0`은 행 action, `m2.1`은 메뉴 경로). 서버가 키로 행을 불러오고 입력을 만듭니다. `{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | 선택한 키로, 또는 `all`이면 `query.filters`와 일치하는 모든 행으로 일괄 action을 실행합니다. `{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | 인라인 편집을 저장합니다. `values`는 폼이 보내는 것처럼 편집된 열과 행 버전을 담습니다. `{row}`. 편집할 수 없는 열은 `403`, `409`, `422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | 일치하는 모든 행을 CSV로 스트리밍합니다 |
| `POST /_zeyn/menu` | `{screen, params, item}` | Screen의 컨텍스트 메뉴의 action 항목을 실행합니다 |

### 4.2.2 가상 스크롤

`data-zeyn-paging="virtual"`이 있는 그리드는 행이 아무리 많아도 모든 행에 걸친 하나의 스크롤 가능한 목록을 보여 줍니다. 서버는 `data-zeyn-total`과 함께 첫 번째 블록(`data-zeyn-page-size`행, 오프셋 0)을 렌더링합니다. 처음 64행은 테이블에, 나머지는 그 뒤의 `<template data-zeyn-rows-later>`에 들어가며, 그리드가 40개 이상의 열을 보여 줄 때는 모든 행이 템플릿에 들어갑니다. 그리드 루트에는 `data-zeyn-row-height`(픽셀)와 CSS 변수 `--z-row-height`도 있습니다.

런타임은 다음을 합니다.

- 테이블 본문을 스크롤 컨테이너에 넣으며, 그 내부 스페이서의 높이는 `total * rowHeight` 픽셀이고 8,000,000 px로 제한됩니다. 상한을 넘으면 스크롤 위치가 비율로 행 인덱스에 매핑되므로(`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`) 1,000,000행도 끝까지 도달합니다.
- 보이는 행과 위아래 20행만 DOM에 유지하며 `transform: translateY(...)`로 배치합니다. 행의 높이는 고정이며 긴 텍스트는 줄임표로 잘립니다.
- `POST /_zeyn/grid`와 `page: { "type": "offset", "offset": blockStart, "limit": blockSize }`로 `data-zeyn-page-size`행의 블록을 불러오며, 첫 호출 이후에는 `with_total: false`입니다. 최대 20개 블록과 최대 100,000개 셀을 캐시에 유지하고(가장 오래 사용하지 않은 것부터 제거. 63개 열 그리드는 7개 블록이며 3개보다 적지 않음), `reload`를 위해 보관하는 블록도 같은 상한에 포함되며 가장 먼저 버려지고, 스크롤 방향의 다음 블록을 미리 가져오며, 뷰를 벗어난 블록의 요청은 취소합니다(`AbortController`).
- 블록을 불러오는 동안 보이는 행을 절대 지우지 않습니다. 없는 행은 해당 블록이 도착할 때까지 `aria-busy="true"`가 있는 자리 표시자 행을 표시하므로 화면이 깜박이지 않습니다.
- 정렬과 필터가 바뀌면 캐시를 버리고 맨 위로 스크롤합니다.
- 키보드: 화살표 키, Page Up, Page Down, Home, End는 활성 행을 이동하고 보이는 곳으로 스크롤합니다. 테이블에는 `aria-rowcount` = total이, 각 행에는 `aria-rowindex`가 있습니다.
- 위치 텍스트(`data-zeyn-page-info`)는 `data-zeyn-text-range-total`을 사용합니다.

카드 그리드(`zeyn::ui::CardGrid`이며 rail이나 행 목록은 해당하지 않음)는 카드가 200개가 되면 스크롤 컨테이너 대신 페이지 자체의 스크롤에 윈도우를 적용합니다. `catalog.js`는 목록에 `data-zeyn-window`를 표시하고, 뷰포트에서 800 px 이내의 행을 그 안에 유지하며 위아래 행은 패딩으로 바꾸고, 모든 행에 처음 카드들 중 가장 높은 카드의 높이를 주며(`grid-auto-rows`), 각 카드에 `aria-posinset`과 `aria-setsize`(페이지가 더 올 수 있는 동안은 `-1`)를 설정합니다. 윈도우 밖에 있는 포커스된 카드는 `z-cards-kept`와 함께 DOM에 남아 자기 셀 위에 배치됩니다. "더 보기"나 다른 페이지에서 온 카드도 같은 윈도우에 합류합니다. 페이지가 ms당 3 px보다 빠르게 스크롤되는 동안 카드는 이미지를 보류한 채 들어오고(`data-zeyn-park`, `src`와 `srcset`은 따로 보관) 120 ms 동안 스크롤이 없으면 이미지를 되찾습니다.

### 4.3 폼 제출

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

값은 사용자가 입력한 그대로 전송됩니다. 입력과 선택 목록은 문자열, 체크박스는 불리언입니다. 서버는 필드 타입에 따라 이를 파싱합니다.

서버는 Screen이 불러온 모델에서 시작하여 폼에 있고 읽기 전용이 아닌 필드만 덮어씁니다. 클라이언트의 다른 값은 무시됩니다. 키는 클라이언트에서 오지 않습니다.

응답:

- `200 { "effects": [ ... ] }`: 저장됨. effect를 순서대로 실행합니다.
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`: 각 메시지를 해당 필드 아래(`#{scope}-{field}-error`)에 표시하고, 입력에 `aria-invalid="true"`를 설정하고, 첫 번째 잘못된 입력에 포커스를 둡니다. `field`가 빈 오류는 폼 맨 위에 표시됩니다.

### 4.4 오류

모든 엔드포인트는 다음으로 응답할 수 있습니다.

| 상태 | 본문 | 의미 |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | 권한 없음, 가드가 거부함, 또는 잘못된 CSRF 토큰 |
| `404` | `{ "error": "not_found", "message": "..." }` | 알 수 없는 Screen 또는 노드 |
| `409` | `{ "error": "conflict", "message": "..." }` | 다른 사람이 레코드를 변경함 |
| `422` | 4.3 참고 | 검증 |
| `504` | `{ "error": "timeout", "message": "..." }` | 호출이 서버 제한 시간보다 오래 걸림 |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | 세부 정보는 서버 로그에만 있음 |

런타임은 `message`를 오류 토스트로 표시합니다.

### 4.5 언어

`{ "locale": "tr" }`와 함께 `POST /_zeyn/locale`은 선택을 `zeyn_locale` 쿠키에 저장하고 `204`로 응답합니다. 런타임은 페이지를 새로 고칩니다. 쿠키가 없으면 서버는 `Accept-Language`에서, 그다음 앱 기본값에서 고릅니다. `<html lang>`은 항상 선택한 로케일과 일치합니다.

셸은 앱에 로케일이 둘 이상이면 언어 선택기(`select[data-zeyn-locale]`)를 렌더링합니다.

### 4.6 테마

셸은 앱이 이를 숨기거나 테마를 하나만 제공하지 않는 한 테마 선택기(`select[data-zeyn-theme-picker]`)를 렌더링합니다. 각 옵션에는 `data-scheme`(`light`, `dark`, 시스템은 비어 있음)과 앱 테마의 `data-custom`이 있습니다. 변경하면 런타임이 `<html>`의 `data-theme`과 `data-zeyn-theme`, 그리고 `color-scheme` meta를 즉시 설정한 다음 `{ "theme": "dark" }`와 함께 `POST /_zeyn/theme`을 보냅니다. 서버는 `zeyn_theme` 쿠키와 함께 `204`로 응답하며, 앱이 제공하지 않는 테마에는 `404`로 응답합니다. 페이지는 그 쿠키의 테마로 렌더링되므로 새로 고침도 깜박임도 없습니다. `docs/theming.md`를 참고하십시오.

### 4.7 엔터프라이즈 엔드포인트

`enterprise.js`와 `enterprise.css`는 런타임 옆에 로드됩니다. 자세한 내용은 `docs/enterprise.md`에 있습니다.

| 엔드포인트 | 용도 |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | 마스킹된 필드의 원본 값 `{value}`. 마스킹 해제 권한이 필요하며 항상 감사됩니다 |
| `POST /_zeyn/reason {screen, reason}` | `reason`으로 표시된 Screen의 사유. `HttpOnly` 쿠키와 함께 `204` |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV 미리 보기(`commit: false`) 또는 가져오기 |
| `POST /_zeyn/keepalive` | 유휴 경고 후의 "로그인 유지". `204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | 라이브니스와 준비 상태. health는 속도 제한이 없고, 준비 상태는 `probe_ips`에서만 제외됩니다 |
| `GET /_zeyn/jobs` | 작업 권한이 있는 사용자를 위한 작업 상태 |

모든 응답에는 `x-zeyn-build`가 붙습니다. 런타임이 처리하는 거부: `428 reason_required`(사유를 묻고 요청을 반복)와 `401 reauth_required`(사용자에게 다시 로그인하도록 요청하고 요청을 반복).

## 5. 보안

- 모든 응답은 `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin`을 설정합니다. `App::csp`는 지정한 `https` 호스트를 `img-src`, `media-src`(와 `blob:`), `frame-src`, `connect-src`, `font-src`에 추가하며, 다른 지시문은 절대 바뀌지 않습니다. 헤더 텍스트는 시작할 때 한 번 만들어집니다.
- 인라인 스크립트나 인라인 이벤트 핸들러는 없습니다. 인라인 `style` 속성은 CSS 사용자 지정 속성(`--z-width`, `--z-flex`)과 고정된 열 너비에만 사용되므로 정책은 `style-src-attr 'unsafe-inline'`을 추가합니다. 스크립트는 엄격하게 유지됩니다.
- CSRF 토큰은 `HttpOnly`, `SameSite=Strict` 쿠키에 저장되며 `x-zeyn-csrf` 헤더와 일정한 시간으로 비교됩니다.
- 페이지, `totp` 폼, `GET /_zeyn/csrf`는 토큰을 쿠키에 있는 그대로 싣지 않습니다. 응답마다 새로운 무작위 패드 뒤에 토큰과 패드의 XOR(16진수 128자리)을 붙이므로 gzip이 토큰을 유출할 수 없습니다(BREACH). 서버는 비교하기 전에 마스킹을 풀며 일반 64자리 토큰도 여전히 받아들입니다.
- 1 KB 이상의 동적 응답(페이지, 부분 Screen, 그리드 블록, 내보내기)은 브라우저가 받아들이면 레벨 2로 gzip 압축되고 `Vary: Accept-Encoding`이 붙습니다. 압축하지 않는 것: 이미 `Content-Encoding`이 있는 응답(한 번 압축해 캐시하는 프레임워크 에셋), 범위 응답(`Content-Range`와 `Range`가 있는 모든 요청), 비디오, 오디오, SVG 외의 이미지, `application/octet-stream`, 아카이브와 PDF, `text/event-stream`, `application/x-ndjson`(어시스턴트의 스트림), WebSocket 업그레이드.
- 각 호출은 순서대로 인증, Screen의 접근 권한, Feature의 가드, 노드 action의 접근 권한을 검사한 다음 인터셉터와 action을 실행합니다.

## 6. 런타임 동작

- JavaScript가 없어도 페이지는 모든 그리드의 첫 페이지와 함께 렌더링됩니다. 폼과 페이징에는 런타임이 필요합니다.
- 키보드: 화살표 키는 그리드 행 사이를 이동하고, Enter는 행을 열고, Escape는 맨 위 대화 상자를 닫으며, `aria-keyshortcuts`의 단축키는 텍스트 입력 안을 제외한 모든 곳에서 동작합니다. 그리드에서: Space는 행을 선택하고, F2는 편집하며, Ctrl+Z는 마지막 편집을 되돌리고, Shift+F10 또는 컨텍스트 메뉴 키는 행이나 헤더의 메뉴를 열고, Shift+클릭은 여러 열로 정렬하며, 오른쪽과 왼쪽 화살표(오른쪽에서 왼쪽 페이지에서는 반전)는 행의 디테일, 그룹, 자식을 열고 닫고, 자식 행에서의 왼쪽 화살표는 부모로 이동합니다. 행에서의 오른쪽 화살표는 그 셀로 들어갑니다. 화살표, Home, End, Ctrl+Home, Ctrl+End, Tab, Shift+Tab은 셀 커서를 이동하고, Shift는 범위를 넓히며(셀의 `aria-selected`), Ctrl+C는 범위를 탭으로 구분된 텍스트로 복사하고, Ctrl+V는 편집 가능한 셀에 붙여넣으며, 바뀐 행마다 `POST /_zeyn/grid/edit` 하나가 나갑니다. 열이 40개 이상인 그리드는 보이는 열만 페이지에 유지합니다(고정 너비의 `colgroup`, `colspan`이 있는 `td.z-col-gap` 스페이서, 셀의 `aria-colindex`).
- 오른쪽 클릭은 정의된 곳(헤더, `data-zeyn-menu`가 있는 행, `data-zeyn-menu`가 있는 Screen)에서 Zeyn의 메뉴를 열며, 그 밖의 모든 곳에서는 브라우저 자체의 메뉴가 유지됩니다.
- 대화 상자가 열릴 때 포커스가 그 안으로 이동하고 닫힐 때 연 요소로 돌아갑니다.
- 클릭한 프레임에서 바로 대기 상태를 보여 줍니다: 화면을 불러오는 동안 `nav.js`가 대상 링크를 현재 항목으로 표시하고 `html[data-zeyn-loading]`을 설정하며, 창과 폼 제출도 이를 설정하고, grid는 정렬, 페이지, 필터 키를 누르는 즉시 행을 흐리게 합니다(`aria-busy`)(필터는 다음 키를 150 ms 기다리고, 새 요청이 이전 요청을 취소합니다). 창 위쪽의 얇은 막대는 응답이 150 ms보다 오래 걸릴 때만 나타납니다. 고정되어 있어 아무것도 움직이지 않으며 `prefers-reduced-motion`에서는 움직이지 않습니다.
- 클라이언트 측 검사(`required`, `minlength`, `maxlength`, `min`, `max`, `type="email"`)는 제출 전에 브라우저의 제약 조건 검증 API로 실행됩니다. 서버는 항상 다시 검증합니다.

## 7. 컴포넌트 라이브러리

`zeyn::ui` 컴포넌트는 1.1절의 모듈과 스타일시트 `/_zeyn/components.css`로 제공됩니다. 런타임처럼 로그인이 필요 없습니다.

### 7.1 엔드포인트

모두 JSON과 CSRF 헤더가 있는 `POST`이며, 먼저 Screen의 접근 권한과 Feature의 가드를 검사합니다.

| 엔드포인트 | 본문 | 응답 |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | 그리드의 첫 페이지가 포함된 노드 하나의 HTML. 지연 탭에서 사용됩니다. |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`: Screen을 한 번 실행해 렌더링한 여러 노드로, 새로 고쳐지거나 drive된 토픽의 리스너를 위한 것입니다. 서로 다른 경로는 최대 32개이며, Screen에 없는 경로는 `404`로 응답합니다. Screen의 접근 권한, guard, interceptor는 한 번만 실행되고, params는 Screen의 타입이 있는 params로 읽힙니다. |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | 노드의 데이터 소스 `key`에서 온 JSON: lookup 옵션(`key`는 필드 이름), 트리 자식(`key`는 비어 있음), 단계 검사(`key`는 `_step`). 소스 action의 권한이 검사되고 그리드와 마찬가지로 인터셉터가 실행됩니다. action에 맞지 않는 입력은 `404`로 거부됩니다. |
| `screen` = `_site.{feature}`와 함께 `/_zeyn/data` | 위와 같음 | 사이트 헤더 슬롯의 컴포넌트에 대해서도 같습니다. `node`는 슬롯(0 start, 1 center, 2 end, 3 bottom), 그 안에서의 노드 인덱스, 그리고 자식 인덱스들입니다. Feature의 가드와 action의 권한이 검사됩니다. |
| `/_zeyn/parse` | `{ kind, text, label }` | 서버가 입력된 날짜나 시간을 사용자의 언어, 시간대, 시계로 읽는 방식: `{ ok, text, iso, utc, zone }` 또는 `{ ok: false, message }`. `kind`는 `date`, `time`, `datetime`, `local`, `presets`(오늘과 빠른 범위)입니다. |

### 7.2 폼이 제출하는 값

- lookup은 id를 텍스트로 제출합니다. `"7"`, 여러 개이면 `"3,7,9"`.
- 라인은 `"{field}": "rows"`와 셀당 하나의 값을 `"{field}.{line}.{column}"`으로, 그리고 행이 유래한 불러온 라인의 인덱스인 `"{field}.{line}._i"`(새 라인은 비어 있음)를 제출합니다. 서버는 각 라인을 불러온 라인에서 다시 만들므로 표시되고 편집 가능한 열만 바뀝니다.
- 라인의 검증 오류는 `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }`를 사용합니다.

### 7.3 이벤트

| 이벤트 | 대상 | 상세 |
|---|---|---|
| `zeyn:content` | `document` | 새 콘텐츠가 있는 요소(탐색, 지연 패널). 런타임이 필요한 모듈을 불러오고, 불러온 각 모듈이 그 안의 것을 설정합니다. |
| `zeyn:errors` | 폼 | 런타임이 표시한 후의 `422` 오류. 라인 편집기와 단계별 폼이 사용합니다. |
| `zeyn:effect` | `document` | 런타임이 모르는 effect: `{ effect, source, run }`. `run(next)`는 다른 effect를 실행합니다. `confirm`이 사용합니다. |
| `zeyn:topics` | `document` | `{ topics }`: 다른 스크립트(알림을 받은 `live.js`)에서 보낸, 새로 고칠 토픽. |
| `zeyn:reload` | 요소 | 그 안의 그리드를 제자리에서 다시 불러옵니다. 현재 페이지, 또는 `{ reset: true }`이면 첫 페이지입니다. |
| `zeyn:density` | `document` | 없음. 사용자가 계정 메뉴에서 다른 밀도를 골랐습니다(`<html>`의 `data-density`는 이미 바뀌었습니다). 윈도잉된 카드 그리드가 행을 다시 측정합니다. |

Effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}`는 모달 알림 대화 상자에서 묻고 사용자가 확인한 후에만 `then`을 실행합니다.

Effect `{"effect":"refresh","topics":["sales"]}`는 토픽의 리스너를 새로 고칩니다. `[data-zeyn-listens="a,b"]`는 리스너(부모 `data-zeyn-node`의 노드)를 표시하고, `[data-zeyn-drives="a"]`는 토픽을 drive하는 컨트롤을 표시합니다. 필터 폼(`data-zeyn-kinds`는 어떤 params가 숫자나 불리언인지 알려 줍니다), 검색 상자(필드 이름은 `q`), 필터 칩(각 칩은 자신의 params를 `data-zeyn-params`에 담습니다)입니다. drive하면 컨트롤의 값이 Screen 루트의 `data-zeyn-params`에 병합되고, 그 Screen이 메인 Screen이면 주소가 그 값으로 바뀌며, 리스너가 새로 고쳐집니다. `topics.js`는 한 애니메이션 프레임에서 지정된 토픽을 한 번에 처리합니다. 그리드를 가진 리스너는 `zeyn:reload`를 받고, Screen의 다른 리스너는 한 번의 `POST /_zeyn/nodes`로 묶이며, 같은 Screen에 대한 더 새로운 요청은 이전 요청을 중단하고 그 노드를 넘겨받습니다. 포커스와 열린 `<details>`는 id, test id, `data-zeyn-key` 행, 위치로 유지됩니다.

가상 목록은 `data-zeyn-row-height`, `data-zeyn-block`, `data-zeyn-total`을 가진 `section[data-zeyn-vlist]`입니다. `vlist.js`는 `{ page: { type: "offset", offset }, with_total: true }`로 `POST /_zeyn/grid`에 블록을 요청하고 각 `<li>`를 `(aria-posinset - 1) * height` 위치에 둡니다. 다른 필드에 의존하는 lookup은 `data-depends-on="field"`를 가지며, 그 필드의 값을 `/_zeyn/data` 입력의 `parent`로 보냅니다.

클라이언트 환경설정(`zeyn::ClientPref`)은 브라우저 모듈 `/_zeyn/state.js`에 있으며, 페이지는 `data-zeyn-remember` 또는 `data-zeyn-prefs-clear`가 있는 요소가 있을 때만 이를 import합니다. `GET /_zeyn/prefs`는 앱의 동의 훅이 세션을 넘겨 환경설정 값을 보관하도록 허용하는지를 `{ preferences }`로 응답하며 `Cache-Control: no-store`를 붙입니다. 서버는 저장된 값을 읽지 않습니다.

### 7.4 미디어

`Video`와 `Audio`는 컴포넌트 라이브러리 뒤에 링크되는 `/_zeyn/media.js`와 `/_zeyn/media.css`로 제공됩니다. `docs/media.md`를 참고하십시오.

플레이어는 처음 화면에 들어올 때 페이지에 하나뿐인 `IntersectionObserver`를 통해 소스와 포스터(`data-zeyn-poster`, 먼저 `data-zeyn-poster-low`)를 설정합니다. `data-zeyn-preview`가 있는 루트는 4분의 1이 보이는 동안 음소거 상태로 반복 재생하며, `prefers-reduced-motion`, `prefers-reduced-data` 또는 Save-Data가 이를 막으면 재생하지 않고, 방문자가 컨트롤을 사용하면 그때부터는 그렇게 동작하지 않습니다. 로드 옵션이 있는 이미지는 스크립트 없이 자리 표시자를 보여 주는 상자(`.z-img-host`) 안에 있으며, `media.js`를 로드하게 하는 표시인 `data-zeyn-img`를 가집니다. 문서의 캡처 리스너 하나가 이 이미지들의 `load`와 `error`를 처리합니다. 로드되면 상자에 `z-img-done`(저해상도 파일 위로 페이드할 때는 `z-img-fade`)을 표시하고, 실패하면 `data-zeyn-fallback`을 한 번 시도한 다음 상자의 `<template data-zeyn-img-error>` 또는 아이콘과 함께 대체 텍스트를 보여 줍니다. 모듈이 실행되기 전에 이미 끝난 이미지는 모듈이 시작될 때 한 번 확인합니다.

| 엔드포인트 | 기능 |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | 범위 요청이 있는 등록된 미디어 소스의 파일: `200`, `206`, `304`, `416`. |
| `GET /_zeyn/media/position?source=&id=` | 로그인한 사용자가 멈춘 위치 `{ at, kept }`. 소스가 위치를 보관하지 않으면 `kept`는 `false`입니다(그러면 플레이어는 로컬 저장소를 사용합니다). |
| `POST /_zeyn/media/position` | CSRF 헤더와 함께 `{ source, id, at }`가 위치를 저장합니다. `204`, 잘못된 위치는 `422`. |

### 7.5 아일랜드

`islands` 기능을 사용하면 `div[data-zeyn-island]`가 있는 Screen은 `<meta name="zeyn-islands">`(아일랜드 이름에서 `{ js, wasm }` URL로), `/_zeyn/islands.js`, `'wasm-unsafe-eval'`이 있는 정책과 함께 제공되며, 다른 페이지는 바뀌지 않습니다. 아일랜드가 있는 부분 응답은 `x-zeyn-islands: 1`을 가지며, 문서에 아일랜드 정책이 없으면 `nav.js`가 이를 전체로 불러옵니다.

| 속성 또는 엔드포인트 | 의미 |
|---|---|
| `data-zeyn-island`, `data-zeyn-load`, `data-zeyn-props` | 아일랜드 이름, `visible`, `interaction`, `idle`, 속성 JSON |
| `data-zeyn-island-fallback` | 대체 콘텐츠이며 아일랜드가 마운트되면 숨겨집니다 |
| `data-zeyn-island-state` | 로더가 설정: `loading`, `ready`, `failed` |
| `GET /_zeyn/islands/{file}` | 매니페스트에 나열된 빌드된 파일이며 1년 동안 캐시됩니다 |
| `key` = remote 이름과 함께 `POST /_zeyn/data` | 7.1의 모든 검사가 적용되는 아일랜드의 타입이 있는 호출 |

`docs/islands.md`를 참고하십시오.

### 7.6 실시간

`realtime` 기능을 사용하면 `div[data-zeyn-live="topic"]`은 서버가 토픽을 알릴 때 새로 고쳐지는 노드를 표시합니다. 이를 가진 페이지는 `/_zeyn/live.js`를 링크하고, 이를 가진 부분 응답은 `x-zeyn-live: 1`을 가지며 `nav.js`가 스크립트를 가져옵니다. 요소의 `zeyn:reload`는 그 안의 그리드를 제자리에서 다시 불러옵니다.

| 엔드포인트 | 기능 |
|---|---|
| `GET /_zeyn/live/topics?t=a,b&csrf=` | WebSocket. 텍스트 프레임 `{ "topic": "a" }` |
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=` | WebSocket. 바이너리 프레임 `0x01` + Y 업데이트(v1)가 양방향으로 오갑니다 |

핸드셰이크는 업그레이드하기 전에 출처, 쿠키에 대한 CSRF 토큰, 로그인한 사용자, 권한을 검사합니다. `docs/realtime.md`를 참고하십시오.

플레이어는 프로그레시브 파일로 대체하기 전에 루트에서 취소 가능한 `zeyn:media-adaptive` 이벤트(`{ media, kind, url, hls, dash }`)를 발생시키며, 스트리밍 라이브러리는 `preventDefault()`를 호출하여 이를 넘겨받습니다.

## 8. 데스크톱 클래스 셸

`/_zeyn/pro.js`와 `/_zeyn/pro.css`는 컴포넌트 라이브러리 뒤에 로드됩니다. 런타임처럼 로그인이 필요 없습니다.

### 8.1 명령과 팔레트

명령은 Rust(`Command::open`, `Command::navigate`, `Command::effect`, `Command::action`)로 선언하고 `Feature::command`로 등록합니다. 셸은 사용자가 실행할 수 있는 명령(Feature 가드를 통과하고 명령의 접근 권한이 허용)마다 숨겨진 버튼 하나를 `div[data-zeyn-commands]` 안에 렌더링합니다.

| 속성 | 의미 |
|---|---|
| `data-zeyn-command` | 명령 id |
| `data-zeyn-effect` | effect 명령의 effect이며 클릭 시 런타임이 실행합니다 |
| `data-zeyn-command-run` | action 명령의 id이며 `POST /_zeyn/command`로 실행됩니다 |
| `aria-keyshortcuts` | 정규화된 단축키(`Control+Alt+N`). 런타임의 단축키 처리가 버튼을 클릭합니다 |
| `data-hint`, `data-icon` | 사용자에게 표시되는 단축키, 아이콘 이름 |
| 컨테이너의 `data-text-*` | 현지화된 팔레트 텍스트 |

`App::check`는 읽을 수 없는 단축키, 한 글자 단축키(WCAG 2.1.4), 팔레트 자신의 단축키, 두 명령이 사용하는 단축키나 id, 브라우저가 페이지에 전달하지 않는 단축키(`BROWSER_RESERVED`: `Ctrl+N`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Alt+F4`와 그 macOS 형태), 그리고 명령이 `.override_browser()`를 호출하지 않는 한 브라우저가 사용하는 단축키(`BROWSER_DEFAULTS`: 인쇄, 저장, 찾기, 주소 표시줄, 새로 고침, 확대/축소, 개발자 도구, 복사와 붙여넣기, `F1`부터 `F12`)를 거부합니다. 툴바 버튼도 실행 시점에 같은 규칙을 따릅니다. 그러한 단축키는 바인드되지 않고 이유가 기록됩니다. 사용할 수 있는 조합은 `Alt+Shift+<letter>`와 `Ctrl+Alt+<letter>`입니다.

`Alt+Shift+P`(macOS에서는 Option+Shift+P)는 기본적으로 팔레트를 엽니다. `App::palette_shortcut("Ctrl+Shift+P")`로 바꿀 수 있으며, 의도적으로 브라우저가 사용하는 키로도 바꿀 수 있습니다. 단축키는 `data-palette-keys`에 있으며, 글자는 물리적인 키로 매칭되므로 Option과 AltGr이 이를 깨뜨리지 않습니다. 팔레트는 메뉴의 `button[data-zeyn-palette-open]`에서도 열립니다. ARIA 콤보박스와 리스트 박스가 있는 모달 `<dialog>`입니다. 앱의 명령, 현재 Screen(창이 열려 있으면 맨 위 창)의 툴바 버튼, 메뉴 막대 명령, 컨텍스트 메뉴 항목, 메뉴의 Screen을 나열합니다. 매칭은 대소문자와 악센트를 무시하는 퍼지 방식이며, 가장 최근에 실행한 명령 여덟 개가 먼저 나옵니다(`localStorage` 키 `zeyn.palette.recent`). Escape는 이를 닫고 포커스는 있던 곳으로 돌아갑니다.

| 엔드포인트 | 본문 | 응답 |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | 서버에 고정된 입력으로 action 명령을 실행합니다. CSRF, Feature 가드, action의 권한, 인터셉터. `{ effects }`. 알 수 없는 id와 effect 명령은 `404` |
| `POST /_zeyn/search` | `{ query }` | 사용자가 실행할 수 있는 모든 `Feature::search`를 실행합니다(CSRF, Feature 가드, action의 권한, 인터셉터). 쿼리는 100자에서 잘립니다. `{ hits: [{ label, group, url }] }`, 최대 20개, 앱 경로만 |

컨테이너에 `data-search`가 있으면 팔레트는 마지막 키 입력 150 ms 후(두 글자 이상) 쿼리를 보내고 결과를 명령 아래에 나열합니다. 가장 최근 쿼리의 응답만 그려집니다. 결과를 선택하면 `url` 앞에 기본 경로를 붙여 `zeyn:navigate`를 보내며, 그 안의 `#anchor`는 Screen이 교체된 후 해당 요소로 스크롤합니다.

런타임은 `document`의 `zeyn:run`을 수신하므로 다른 스크립트가 이를 통해 작업을 실행합니다. `{ effects, source }`는 effect를 실행하고, `{ screen, item }`은 Screen 컨텍스트 메뉴 항목을 실행하며, `{ error }`는 오류 토스트를 표시합니다.

### 8.2 메뉴 막대, 도구 상자, 속성 검사기, 상태 표시줄

| 마크업 | 동작 |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG 메뉴 막대. 상단 항목은 roving tab stop이 있는 `button[role=menuitem][aria-haspopup=menu]`이며, 각각 닫혀 있는 동안 `hidden`인 `ul[role=menu]`를 제어합니다. 항목은 8.1의 명령 속성을 가집니다. 왼쪽과 오른쪽 화살표는 메뉴 사이를 이동하고, 아래쪽 화살표, Enter, Space는 열고, 위쪽 화살표는 마지막 항목에서 열며, Home과 End는 건너뛰고, 문자는 그 문자로 시작하는 다음 항목으로 이동하며, Escape는 닫고 막대로 돌아가고, Tab과 바깥 클릭은 닫습니다. 사용자가 실행할 수 없는 메뉴와 항목은 렌더링되지 않습니다. |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | `button.z-tool[aria-pressed]`의 토글 그룹. 화살표, Home, End는 roving tab stop을 이동하며, 클릭, Enter, Space는 도구 하나를 켜고 `zeyn:tool`(`detail.tool`)을 보내며 `data-text-selected`를 상태 표시줄에 기록합니다. |
| `section.z-inspector` | `Form::section` 그룹(`role=group` 본문이 있는 `details.z-form-section`)이 있는 폼이며 검사기의 노드 경로에서 `POST /_zeyn/form`으로 저장됩니다. 실패한 필드가 있는 섹션은 브라우저의 검사 전과 `422` 후에 열립니다. |
| `div[data-zeyn-statusbar]` | 시작과 끝의 항목과 `[data-zeyn-status-live]`(`role=status`, `aria-live=polite`). Screen의 모든 요소에서 오는 `zeyn:status`(`detail.text`)는 라이브 텍스트를 교체합니다. |

### 8.3 도킹 패널

`div.z-dock[data-zeyn-dock]`는 네 개의 `section.z-dock-area[data-area]`(`left`, `center`, `bottom`, `right`)와 세 개의 `div.z-dock-bar[role=separator][data-edge]` 구분선으로 이루어진 그리드입니다. 크기는 사용자 지정 속성 `--z-dock-left`, `--z-dock-right`(너비의 퍼센트), `--z-dock-bottom`(높이의 퍼센트)이며 10에서 60입니다. 각 영역에는 끌 수 있는 `button.z-dock-tab[data-panel]`의 `role=tablist`, 옵션 버튼, 자식 노드를 담은 `role=tabpanel` 패널이 있습니다(경로는 다른 컨테이너와 같으며 패널이 이동해도 바뀌지 않습니다). 열린 패널이 없는 측면 영역은 숨겨집니다.

키보드: 왼쪽과 오른쪽 화살표(오른쪽에서 왼쪽 페이지에서는 반전), Home, End는 영역의 열린 탭 사이를 이동하고, Delete는 포커스된 패널을 닫으며, Shift+F10 또는 컨텍스트 메뉴 키는 영역의 옵션(다른 영역으로 이동, 닫기, 닫힌 패널 표시)을 열고, 구분선에서의 화살표는 5퍼센트씩 크기를 조절하며 Home과 End는 한계로 이동합니다. `MenuList::panels`로 만든 메뉴 막대 항목은 모든 패널을 `menuitemcheckbox`로 나열합니다. 변경 사항은 `[data-zeyn-dock-live]`에서 안내됩니다.

| 엔드포인트 | 본문 | 응답 |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: 로그인한 사용자의 저장된 레이아웃 또는 `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`. `layout: null`은 삭제합니다 |

`layout`은 탭 순서의 `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }`입니다. 서버는 CSRF, Screen의 접근 권한, Feature의 가드를 검사하고, 도크 자체의 패널만(각각 한 번씩) 유지하고 크기를 제한한 다음, 테넌트, Screen, 노드 아래의 앱의 뷰 스토어(`App::datagrid_views`)에 저장합니다. 스토어가 없으면 엔드포인트는 `404`로, 사용자가 없으면 `403`으로 응답하며, 그러면 브라우저는 레이아웃을 `localStorage`에 보관합니다.

## 9. AI 에이전트: MCP와 WebMCP

외부 AI 에이전트는 두 개의 문을 통해 앱에 도달하며, 둘 다 기본적으로 꺼져 있고 `App::mcp`로 켭니다.

| 설정 | MCP 엔드포인트 | WebMCP 도구 | 쓰기 |
|---|---|---|---|
| `Mcp::off()` (기본값) | `404` | `404` | 아니요 |
| `Mcp::read_only()` | 켜짐 | 켜짐 | 아니요 |
| `Mcp::on()` | 켜짐 | 켜짐 | 폼 저장과 쓰는 API action |

`.server(false)`와 `.webmcp(false)`는 문 하나를 끕니다. `zeyn new`로 만든 새 프로젝트는 `Mcp::read_only()`로 시작합니다.

꺼져 있는 이유: 엔드포인트는 유효한 `Authorization` 헤더를 가진 누구나 모델을 통해 앱을 조작할 수 있게 하며, 모델은 읽는 데이터에 의해 조종될 수 있습니다. 프레임워크는 모든 앱이 에이전트를 맞이할 준비가 되게 하지만(도구는 코드에서 생성되며 작성할 것이 없습니다), 앱이 그 문을 열지, 에이전트가 쓸 수 있게 할지는 이를 운영하는 사람들의 결정입니다. 읽기 전용은 한 줄이면 됩니다.

### 9.1 도구

도구는 로그인한 사용자를 위해 앱에서 만들어집니다.

- 매개변수 없이 열리는 모든 경로 Screen의 그리드마다 검색 도구 하나(`{screen}_{node}_search`, 입력: `DataQuery`), 쓰기가 허용되면 폼마다 저장 도구 하나(`{screen}_{node}_save`, 입력: 사용자가 입력한 폼 값),
- 게시된 API action마다 도구 하나(`api_{feature}_{action}`). 입력이 `DataQuery`인 action은 읽기이며, 그 밖의 모든 API action은 쓰기로 간주됩니다.

설명은 Screen 제목, Screen과 action의 `///` 문서 주석의 첫 문단, 그리고 `ai = "..."` 안내로 만들어집니다. 사용자가 사용할 수 없는 그리드와 폼, 그리고 `ai = false`로 표시된 Screen과 action은 나열되지 않으며 호출하면 거부됩니다.

`#[action(confirm)]`은 사용자가 앱에서 확인한 후에만 모델이 실행할 수 있는 action을 표시합니다. `/_zeyn/mcp`는 그런 도구를 나열하지도 실행하지도 않습니다(그 채널에서는 아무도 확인할 수 없습니다). WebMCP 도구는 모달 대화 상자(`zeyn:confirm`)에서 묻고 폼과 함께 `confirmed: true`를 보냅니다. 내장 어시스턴트는 채팅에서 묻습니다(`docs/assistant.md` 참고).

### 9.2 엔드포인트

| 엔드포인트 | 기능 |
|---|---|
| `GET /_zeyn/tools?screen&params` | 현재 보고 있는 Screen에 대한 `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }`. `zeyn.js`가 이를 `document.modelContext`에 등록합니다 |
| `POST /_zeyn/mcp` | JSON-RPC: `initialize`, `tools/list`, `tools/call`. `Authorization` 헤더와 로그인한 사용자가 필요합니다 |
| `via_agent: true`와 함께 `POST /_zeyn/form` | WebMCP 폼 저장. WebMCP와 함께 `Mcp::on()`이 필요합니다. `confirmed: true`는 사용자가 확인한 후에만 |

모든 에이전트 호출은 사용자로서 실행됩니다. Screen의 접근 권한, 가드, action의 권한, 인터셉터, 속도 제한, 시간 제한이 적용됩니다. 에이전트 호출은 항상 감사되며(`via_agent: true`) 마스킹된 필드는 사용자가 공개할 수 있더라도 그들에게는 항상 마스킹됩니다. OpenAPI는 각 작업을 `x-zeyn-ai`로 표시합니다. `ai = false`이면 `false`, 그렇지 않으면 `{ hint, confirm, writes }`입니다.
