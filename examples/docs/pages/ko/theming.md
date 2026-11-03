# 테마

상태: `crates/zeyn/assets/zeyn.css`에 구현되어 있습니다.

기본 UI 키트는 모든 색상, 글꼴, 크기, 둥글기를 `:root`의 CSS 사용자 지정 속성에서 읽습니다. 조직은 그중 몇 가지를 덮어써서 외관을 바꿉니다. `zeyn.css`를 편집하거나 선택자를 작성할 필요가 전혀 없습니다.

## 1. 테마가 적용되는 방식

서버는 먼저 `zeyn.css`를 제공하고, 앱에 있으면 토큰 덮어쓰기가 들어 있는 추가 스타일시트를 제공합니다. 덮어쓰기 파일에는 `:root` 블록만 들어 있습니다.

```css
:root {
  --z-brand: #7a2448;
  --z-font: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --z-radius: 8px;
  --z-density: 0.75;
}
```

덮어쓰기 파일의 규칙은 다음과 같습니다.

- 앱 자체의 출처에서 제공되어야 합니다. Content Security Policy가 `style-src 'self'`이므로 인라인 `<style>` 블록과 서드파티 스타일시트는 차단됩니다.
- 글꼴은 사용자의 컴퓨터에 이미 설치되어 있거나 앱의 출처에서 제공되어야 합니다. 키트 자체는 웹 글꼴을 불러오지 않습니다.
- `zeyn.css` 뒤에 불러오십시오. 그러면 같은 선택자는 순서로 우선순위가 결정됩니다.
- 2절의 기본 토큰을 설정하십시오. 3절의 파생 토큰은 자동으로 따라옵니다. 계산된 값이 브랜드에 맞지 않을 때만 파생 토큰을 덮어쓰십시오.

### 라이트, 다크, 앱 테마

모든 앱에는 메뉴의 언어 선택기 옆에 테마 선택기가 있습니다. 시스템(`prefers-color-scheme`을 따름), 라이트, 다크, 그리고 앱이 정의한 모든 테마를 제공합니다.

```rust
App::new("Billing")
    .theme(Theme::new().brand("#7a2448").radius(6))
    .themes([
        Theme::light(),
        Theme::dark(),
        Theme::named("sand", [("--z-bg", "#f5f1ea"), ("--z-surface", "#fffdf8")]).label(t!("theme.sand")),
    ])
    .default_theme("system")
```

- `App::theme`는 조직의 외관이며 모든 테마에 적용됩니다.
- `App::themes`는 사용자가 선택할 수 있는 것을 나열합니다. 이를 지정하지 않으면 목록은 라이트와 다크입니다. 시스템은 둘 다 목록에 있을 때 제공됩니다. `Theme::named(...)`는 라이트 테마를 기반으로 만들며, `.dark_base()`는 다크 테마를 기반으로 만듭니다.
- `App::default_theme`는 선택하지 않은 사용자의 테마입니다. 설정하지 않으면 `system`입니다.
- `App::theme_picker(false)`는 선택기를 숨기며, 모든 사람이 기본 테마를 받습니다.
- 테마가 하나뿐이면 앱이 고정됩니다. `.themes([Theme::named("acme", tokens)])` 또는 `.theme(Theme::dark())`.

