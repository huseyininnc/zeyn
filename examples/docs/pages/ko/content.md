# 텍스트와 콘텐츠

기사, 프로필, 목록 같은 읽기용 페이지는 몇 가지 작은 컴포넌트로 만듭니다. 서버에서 렌더링되는 일반 HTML이며 스크립트가 없고, 모든 텍스트는 로케일 파일을 거치거나 이스케이프됩니다.

@live content

## Heading

`Heading::h2(text)`부터 `Heading::h6(text)`까지입니다. Screen 제목이 페이지의 유일한 `h1`이므로 여기에는 `h1`이 없습니다. 원하는 크기가 아니라 문서 구조에 맞는 수준을 고르세요. `.id("anchor")`를 쓰면 링크가 이 제목을 가리킬 수 있습니다.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

이스케이프된 텍스트로 된 문단입니다. `.muted()`는 보조 텍스트 색을, `.small()`은 작은 글꼴 크기를 사용합니다. 긴 단어는 줄바꿈됩니다.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

날짜, 읽는 시간, 분류처럼 한 줄에 놓이는 짧은 정보입니다. 구분자(`.separator("/")`로 따로 지정하지 않으면 가운뎃점)는 스크린 리더에서 숨겨지며, 비어 있는 부분은 빠집니다.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

`rel="prev"`와 `rel="next"`가 붙은 이전 항목과 다음 항목으로의 링크입니다. 페이지를 다시 불러오지 않고 열립니다. "Previous"와 "Next"라는 단어는 프레임워크의 텍스트에서 오며, `.prev_title`과 `.next_title`은 그 아래에 항목의 제목을 추가합니다. 오른쪽에서 왼쪽으로 쓰는 언어에서는 화살표 방향이 바뀌고, 휴대폰에서는 두 링크가 위아래로 쌓입니다.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

사용 가능한 너비에 따라 열 수가 정해지는 그리드에 자식 요소를 배치합니다. 너비가 최소 `min` 픽셀(지정하지 않으면 240)인 열을 들어가는 만큼 놓고, 휴대폰에서는 한 열이 됩니다. 브라우저가 `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`로 배치하므로 중단점도 스크립트도 없습니다. 카드, 패널, 그리드 등 어떤 노드든 담을 수 있습니다. 이 이름은 데이터 행을 보여 주는 `DataGrid`와 구별하기 위한 것입니다.

```rust
AutoGrid::new()
    .min(220)
    .gap(4)
    .add(Card::new().title(t!("home.orders")))
    .add(Card::new().title(t!("home.stock")))
    .add(Card::new().title(t!("home.people")))
```

@live autogrid

## List

`<ul>` 안에서 위아래로 놓이는 행입니다. 각 `ListItem`에는 제목이 있고, 앞쪽의 작은 이미지(카드 이미지와 같은 안전한 주소: 경로, `https:` 또는 `data:image/`), 부제목, 끝부분의 짧은 메타 텍스트를 둘 수 있습니다. `.link(screen, params)`를 쓰면 행 전체가 하나의 링크가 되므로, 탭 순서에서 보이는 포커스 링을 가진 하나의 정지점이 되고 페이지를 다시 불러오지 않고 열립니다. 긴 텍스트는 줄바꿈되며, 휴대폰에서는 메타 텍스트가 제목 아래로 이동하고, 오른쪽에서 왼쪽으로 쓰는 페이지에서는 행이 좌우 반전됩니다.

