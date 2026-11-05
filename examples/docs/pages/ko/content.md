# 텍스트와 콘텐츠

기사, 프로필, 목록 같은 읽기 중심 페이지는 몇 가지 작은 컴포넌트로 만듭니다. 이들은 서버에서 렌더링되는 일반 HTML이며 스크립트가 없고, 모든 텍스트는 locale 파일에서 가져오거나 이스케이프됩니다.

@live content

## Heading

`Heading::h2(text)`부터 `Heading::h6(text)`까지 있습니다. screen 제목이 페이지의 유일한 `h1`이므로 여기에는 `h1`이 없습니다. 원하는 크기가 아니라 문서 구조상 이어지는 수준을 고르십시오. `.id("anchor")`를 쓰면 링크가 이 제목을 가리킬 수 있습니다.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

이스케이프된 텍스트 단락입니다. `.muted()`는 보조 텍스트 색을, `.small()`은 작은 글꼴 크기를 사용합니다. 긴 단어는 줄 바꿈됩니다.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

날짜, 읽는 시간, 카테고리처럼 짧은 정보를 한 줄에 보여 줍니다. 구분 기호(`.separator("/")`로 바꾸지 않으면 가운뎃점)는 스크린 리더에서 숨겨지고, 비어 있는 부분은 생략됩니다.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

이전 항목과 다음 항목으로 가는 링크이며 `rel="prev"`와 `rel="next"`가 붙습니다. 페이지를 다시 불러오지 않고 열립니다. "이전"과 "다음"이라는 단어는 프레임워크의 텍스트에서 가져오며, `.prev_title`과 `.next_title`은 그 아래에 항목의 제목을 추가합니다. 화살표는 오른쪽에서 왼쪽으로 쓰는 언어에서는 방향이 바뀌고, 휴대폰에서는 두 링크가 위아래로 쌓입니다.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

열 개수가 사용 가능한 너비에 따라 달라지는 그리드에 자식을 배치합니다. 너비가 `min` 픽셀 이상(설정하지 않으면 240)인 열이 들어갈 수 있는 만큼 배치되고, 휴대폰에서는 한 열입니다. 브라우저가 `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`로 배치하므로 중단점도 스크립트도 없습니다. 카드, 패널, 그리드 등 어떤 노드든 담을 수 있습니다. 이름은 데이터 행을 보여 주는 `DataGrid`와 구분하기 위한 것입니다.

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

`<ul>` 안에 행을 위아래로 나열합니다. 각 `ListItem`에는 제목이 있고, 시작 부분의 작은 이미지(카드 이미지와 같은 안전한 주소: 경로, `https:` 또는 `data:image/`), 부제목, 끝에 짧은 메타를 둘 수 있습니다. 텍스트나 `RelativeTime` 같은 인라인 컴포넌트를 쓸 수 있습니다. `.link(screen, params)`를 쓰면 행 전체가 하나의 링크가 되므로 탭 순서에서 한 지점이고, 포커스 링이 눈에 보이며, 페이지를 다시 불러오지 않고 열립니다. 긴 텍스트는 줄 바꿈되며, 휴대폰에서는 메타 텍스트가 제목 아래로 이동하고, 오른쪽에서 왼쪽으로 쓰는 페이지에서는 행이 좌우 반전됩니다.