토큰은 앱이 시작할 때 검사됩니다. `App::check`(따라서 `router`와 `serve`)는 알 수 없는 토큰 이름, 선언을 벗어날 수 있는 값(`;`, 중괄호, `\`, `<`, `>`, `url(`), 잘못된 테마 이름, 알 수 없는 기본 테마, 그리고 테마가 WCAG 2.2 AA 미만으로 떨어뜨리는 5절의 모든 색상 쌍을 보고합니다. 대비는 `#rgb`와 `#rrggbb` 값만 검사할 수 있으며, 다른 색상 구문은 신뢰합니다.

모든 테마는 시작할 때 한 번 만들어져 키트의 스타일시트 뒤에 불러오는 하나의 스타일시트 `/_zeyn/theme.css`로 제공됩니다. 선택은 `zeyn_theme` 쿠키에 저장되며, 뷰 스토어가 있는 앱(`App::datagrid_views`)에서 로그인한 사용자는 그 스토어에도 저장되므로 새 기기에서도 따라옵니다. 서버가 `<html>`에 테마를 렌더링하므로 첫 번째 그리기부터 이미 올바르며 인라인 스크립트가 필요 없습니다.

| 마크업 | 결과 |
|---|---|
| `<html>` | 시스템: `prefers-color-scheme`을 따릅니다 |
| `<html data-theme="light">` | 항상 라이트 |
| `<html data-theme="dark">` | 항상 다크 |
| `<html data-theme="light" data-zeyn-theme="sand">` | 라이트 테마 위의 앱 테마 `sand` |

선택기에서 테마를 바꾸면 이 속성과 `color-scheme` meta만 바뀌고 `POST /_zeyn/theme`을 호출합니다. 아무것도 새로 고쳐지지 않습니다.

직접 작성한 덮어쓰기 파일에서 다크 테마에 별도의 값을 주려면 키트가 사용하는 선택자를 반복하십시오.

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### 밀도

`--z-density`는 간격 단위를 조절합니다. `1`은 보통이고 `0.75`는 촘촘함입니다. `<html data-density="compact">`는 스타일시트 없이 `0.75`를 설정합니다. 컨트롤과 그리드 행은 24 px보다 작아지지 않으므로 촘촘한 모드에서도 WCAG 2.2 대상 크기를 충족합니다.

가상 그리드의 행 높이는 런타임이 이로부터 스크롤 위치를 계산하므로 서버가 고정합니다(`Paging::virtual_rows(px)`, 기본 32 px). 밀도는 이를 바꾸지 않습니다.

## 2. 기본 토큰

조직이 설정할 것으로 기대되는 토큰입니다.

| 토큰 | 용도 | 라이트 | 다크 |
|---|---|---|---|
| `--z-brand` | 하나의 브랜드 색상. 기본 버튼, 앱 이름 띠, 현재 메뉴 항목, 포커스된 행, 포커스 링 | `#0f6674` | 같은 값. `--z-accent` 참고 |
| `--z-on-brand` | 단색 `--z-brand` 면 위의 텍스트 | `#ffffff` | `#ffffff` |
| `--z-font` | 인터페이스 글꼴 스택 | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | 같음 |
| `--z-font-mono` | 코드, API 예제 | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | 같음 |
| `--z-font-size` | 본문 텍스트 | `0.875rem` (14 px) | 같음 |
| `--z-font-size-sm` | 그리드 셀, 레이블, 필터 | `0.8125rem` (13 px) | 같음 |
| `--z-font-size-xs` | 열 헤더, 메뉴 그룹 제목, 페이지 정보 | `0.75rem` (12 px) | 같음 |
| `--z-radius` | 기본 모서리 둥글기, 0에서 12 px. 모든 컴포넌트가 이를 기준으로 조정됩니다 | `4px` | 같음 |
| `--z-density` | 간격 배율. `1`은 보통, `0.75`는 촘촘함 | `1` | 같음 |
| `--z-bg` | 패널 뒤의 페이지 배경 | `#f4f5f6` | `#121416` |
| `--z-surface` | 패널, 그리드, 창, 입력 | `#ffffff` | `#1a1d20` |
| `--z-surface-2` | 그리드 헤더, 줄무늬 행, 창 바닥글, 읽기 전용 입력 | `#f7f8f9` | `#202326` |
| `--z-hover` | 행, 메뉴 항목, 버튼의 호버 배경 | `#eceef1` | `#2a2e33` |
| `--z-border` | 구분선과 패널 윤곽선 | `#dcdfe3` | `#33383e` |
| `--z-border-strong` | 입력과 버튼 윤곽선, 헤더 선. `--z-surface`에 대해 3:1에 도달해야 합니다 | `#7d848d` | `#7a818a` |
| `--z-text` | 본문 텍스트와 값 | `#1a1d21` | `#e4e6e9` |
| `--z-text-muted` | 레이블, 헤더, 보조 텍스트. `--z-surface-2`에 대해 4.5:1에 도달해야 합니다 | `#5b626b` | `#a2a9b1` |
| `--z-danger` | 오류, 필수 표시, danger 행 | `#b3261e` | `#f28b82` |
| `--z-warning` | warning 행 | `#8a5300` | `#e6b45a` |
| `--z-success` | success 행 | `#1e6b3a` | `#6fcf8f` |
| `--z-info` | info 토스트 | `#2b5d8a` | `#8ab8e6` |

## 3. 파생 토큰

`color-mix()`와 상대 색상 구문으로 기본 토큰에서 계산됩니다. 기본 토큰이 바뀌면 저절로 갱신됩니다.

| 토큰 | 용도 | 라이트 | 다크 |
|---|---|---|---|
| `--z-accent` | 면 위의 텍스트나 가는 선으로 쓰이는 브랜드 색상: 링크, 현재 메뉴 항목, 포커스된 행 막대 | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, 고정된 밝은 톤의 브랜드 색조. 상대 색상 구문이 없는 브라우저는 `color-mix(in oklab, var(--z-brand) 45%, #fff)`를 사용합니다 |
| `--z-primary` | 기본 버튼 채우기 | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | 기본 버튼 텍스트 | `var(--z-on-brand)` | `#121416` |
| `--z-primary-hover` | 기본 버튼 호버 | 브랜드 86%에 검정을 섞음 | 강조색 84%에 흰색을 섞음 |
| `--z-primary-active` | 기본 버튼 눌림 | 브랜드 74%에 검정을 섞음 | 강조색 70%에 흰색을 섞음 |
| `--z-brand-subtle` | 현재 메뉴 항목과 포커스된 그리드 행 뒤의 색조 | `--z-surface` 위에 `--z-tint`만큼의 강조색 | 같은 공식 |
| `--z-focus` | 포커스 링 | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | 행과 메시지의 배경 | `--z-surface` 위에 `--z-tint`만큼의 상태 색상 | 같은 공식 |
| `--z-tint` | 은은한 색조의 강도 | `9%` | `20%` |
| `--z-space` | 간격 단위 | `calc(4px * var(--z-density))` | 같음 |
| `--z-control` | 버튼과 입력의 높이 | `max(24px, 8 * --z-space)` (32 px) | 같음 |
| `--z-row` | 페이지 방식과 무한 그리드의 그리드 행 높이 | `max(24px, 8 * --z-space)` (32 px) | 같음 |
| `--z-gap` | Screen 섹션 사이의 간격 | `3 * --z-space` (12 px) | 같음 |
| `--z-radius-sm`, `--z-radius-lg` | 작은 부분(필터, 코드), 창 | `--z-radius`의 `0.75x`와 `1.5x` | 같음 |
| `--z-shadow-2` | 창과 토스트에만 적용. 그리드 행에는 그림자가 없습니다 | 부드러운 두 겹 그림자 | 더 어두움 |
| `--z-backdrop` | 모달 창 뒤 | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | 논리 속성이 없는 오프셋(시작 또는 끝 쪽을 표시하는 안쪽 그림자, 슬라이드 변환)에 쓰는 읽기 방향 부호: `calc(2px * var(--z-dir))` | `1`, `dir="rtl"`에서는 `-1` | 같음 |

서버 마크업이 설정하는 요소별 토큰: `--z-width`(창 너비), `--z-flex`(열의 비율), `--z-row-height`(가상 그리드 행 높이), 그리고 컴포넌트 라이브러리를 위한 `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level`, `--z-pop-x` / `--z-pop-y`(팝업 위치).

차트 계열 색상은 `components.css`의 `--z-chart-1`부터 `--z-chart-6`까지이며, 다크 테마에서는 더 밝은 값을 사용합니다. 각각 `--z-surface`에 대해 3:1에 도달합니다. 다른 기본 토큰처럼 덮어쓸 수 있습니다.

## 4. 브랜드 색상 고르기

- `--z-brand`는 라이트 테마에서 흰색 텍스트를 올립니다(버튼, 앱 이름 띠). `--z-on-brand`에 대해 최소 4.5:1인 색상을 고르십시오. 대부분의 중간에서 어두운 브랜드 색상은 통과합니다. 노란색 같은 밝은 브랜드 색상에는 `--z-on-brand`도 어두운 색으로 설정하십시오.
- 다크 테마에서 키트는 텍스트, 링, 기본 버튼에 쓰는 브랜드를 밝게 하고, 앱 이름 띠에는 단색 브랜드를 유지합니다. 다른 브랜드도 두 번째 색상 없이 두 테마 모두에서 의도한 것처럼 보입니다.
- 상태 색상은 브랜드와 구별되게 유지하십시오. 빨간 브랜드 옆에 빨간 오류 행이 있으면 오류를 알아보기 어렵습니다. 그런 경우 `--z-danger`를 다른 빨간색으로 옮기거나, `--z-brand-subtle`을 덮어써서 그리드에서 브랜드를 빼십시오.
- 5절의 표를 목표로 삼아 색상을 확인하십시오. 임계값 미만인 쌍은 WCAG 2.2 AA에 실패합니다.

## 5. 검증된 대비

실행 중인 예제 앱에서 Chromium으로 측정했습니다. 모든 토큰을 렌더링된 sRGB 값(`color-mix()`와 상대 색상 포함)으로 해석한 다음 WCAG 2.x 대비율을 계산했습니다. 임계값은 텍스트 4.5:1, UI 컴포넌트 윤곽선과 포커스 표시기 3:1입니다.

| 쌍 | 라이트 | 다크 | 필요 |
|---|---|---|---|
| `--z-text` on `--z-bg` | 15.49 | 14.77 | 4.5 |
| `--z-text` on `--z-surface` | 16.91 | 13.54 | 4.5 |
| `--z-text` on `--z-surface-2` | 15.91 | 12.63 | 4.5 |
| `--z-text` on `--z-hover` | 14.55 | 10.93 | 4.5 |
| `--z-text` on `--z-brand-subtle` (포커스된 행) | 14.79 | 9.27 | 4.5 |
| `--z-text` on `--z-danger-subtle` | 14.73 | 9.76 | 4.5 |
| `--z-text` on `--z-warning-subtle` | 14.82 | 9.31 | 4.5 |
| `--z-text` on `--z-success-subtle` | 14.76 | 9.33 | 4.5 |
| `--z-text-muted` on `--z-bg` | 5.65 | 7.78 | 4.5 |
| `--z-text-muted` on `--z-surface` | 6.17 | 7.13 | 4.5 |
| `--z-text-muted` on `--z-surface-2` (헤더) | 5.80 | 6.65 | 4.5 |
| `--z-text-muted` on `--z-hover` | 5.31 | 5.76 | 4.5 |
| `--z-text-muted` on `--z-danger-subtle` (흐린 행) | 5.37 | 5.14 | 4.5 |
| `--z-text-muted` on `--z-warning-subtle` | 5.41 | 4.90 | 4.5 |
| `--z-accent` on `--z-surface` (링크) | 6.62 | 9.33 | 4.5 |
| `--z-accent` on `--z-bg` | 6.06 | 10.18 | 4.5 |
| `--z-accent` on `--z-brand-subtle` (현재 메뉴 항목) | 5.79 | 6.39 | 4.5 |
| `--z-on-primary` on `--z-primary` | 6.62 | 10.18 | 4.5 |
| `--z-on-primary` on `--z-primary-hover` | 8.82 | 11.26 | 4.5 |
| `--z-on-primary` on `--z-primary-active` | 11.12 | 12.26 | 4.5 |
| `--z-on-brand` on `--z-brand` (앱 이름 띠) | 6.62 | 6.62 | 4.5 |
| `--z-danger` on `--z-surface` (필드 오류) | 6.54 | 7.09 | 4.5 |
| `--z-danger` on `--z-danger-subtle` | 5.69 | 5.11 | 4.5 |
| `--z-warning` on `--z-surface` | 6.33 | 8.90 | 4.5 |
| `--z-warning` on `--z-warning-subtle` | 5.55 | 6.12 | 4.5 |
| `--z-success` on `--z-surface` | 6.52 | 8.87 | 4.5 |
| `--z-info` on `--z-surface` | 6.92 | 8.13 | 4.5 |
| `--z-focus` on `--z-surface` | 6.62 | 9.33 | 3 |
| `--z-focus` on `--z-bg` | 6.06 | 10.18 | 3 |
| `--z-focus` on `--z-brand-subtle` | 5.79 | 6.39 | 3 |
| `--z-border-strong` on `--z-surface` (입력 윤곽선) | 3.78 | 4.30 | 3 |
| `--z-border-strong` on `--z-surface-2` | 3.55 | 4.01 | 3 |

`--z-border`는 장식용(이미 텍스트가 있는 항목 사이의 구분선)이며 3:1 기준이 적용되지 않습니다.

## 6. 그 밖의 접근성 동작

- 모든 상호작용 요소는 `--z-focus` 색상의 2 px `:focus-visible` 링을 표시합니다. 그리드 행은 스크롤 컨테이너에 잘리지 않도록 링을 행 안쪽에 그립니다.
- 버튼, 입력, 정렬 헤더, 필터 상자, 페이저 버튼, 창 닫기 버튼은 어떤 밀도에서도 최소 24 x 24 px입니다.
- `prefers-reduced-motion: reduce`는 토스트 등장과 모든 전환을 제거합니다.
- `forced-colors` 모드에서 아이콘은 `CanvasText`를 사용하고, 기본 버튼과 포커스된 행은 `Highlight`를 사용하며, 배경 색조에 의존하는 상태(현재 메뉴 항목, 행 상태)는 대신 윤곽선이나 테두리를 얻습니다.
