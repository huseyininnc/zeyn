# Text and content

Reading pages such as articles, profiles and listings are built from a few small components. They are plain HTML rendered on the server, with no script, and every text goes through the locale files or is escaped.

@live content

## Heading

`Heading::h2(text)` to `Heading::h6(text)`. The screen title is the page's only `h1`, so there is no `h1` here: pick the level that follows the outline, not the size you want. `.id("anchor")` lets links point at it.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

A paragraph of escaped text. `.muted()` uses the secondary text color and `.small()` the small font size. Long words wrap.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

Short facts on one line, such as a date, a reading time and a category. The separator (a middle dot unless `.separator("/")` says otherwise) is hidden from screen readers, and empty parts are left out.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

Links to the previous and the next item, with `rel="prev"` and `rel="next"`. They open without a page reload. The words "Previous" and "Next" come from the framework's texts; `.prev_title` and `.next_title` add the item's title under them. The arrows turn around in right to left languages, and on phones the two links stack.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

Children in a grid whose column count follows the available width: as many columns of at least `min` pixels (240 unless set) as fit, and one column on a phone. The browser lays it out with `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`, so there are no breakpoints and no script. It holds any nodes: cards, panels, grids. The name keeps it apart from `DataGrid`, which shows rows of data.

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

Rows one under the other in a `<ul>`. Each `ListItem` has a title and may have a small leading image (the same safe addresses as card images: a path, `https:` or `data:image/`), a subtitle and short meta text at the end. With `.link(screen, params)` the whole row is one link, so it is one stop in the tab order with a visible focus ring, and it opens without a page reload. Long text wraps; on phones the meta text moves under the title, and right to left pages mirror the row.

`List::new()` holds the items it is given. `List::of(action)` loads rows through the same kind of action a DataGrid uses and pages like a [card grid](catalog.md#card-grid): 20 rows with a "Show more" button unless `paging` says otherwise, sort options, server side search with `.search(text)`, and the action's access checked for every page.

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

A round picture of a person or a team, 32, 40 or 64 pixels (`AvatarSize::Small`, `Medium`, `Large`). The name is required: it is the alternative text and the source of the initials shown when there is no picture, or when the picture is not on a host the app's `img-src` allows (the same rule as `Image`). Initials follow the language's capitals, so Turkish `i` becomes `İ`. When the name is written right next to the avatar, `.decorative()` hides it from screen readers so they do not read the name twice.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

Content hidden until the reader opens it, such as the answer to a question. It is a native disclosure (`<details>` and `<summary>`), so it works with the keyboard and screen readers without any script, and the content stays out of the reading order until it is opened. The label comes from the app.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

A bar of letters, each a link to a screen with the letter in its params, such as people whose names start with it. The letters come from the user's language in its own order (`zeyn::ui::alphabet`): Turkish has `Ç Ğ I İ Ö Ş Ü` in place, Spanish has `Ñ`, Russian and Arabic have their own alphabets, and every other language gets `A` to `Z`. The letter of the screen being shown is marked with `aria-current="page"` (a Turkish `i` marks `İ`), and `.other()` adds a last `#` link for names that start with anything else. The links open without a page reload and wrap onto more lines on phones.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```
