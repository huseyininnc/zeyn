# 컨트롤과 피커

폼 필드는 자신의 타입에 맞는 컨트롤을 고릅니다. `.control(M::f, Control::...)`은 컨트롤을 바꾸고, `.lookup(M::f, action)`은 필드를 서버에 대한 검색으로 바꿉니다. 모든 컨트롤은 내부적으로 기본 폼 요소이므로 런타임이 있든 없든 값이 같은 방식으로 전송되며, 모든 컨트롤은 키보드와 스크린 리더 사용자를 위한 ARIA 패턴을 따릅니다.

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## 기본 컨트롤

| 필드 타입 | 컨트롤 |
|---|---|
| `String` | 텍스트 입력. `max_len`이 255를 넘으면 글자 수 카운터가 있는 자동 확장 텍스트 영역 |
| `i32`, `i64` | `min`과 `max` 규칙이 있는 숫자 입력 |
| `Money` | 사용자의 숫자 형식으로 표시하는 금액과 통화 |
| `Decimal` | 사용자의 형식으로 표시하는 정확한 숫자이며 필드의 `scale`을 따릅니다 ([Decimal 입력](#decimal-and-percent) 참고) |
| `bool` | 체크박스 |
| `#[choice]` enum | 선택: 검색 가능한 목록을 여는 버튼 |
| `#[choice]` enum의 `Vec` | 다중 선택: 칩과 여러 옵션을 고르는 목록 |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | 날짜 및 시간 피커 |
| 모델의 `Vec<M>` | 라인 편집기 ([라인 편집기](lines.md) 참고) |

## 그 밖의 컨트롤

| 컨트롤 | 용도 |
|---|---|
| `Control::Radio` | 옵션이 몇 개 없어 모두 보이는 선택 |
| `Control::Segmented` | 짧은 옵션이 둘에서 다섯 개인 선택 |
| `Control::Listbox` | 입력하여 검색하는 긴 선택 목록 |
| `Control::Switch` | 설정처럼 즉시 적용되는 `bool` |
| `Control::Percent` | 퍼센트 기호와 함께 표시되는 `Decimal` 또는 정수 |
| `Control::Password`, `Control::NewPassword` | 비밀번호로 입력되며 표시와 숨김 버튼이 있는 텍스트 필드 |
| `Control::PasswordMeter` | 강도 표시기가 있는 새 비밀번호 |
| `Control::Rating` | 1부터 N까지의 정수를 별로, `Decimal` 필드는 반 별로 표시합니다([Rating](#rating) 참조) |
| `Control::Multiline` | 높이가 늘어나는 텍스트 영역에 쓰는 길이 제한 없는 텍스트 |
| `Control::Color { palette }` | 16진수 색을 담는 `String`, 색 선택기 포함([Color and length](#color-and-length) 참고) |
| `Control::Length { units }` | 픽셀로 저장하고 사용자가 고른 단위로 보여 주는 `Decimal` 길이([Color and length](#color-and-length) 참고) |
| `Control::Code(lang)` | 강조 표시, 제안, 오류 표시가 있는 SQL 또는 식([코드 편집기](#code-editor) 참고) |

## Rating

`Control::Rating`은 정수 필드를 1부터 N까지의 별로 표시하며, N은 필드의 `max` 규칙입니다(규칙이 없으면 5, 최대 10). 레이블이 필드 레이블인 `fieldset` 안의 네이티브 라디오 버튼 그룹이므로 스크립트 없이 W3C APG 라디오 그룹 패턴을 따릅니다. Tab은 그룹에 한 번만 도달하고, 화살표 키로 이동하며 선택하고, 각 별은 사용자의 언어로 "3 / 5"처럼 이름이 붙습니다. 마우스를 올리면 포인터 위치까지 별이 밝아집니다. 서버는 다른 숫자처럼 필드의 규칙으로 값을 검증하므로 필드에 `min`과 `max`를 지정하십시오. `Option<i64>`에 `required`가 있으면 모든 별을 비워 두는 것은 거부됩니다. `#[rule(min = 1, max = 5, scale = 1)]`이 있는 `Decimal` 필드는 반 별 단위로 받으며, 서버는 4.3처럼 반 단위 사이의 값을 거부합니다([쇼핑 컴포넌트](commerce.md) 참조).

```rust
#[model]
pub struct Feedback {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
}

Form::of(Feedback::default())
    .field(Feedback::stars)
    .control(Feedback::stars, Control::Rating)
    .submit(t!("feedback.send"), send_feedback)
```

@live rating


## Color and length

```rust
const BRAND: &[&str] = &["#1E88E5", "#43A047", "#00000080"];

Form::of(band)
    .field(Band::fill)
    .control(Band::fill, Control::Color { palette: BRAND })
    .field(Band::height)
    .control(Band::height, Control::Length { units: &[LengthUnit::Mm, LengthUnit::Pt] })
```

`Control::Color`는 `String` 필드를 `#RRGGBB` 또는 `#RRGGBBAA`로 저장합니다. 필드는 색 견본이 있는 텍스트 입력이라 스크립트 없이도 동작합니다. 버튼을 누르면 빨강, 초록, 파랑, 불투명도 슬라이더와 최근에 쓴 색, 앱의 팔레트가 있는 대화 상자가 열립니다. Escape나 완료를 누르면 닫히고 포커스가 돌아옵니다. 서버는 16진수 색이 아닌 값을 거부합니다.

`Control::Length`는 `Decimal` 필드를 CSS 픽셀(1인치에 96)로 저장합니다. 사용자는 밀리미터, 센티미터, 인치, 포인트, 픽셀로 입력하고 숫자 옆에서 단위를 고릅니다. 브라우저는 모든 길이 필드에 대해 이 단위를 기억합니다. 위아래 화살표는 0.1씩, Shift와 함께 1씩, 픽셀 단위인 `min`과 `max` 규칙 안에서 움직입니다. 필드에 `scale = 4`를 지정하세요.

## Lookup

lookup은 사용자가 입력하는 동안 서버에서 레코드를 검색합니다. action은 `LookupQuery`를 받아 `Page<LookupItem>`을 반환하며, 필드에는 id가 저장됩니다.

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)`은 `Vec<i64>` 필드에 같은 기능을 제공하며, 선택된 항목을 제거 가능한 태그로 표시합니다.

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])`는 목록에 여러 열을 표시합니다. 검색은 여전히 서버의 action으로 가며, 키 입력마다 이루어지는 검색에서 그 권한이 검사됩니다. 각 항목은 `LookupItem::cells`로 헤더 순서대로 값을 제공합니다. `LookupQuery::apply`는 셀도 검색합니다. 스크린 리더는 각 옵션을 "Code: C-2, Name: Mert Kaya, City: Ankara"처럼 읽습니다. 좁은 화면에서는 열이 각 옵션 안에 세로로 쌓입니다.

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## 다중 선택

`#[choice]` enum의 `Vec`은 다중 선택으로 편집합니다. 선택된 옵션은 제거 버튼이 있는 칩으로 표시되고, 콤보박스는 옵션을 켜고 끄는 목록을 엽니다(콤보박스 안의 `aria-multiselectable`이 있는 APG 리스트 박스). 내부는 기본 `<select multiple>`이므로 런타임이 없어도 필드가 동작합니다. 런타임은 값을 쉼표로 이어서 전송하고(`Email,Sms`), 서버는 이를 하나씩 읽고 알 수 없는 값은 거부하며 중복은 제거합니다. JSON(API, AI 도구, 데이터베이스)에서 이 필드는 옵션 값의 배열입니다.

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| 키 | 동작 |
|---|---|
| 아래쪽 화살표, 위쪽 화살표, Enter, Space | 닫힌 콤보박스에서 목록을 엽니다 |
| Home, End | 목록을 열고 첫 번째 또는 마지막 옵션으로 이동합니다 |
| 아래쪽 화살표, 위쪽 화살표 | 열린 목록 안에서 이동합니다 |
| Shift와 아래쪽 화살표 또는 위쪽 화살표 | 이동하면서 다음 옵션을 켜거나 끕니다 |
| Space, Enter | 활성 옵션을 켜거나 끕니다. 목록은 열린 상태로 유지됩니다 |
| 문자 | 그 문자로 시작하는 다음 옵션으로 이동합니다 |
| Backspace, Delete | 닫힌 콤보박스에서 마지막으로 선택한 옵션을 제거합니다 |
| Escape, Alt와 위쪽 화살표 | 목록을 닫습니다. 포커스는 콤보박스에 남습니다 |
| Tab | 목록을 닫고 다음으로 이동합니다 |

Ctrl+A(APG 패턴의 모두 선택)는 단축키 정책에 따라 브라우저에 맡기며, 칩의 제거 버튼은 탭 순서에 있습니다. 목록, 칩, lookup 열에는 gzip 기준 약 1.7 KB의 스크립트가 필요하고 요청은 필요 없습니다. 필드는 노드 3개에 선택된 옵션마다 하나가 더해집니다.

## Decimal과 퍼센트

`Decimal` 필드는 정확한 숫자를 담습니다. 입력, 서버, 데이터베이스, 내보내기 사이 어디에도 부동 소수점이 없습니다. 입력은 타이핑하는 동안 사용자의 언어로 숫자를 서식화하고(터키어는 `1.234,5678`, 영어는 `1,234.5678`) 필드의 `scale`만큼의 소수 자릿수를 허용하며, 서버는 같은 언어로 이를 읽고 `scale`, `precision`, `min`, `max`를 다시 검사합니다.

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent`는 값을 퍼센트 포인트로 유지합니다. `12.5`는 영어에서 `12.5 %`, 터키어에서 `%12,5`로 표시되고 `12.5`로 저장됩니다. 스크린 리더는 입력의 설명을 통해 "percent"라는 단어를 읽어 줍니다.

## 마스크 입력

`mask` 규칙이 있는 텍스트 필드는 입력하는 동안 패턴을 보여 주고 원시 문자만 전송합니다. 패턴이 곧 규칙입니다. 서버는 입력된 리터럴을 제거하여 읽고(`(532) 123 45 67`은 `5321234567`이 됩니다), `A` 슬롯을 대문자로 바꾸며, 모든 슬롯을 채우지 않은 값은 거부합니다. API 호출과 AI 도구는 원시 값을 보내며 같은 검사를 받습니다.

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| 패턴 문자 | 의미 |
|---|---|
| `9` | 숫자 |
| `a` | 문자 |
| `A` | 대문자로 기록되는 문자 |
| `*` | 문자 또는 숫자 |
| 백슬래시 | 다음 문자가 리터럴임을 뜻합니다. 예: `\T` |
| 그 밖의 모든 문자 | 리터럴: 표시되지만 저장되지 않습니다 |

런타임이 없으면 필드는 원시 값을 가진 일반 텍스트 입력이며, 사용자는 리터럴을 포함하거나 포함하지 않고 입력할 수 있습니다.

## 비밀번호

`Control::Password`(로그인), `Control::NewPassword`(가입, 변경), `Control::PasswordMeter`(강도 표시기가 있는 새 비밀번호)는 서버에서 값을 채우지 않고 초안에도 저장하지 않는 비밀번호 입력입니다. 표시와 숨김 버튼은 APG 토글 버튼 패턴을 따릅니다. 이름은 "Show password"로 유지되며 `aria-pressed`가 텍스트가 표시되는지 알려 줍니다. 폼은 전송할 때 비밀번호를 다시 숨김 상태로 되돌립니다.

표시기는 길이, 문자 종류, 뻔한 패턴으로 브라우저에서 계산하는 힌트이며, 막대 네 개와 정중한 라이브 리전의 텍스트("Strength: good")로 표시됩니다. 서버가 무엇을 허용하는지는 필드의 규칙(`min_len`, 모델의 `validate` 함수)이 결정하며, 표시기는 결정하지 않습니다.

@live inputs

### 키보드

| 키 | 위치 | 동작 |
|---|---|---|
| 위쪽 화살표, 아래쪽 화살표 | Decimal 및 퍼센트 입력 | `min`과 `max` 안에서 정수 한 단위씩 증감합니다 |
| Page Up, Page Down | Decimal 및 퍼센트 입력 | 10씩 증감합니다 |
| Backspace | 마스크 입력 | 캐럿 앞의 문자를 지웁니다. 리터럴 위에서는 리터럴 앞의 문자를 지웁니다 |
| Tab | 비밀번호 입력 | 표시와 숨김 버튼으로 이동합니다 |
| Enter, Space | 표시와 숨김 버튼 | 비밀번호를 표시하거나 숨깁니다. 포커스는 버튼에 남습니다 |

### 예산과 검사

| | |
|---|---|
| 추가된 스크립트 | 마스크, 토글, 표시기를 위해 `inputs.js`에 gzip 기준 약 2 KB(원본 7 KB). Decimal과 퍼센트 입력은 숫자 컨트롤을 사용합니다 |
| DOM 노드 | 마스크 입력은 3개, 토글이 있는 비밀번호는 4개, 표시기가 있으면 11개 |
| 요청 | 입력하는 동안에는 없음 |
| 서버 검사 | `scale`, `precision`, `min`, `max`, `mask`는 페이지, API, AI 도구 모두에서 저장할 때마다 실행됩니다 |

## 코드 편집기

`Control::Code(lang)`는 텍스트 필드를 SQL(`CodeLang::Sql`), `[Orders.Total] * 1.18m` 같은 보고서 식(`CodeLang::Expression`) 또는 일반 텍스트를 위한 코드 편집기로 바꿉니다. `CodeEditor`는 같은 편집기를 모델 폼 밖에서 독립된 노드로 쓰는 것입니다. `.complete(action)`은 제안을, `.check(action)`은 오류 표시를 추가합니다.

```rust
Form::of(calc)
    .control(Calc::formula, Control::Code(CodeLang::Expression))
    .complete(Calc::formula, complete_fields)
    .check(Calc::formula, check_formula)

CodeEditor::new("query", CodeLang::Sql).label("Report query").value(sql).complete(complete_sql)
```

입력은 일반 `<textarea>`이므로 입력, 실행 취소, 입력기, 모바일 키보드, 화면 낭독기가 다른 텍스트 필드와 똑같이 동작하고, 폼은 일반 텍스트를 보냅니다. 그 아래에는 텍스트를 강조 표시한 사본과 줄 번호 열이 있으며, 둘 다 화면 낭독기에서 숨겨집니다. 강조 표시는 줄 단위라서 키 입력 한 번은 바뀐 줄만 다시 그립니다. 스크립트가 없으면 필드는 일반 텍스트 영역입니다.

완성 액션은 `CodeQuery`(`text`와 `cursor`, 입력 중인 단어는 `word()`)를 받아 `Vec<Completion>`을 반환하며, 고른 제안의 `insert`가 그 단어를 대신합니다. 검사 액션은 같은 쿼리를 받아 `line`, `column`, `length`, `message`가 담긴 `Vec<CodeError>`를 반환합니다. 각 오류에는 밑줄이 그어지고 편집기 아래에 나열되며, 화면 낭독기는 그곳에서 이를 읽습니다. 모델의 `validate` 함수에서는 `errors.add_at(field, line, column, length, message)`로도 그 위치에 밑줄을 긋습니다. 텍스트 밖에 있는 표시는 텍스트 안으로 옮겨집니다.

### 키보드

| 키 | 동작 |
|---|---|
| Ctrl+Space | 커서 위치의 제안을 보여 줍니다 |
| 아래쪽 화살표, 위쪽 화살표 | 제안 사이를 이동합니다 |
| Enter, Tab | 활성 제안을 삽입합니다 |
| Escape | 제안을 닫습니다 |
| Tab(열린 제안이 없을 때) | 다른 텍스트 영역처럼 포커스를 편집기 밖으로 옮깁니다 |

### 예산과 검사

| | |
|---|---|
| 추가 스크립트 | `codeedit.js` 약 8.0 KB(gzip)와 `codeedit.css` 1.4 KB(gzip), 편집기가 있는 페이지에서만 |
| 입력 | 2,000줄 텍스트에서 키 입력당 약 0.5 ms의 스크립트 |
| 요청 | 키 입력마다 없음. 완성은 150 ms, 검사는 500 ms 멈춘 뒤에 보내고 이전 요청은 취소됩니다 |
| 서버 검사 | 모든 요청에서 CSRF, 액션 권한, 쓰기 제한을 확인합니다. 먼저 요청 본문을 읽고(기본 본문 제한으로 최대 2 MB), 그다음 64 KB를 넘는 텍스트를 액션 실행 전에 거부합니다. 완성과 검사는 저장처럼 `App::write_limit`에 포함되며, 브라우저에는 최대 200자의 제안 50개와 500자의 오류 100개만 전달됩니다 |

## 날짜 및 시간 피커

피커는 사용자의 형식으로 입력한 값을 받고 달력이나 시간 목록을 엽니다. `DateOptions`로 제한을 설정합니다. `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`가 있습니다. `.range(from, to)`는 두 날짜 필드를 *This month* 같은 프리셋이 있는 하나의 범위 피커로 합칩니다. 라이브 예제는 [시간과 언어](time-and-languages.md#pickers)에 있습니다.