`List::new()`는 받은 항목을 그대로 담습니다. `List::of(action)`는 DataGrid가 쓰는 것과 같은 종류의 action으로 행을 불러오고 [카드 그리드](catalog.md#card-grid)처럼 페이징합니다. `paging`이 달리 지정하지 않으면 20행과 "Show more" 버튼, 정렬 옵션, `.search(text)`를 이용한 서버 측 검색을 제공하며, 페이지마다 action의 접근 권한을 확인합니다.

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
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

사람이나 팀의 둥근 사진으로, 크기는 32, 40, 64픽셀입니다(`AvatarSize::Small`, `Medium`, `Large`). 이름은 필수입니다. 이름은 대체 텍스트이면서, 사진이 없거나 사진이 앱의 `img-src`가 허용하는 호스트에 있지 않을 때 표시되는 이니셜의 원본입니다(`Image`와 같은 규칙). 이니셜은 해당 언어의 대문자 규칙을 따르므로 튀르키예어의 `i`는 `İ`가 됩니다. 이름이 아바타 바로 옆에 적혀 있을 때는 `.decorative()`가 아바타를 스크린 리더에서 숨겨 이름이 두 번 읽히지 않게 합니다.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

독자가 열기 전까지 숨겨 두는 콘텐츠이며, 예를 들어 질문의 답이 있습니다. 기본 제공 펼침 요소(`<details>`와 `<summary>`)이므로 스크립트 없이도 키보드와 스크린 리더로 사용할 수 있고, 열기 전까지 콘텐츠는 읽기 순서에 포함되지 않습니다. 레이블은 앱에서 정합니다.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## Chips

여러 줄로 이어지는 작은 둥근 링크의 한 줄입니다. `Chips::links(label)`은 문서의 태그처럼 화면으로 연결되는 일반 링크를 label로 이름 붙인 `nav` 안에 담습니다. `Chips::filter(label, screen, params)`는 화면 파라미터에 대한 토글 버튼을 만듭니다. 각 chip은 같은 화면으로 연결되는 링크이며 해당 값이 파라미터에 추가되거나 제거되므로, 서버가 받은 파라미터로 `aria-pressed`를 렌더링하고 브라우저에는 아무것도 저장되지 않습니다. chip을 누르면 `nav.js`를 통해 페이지 새로고침 없이 새 주소가 열리고, 포커스와 스크롤 위치는 chip에 남습니다. `.multiple()`을 호출하지 않으면 한 번에 하나의 값만 눌립니다. 파라미터에서 옵션이 아닌 값은 무시됩니다. 키보드: Tab은 chip 사이를 이동하고, Enter 또는 Space로 누릅니다. 눌린 chip은 체크 표시도 보여 주므로 상태가 색에만 의존하지 않습니다. 스크립트(`chips.js`, 1 KB 미만)는 필터 chip이 있는 페이지에서만 로드됩니다.

```rust
#[screen(route = "/articles", public)]
pub fn articles(p: ArticleParams) -> Screen {
    Screen::new(t!("blog.articles")).body(
        Chips::filter(t!("blog.tags"), articles, |tags| ArticleParams { tags: tags.join(",") })
            .current(p.tags.split(','))
            .option("guide", t!("blog.guides"))
            .option("news", t!("blog.news"))
            .multiple(),
    )
}
```

@live chips


## Hero

페이지 맨 위의 큰 영역으로, 배경 이미지에 제목, 한 줄의 텍스트, 버튼 같은 동작이 들어갑니다. 기본적으로 정적입니다. `.slide(title)`은 슬라이드를 하나 더 추가하며, `.text`, `.image`, `.action`은 항상 마지막 슬라이드에 적용됩니다. 슬라이드가 여러 개이면 W3C APG 캐러셀 패턴을 따릅니다. 슬라이드 선택 버튼이 있고, 각 슬라이드는 "2 / 3"이라는 이름의 그룹이며, 선택된 슬라이드는 정중하게 안내됩니다. 앱이 `.rotate(seconds)`(4에서 30)를 호출하지 않는 한 슬라이드는 사용자가 선택할 때만 바뀝니다. 호출하면 일시정지 및 재생 버튼이 맨 앞에 오고(WCAG 2.2.2), 포인터가 hero 위에 있거나 포커스가 그 안에 있을 때와 탭이 숨겨져 있을 때 회전이 멈추며, 사용자가 동작 줄이기를 선호하면 시작되지 않습니다. 슬라이드가 움직이는 동안에는 안내되지 않습니다. 스크립트가 없으면 첫 슬라이드가 표시됩니다.

이미지는 `Image` 규칙(앱의 경로 또는 앱의 `img-src`에 있는 호스트의 `https` 주소)을 따릅니다. 텍스트 뒤의 장식이므로 대체 텍스트는 비어 있습니다. 중요한 내용은 제목과 텍스트에 넣으십시오. 이 둘은 대비를 위해 어두운 그라데이션 위에 놓입니다. 첫 슬라이드의 이미지는 빠른 첫 페인트를 위해 높은 우선순위로 로드되고, 나머지는 지연 로드됩니다. 스크립트(`hero.js`, 1.6 KB gzip)는 슬라이드가 있는 페이지에서만 로드됩니다.

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero


## Reactions

앱이 고른 반응 버튼의 한 줄로, 각 버튼은 키, 이름, 이모지 같은 기호, 그리고 횟수를 가집니다. `Reactions::of(action, target)`은 반응이 어떤 항목에 대한 것인지 지정합니다. target은 화면이 정하며 브라우저에서 읽지 않습니다. 버튼을 누르면 반응이 `POST /_zeyn/data`로 전송되며, 이 엔드포인트는 CSRF를 확인하고, 쓰기 속도 제한에 포함되며, 화면을 열고, 액션의 권한을 확인하고, 액션이 실행되기 전에 집합 밖의 반응을 거부합니다. 액션은 `ReactionPress`(target, reaction, pressed)를 받아 저장하고 새 `ReactionState`(횟수와 사용자 본인의 반응)를 반환하며, 횟수와 `aria-pressed` 상태는 그 자리에서 갱신됩니다. 각 버튼은 반응의 이름과 횟수로 이름 붙은 toggle button이며, 기호는 스크린 리더에서 숨겨집니다. `.state(...)`는 첫 렌더링의 횟수를 지정하므로 스크립트 없이도 표시됩니다. 스크립트(`reactions.js`, 1.3 KB gzip)는 반응이 있는 페이지에서만 로드됩니다.

```rust
#[action(permission = "answers.react")]
pub async fn react_to_answer(press: ReactionPress, db: Db, ctx: Ctx) -> Result<ReactionState> {
    db.toggle_reaction(&ctx, &press.target, &press.reaction, press.pressed).await?;
    db.reaction_state(&ctx, &press.target).await
}

Reactions::of(react_to_answer, format!("answer-{}", answer.id))
    .reaction("helpful", t!("answers.helpful"), "\u{1f44d}")
    .reaction("thanks", t!("answers.thanks"), "\u{1f64f}")
    .state(answer.reactions.clone())
```

@live reactions


## RelativeTime

"3분 전"이나 "2일 후"처럼 페이지의 언어로 상대 시간으로 표시되는 시각입니다. 서버는 사용자의 시간대 기준 정확한 날짜와 시간을 `<time datetime="...">` 안에 쓰므로 HTML은 분마다 같게 유지되고 캐시된 페이지도 여전히 올바릅니다. 런타임은 페이지가 로드될 때, `nav.js`가 화면을 교체할 때, 그리고 1분마다 `Intl.RelativeTimeFormat`으로 텍스트를 다시 씁니다. 정확한 시간은 툴팁에 남습니다. 스크립트가 없으면 정확한 시간이 텍스트입니다.

```rust
RelativeTime::new(post.published)
```


## Share

링크를 공유하는 버튼입니다. 브라우저에 Web Share API가 있으면(대부분의 휴대폰) 시스템 공유 시트를, 없으면 그 사실을 알리는 토스트와 함께 클립보드에 복사합니다. 표시 중인 페이지를 공유하거나, 컴파일러가 파라미터를 검사하는 `Share::screen(screen, params)`로 화면을 공유합니다. `.title`은 공유 시트가 보여 주는 제목을 정하며, 레이블은 `.label`로 지정하지 않으면 사용자의 언어로 "공유"입니다. 서버는 버튼을 숨긴 상태로 렌더링하고 스크립트가 이를 표시하므로, 스크립트가 없는 페이지에는 아무 일도 하지 않는 버튼이 없습니다. 스크립트(`share.js`, 1 KB 미만)는 공유 버튼이 있는 페이지에서만 로드됩니다.

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


## AlphaIndex

글자로 이루어진 막대이며, 각 글자는 그 글자를 파라미터로 가진 screen으로 가는 링크입니다. 예를 들어 이름이 그 글자로 시작하는 사람들을 보여 줄 때 씁니다. 글자는 사용자 언어의 고유한 순서를 따릅니다(`zeyn::ui::alphabet`). 튀르키예어는 `Ç Ğ I İ Ö Ş Ü`가 제자리에 있고, 스페인어에는 `Ñ`가 있으며, 러시아어와 아랍어는 각자의 알파벳을 쓰고, 그 밖의 모든 언어는 `A`부터 `Z`까지입니다. 현재 보이는 screen의 글자는 `aria-current="page"`로 표시되며(튀르키예어 `i`는 `İ`를 표시), `.other()`는 그 밖의 문자로 시작하는 이름을 위해 마지막에 `#` 링크를 추가합니다. 링크는 페이지를 다시 불러오지 않고 열리며, 휴대폰에서는 여러 줄로 나뉩니다.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

action에서 가져온 행을 요일별 일곱 개의 열로 보여 줍니다. 한 주의 수업, 근무, 이벤트 등에 사용합니다. action은 DataGrid가 쓰는 것과 같은 종류이며 같은 방식으로 검사됩니다. `.day`는 행을 해당 요일에 대응시킵니다. `Date`, `Option<Date>`(날짜가 없는 행은 제외됨), 또는 1(월요일)부터 7(일요일)까지의 요일 번호를 쓸 수 있으며, 그 밖의 것은 컴파일되지 않습니다. 각 행은 `ListItem`으로 표시되므로 screen으로 링크할 수 있습니다. 한 주는 사용자의 첫 요일에 시작하고(튀르키예어는 월요일, 미국 영어는 일요일), 오늘이 표시되며, 행이 없는 날은 그렇다고 알려 주고, 좁은 화면에서는 요일이 위아래로 쌓입니다. 서버는 action의 순서대로 최대 `limit`개의 행(설정하지 않으면 100, 최대 500)을 불러옵니다. action에는 그 주의 행만 주십시오. 예를 들어 날짜에 필터를 쓰면 됩니다.

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