`List::new()`는 전달받은 항목을 담습니다. `List::of(action)`은 DataGrid가 사용하는 것과 같은 종류의 action으로 행을 불러오고 [카드 그리드](catalog.md#card-grid)처럼 페이지를 나눕니다. `paging`으로 따로 지정하지 않으면 "Show more" 버튼과 함께 20행을 보여 주며, 정렬 옵션, `.search(text)`를 통한 서버 측 검색이 있고, 모든 페이지에서 action의 접근 권한을 검사합니다.

```rust
List::new()
    .title(t!("team.title"))
    .item(ListItem::new("Ada Lovelace").subtitle(t!("team.math")))
    .item(ListItem::new("Alan Turing").subtitle(t!("team.computing")).meta(t!("team.lead")))

List::of(search_people)
    .title(t!("people.title"))
    .item(|p: &Person| {
        ListItem::new(p.name.clone())
            .image(p.photo.clone())
            .subtitle(p.city.clone())
            .meta(p.joined.format(&locale))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

사람이나 팀을 나타내는 32, 40 또는 64픽셀의 둥근 그림입니다(`AvatarSize::Small`, `Medium`, `Large`). 이름은 필수입니다. 이름은 대체 텍스트이자, 그림이 없거나 그림이 앱의 `img-src`가 허용하는 호스트에 있지 않을 때(`Image`와 같은 규칙) 표시되는 머리글자의 출처입니다. 머리글자는 해당 언어의 대문자 규칙을 따르므로 터키어 `i`는 `İ`가 됩니다. 이름이 아바타 바로 옆에 적혀 있으면 `.decorative()`가 스크린 리더에서 아바타를 숨겨 이름을 두 번 읽지 않게 합니다.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

질문의 답처럼 독자가 열 때까지 숨겨져 있는 콘텐츠입니다. 네이티브 펼침 요소(`<details>`와 `<summary>`)이므로 스크립트 없이도 키보드와 스크린 리더에서 동작하며, 열기 전까지 콘텐츠는 읽기 순서에서 빠져 있습니다. 레이블은 앱에서 제공합니다.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

글자가 나열된 막대로, 각 글자는 그 글자를 params에 담은 Screen으로 가는 링크입니다. 예를 들어 이름이 그 글자로 시작하는 사람들입니다. 글자는 사용자 언어의 고유한 순서로 나옵니다(`zeyn::ui::alphabet`). 터키어에는 `Ç Ğ I İ Ö Ş Ü`가 제자리에 있고, 스페인어에는 `Ñ`가 있으며, 러시아어와 아랍어는 각자의 알파벳을 쓰고, 그 밖의 모든 언어는 `A`부터 `Z`까지를 받습니다. 현재 표시 중인 Screen의 글자는 `aria-current="page"`로 표시되며(터키어 `i`는 `İ`를 표시), `.other()`는 다른 문자로 시작하는 이름을 위해 마지막에 `#` 링크를 추가합니다. 링크는 페이지를 다시 불러오지 않고 열리며 휴대폰에서는 여러 줄로 줄바꿈됩니다.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

한 주의 수업, 근무, 일정처럼 action에서 가져온 행을 일곱 개의 요일 열로 보여 줍니다. action은 DataGrid가 사용하는 것과 같은 종류이며 같은 방식으로 검사됩니다. `.day`는 행을 해당 요일에 연결합니다. `Date`, `Option<Date>`(날짜가 없는 행은 빠짐) 또는 1(월요일)부터 7(일요일)까지의 요일 번호를 쓸 수 있으며, 그 밖의 것은 컴파일되지 않습니다. 각 행은 `ListItem`으로 표시되므로 Screen에 링크할 수 있습니다. 주는 사용자의 첫 요일(터키어에서는 월요일, 미국 영어에서는 일요일)에 시작하고, 오늘이 표시되며, 행이 없는 날은 그렇다고 알리고, 좁은 화면에서는 요일이 위아래로 쌓입니다. 서버는 action의 순서대로 최대 `limit`행(지정하지 않으면 100, 최대 500)을 불러옵니다. 예를 들어 날짜에 필터를 걸어 action에 그 주의 행을 넘기세요.

```rust
WeekSchedule::of(this_week_classes)
    .title(t!("classes.this_week"))
    .day(|c: &Class| c.date)
    .item(|c: &Class| {
        ListItem::new(c.name.clone())
            .meta(c.starts.to_string())
            .link(class, ClassParams { id: c.id })
    })
```

@live week
