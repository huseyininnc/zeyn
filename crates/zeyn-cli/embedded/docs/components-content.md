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
    .shortcut("Alt+Shift+N")
```

### Link shortcuts

`.shortcut(keys)` gives the side set last a keyboard shortcut, and `Chips::links(...).link(...).shortcut(keys)` gives one to the link chip added last. Pressing the keys follows the link through `nav.js` without a page reload, on app shell and site shell pages alike. The keys do nothing while focus is in an input, a text area, a select or an editable area, so typing is never taken over. The link carries `aria-keyshortcuts` for screen readers and shows the keys in a `<kbd>` hint, which touch screens hide. Shortcuts are read like `Command::shortcut`: shortcuts the browser keeps (`Ctrl+T`, `Ctrl+W`) or uses (`Ctrl+P`, `F5`) are not bound. `App::check` reports link shortcuts in the site header slots that cannot be read, belong to the browser, are a single character key (WCAG 2.2, 2.1.4: `Shift+N` counts, `Alt+Shift+N` does not) or clash with a command, the palette or another link; `App::check_page(&screen)` does the same for a screen built in a test with its params, since `App::check` cannot build screens itself.

Keys pressed while focus is inside a frame from another origin, such as an embedded video player, go to that frame's document and never reach the page; no script can change that. Tell users to click outside the frame, or press Tab to leave it, before using a shortcut.

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

Rows one under the other in a `<ul>`. Each `ListItem` has a title and may have a small leading image (the same safe addresses as card images: a path, `https:` or `data:image/`), a subtitle and short meta at the end: text, or an inline component such as `RelativeTime`. With `.link(screen, params)` the whole row is one link, so it is one stop in the tab order with a visible focus ring, and it opens without a page reload. Long text wraps; on phones the meta text moves under the title, and right to left pages mirror the row.

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
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name)
    .paging(Paging::pages(20))
```

@live list

### Sortable lists

`.sortable(action)` lets people put the rows of `List::of` or a `CardGrid` in their own order. Each row gets a handle: a mouse or pen drags it, a touch drags it after a short hold, and a line shows where it lands. On the focused handle Space lifts the row, the arrow keys move it, Space drops it and Escape puts it back; Alt with Up or Down moves it at once. A click or Enter opens a move menu (up, down, to the top, to the end), so one pointer is enough. Every step is announced.

The move shows at once and the action gets a `ReorderRequest` with the key of the row and the keys of its new neighbors (`after`, `before`), never positions. `r.rank(lookup)` gives the new `Rank` between the neighbors and refuses a neighbor the lookup does not find (`NotFound`) or neighbors that are no longer in this order (`Conflict`, someone else moved rows first); then the row goes back and the reason is shown. The source action lists the rows by rank, so sort options are left out, and while more rows are still to load no row can become the last one.

```rust
#[action(permission = "tasks.write")]
pub async fn reorder_tasks(r: ReorderRequest<TaskId>, db: Db) -> Result<()> {
    let rank = r.rank(|k| db.rank_of(*k))?;
    db.set_rank(r.key, rank).await
}

List::of(search_tasks)
    .item(|t: &Task| ListItem::new(t.title.clone()))
    .sortable(Task::rank, reorder_tasks)
```

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

## AvatarGroup

The people of one thing, such as the assignees of a task, as avatars that overlap in a row. The label names the group for screen readers. The group shows the first avatars, 5 unless `.max(n)` says otherwise and never more than 12, and then the number of the rest as "+3", read as "3 more" in the user's language. `.size(...)` sets the size of every avatar of the group.

```rust
AvatarGroup::new(t!("task.assignees"))
    .add(Avatar::new(a.name.clone()).image(a.photo.clone()))
    .add(Avatar::new(b.name.clone()))
    .max(3)
    .size(AvatarSize::Small)
```

## Quote

Words quoted from a person or a work, as a `blockquote` in a `figure`. `.author(name)` says who said them and `.source(title)` names the work, written as a title (`cite`).

```rust
Quote::new(review.text.clone()).author(review.name.clone()).source(t!("shop.verified_buyer"))
```

## Kbd

Keys to press, written as keyboard input (`kbd`). `Kbd::new("Ctrl+K")` reads a combination with `+` between its keys; `Kbd::keys(["Ctrl", "+"])` takes the keys one by one, for a combination that holds the plus key itself. Key names are written as given, at most 6 of them. It only shows the keys; a shortcut that does something is set where the command is, as `PrevNext::shortcut` does.

```rust
Kbd::new("Alt+Shift+P")
```

## Divider

A line between two parts of a page. `Divider::new()` is a thematic break (`hr`). `.label(text)` puts a text in the middle of the line, such as "or" between two ways to sign in; the divider is then a `separator` named by that text. `.vertical()` draws the line from top to bottom, for the space between the columns of a compact `Row`.

```rust
Divider::new().label(t!("signin.or"))
```

@live pieces

## Highlight

`Highlight::new(text, query)` writes a text with the words of a search marked in it (`mark`), such as the name of a product in a list of results. The query is split at white space, at most 8 words of it are looked for, and every place one of them is found is marked without regard to case; an empty query marks nothing. Both the text and the query are user data: the text is escaped, and the query is only compared, never written to the page. Letters are compared in lower case as Unicode defines it, so the Turkish pairs `I ı` and `İ i` are not folded together.

```rust
Highlight::new(product.name.clone(), p.q.clone())
```

## Toc

A table of contents: links to the headings of the page being shown, for long pages such as an article or a policy. `Toc::new(label)` is a `nav` named by the label; `.item(title, id)` links the heading that was given the same anchor with `Heading::id`, and `.sub(title, id)` adds an entry one level under the entry before it. Anchors are cleaned as `Heading::id` cleans them, so the same string works in both places, and an entry whose anchor comes out empty is left out; a table of contents holds at most 100 entries. The links are plain links to `#anchor`, so they work without script, the browser scrolls to the heading, and the address can be shared. Nothing marks the section being read, since that would need a script.

```rust
Toc::new(t!("page.contents"))
    .item(t!("policy.delivery"), "delivery")
    .sub(t!("policy.abroad"), "delivery-abroad")
    .item(t!("policy.returns"), "returns")
Heading::h2(t!("policy.delivery")).id("delivery")
```

@live marks

## Stepper

Where a process stands: its steps in order, the ones done, the current one and the ones still to come, such as the way of an order from placed to delivered. `.step(title)` adds a step and `.description(text)` puts a line under the step added last. `.current(index)` counts from 0 and marks the steps before it done; `.finished()` marks every step done. `.vertical()` puts the steps one under the other, which is also how they show on narrow screens. A stepper holds at most 12 steps.

```rust
Stepper::new(t!("order.progress"))
    .step(t!("order.placed"))
    .step(t!("order.packed"))
    .step(t!("order.shipped"))
    .description(order.shipped_note.clone())
    .step(t!("order.delivered"))
    .current(order.stage as usize)
```

@live stepper

The steps are an ordered list named by the label. The current step carries `aria-current="step"`, and a done step says "completed" to screen readers in the user's language, so the state is not given by color alone. A stepper only shows the state the server gives it and nothing in it can be pressed; a form filled in steps is a `Form` with its own steps (see [Forms](forms.md)).

## Pagination

Links to the pages of a list that a screen pages itself, such as the articles of a blog: previous, the page numbers and next. Each one is a link to the same screen with the page in its params, so the server renders the page it is asked for, the address can be shared, and nothing is kept in the browser. The links open through `nav.js` without a page reload and work without script. `DataGrid`, `List` and `CardGrid` page themselves with `.paging(...)`; use this only where the screen builds its own content.

```rust
#[screen(route = "/blog", public)]
pub fn blog(p: BlogParams) -> Screen {
    Screen::new(t!("blog.title")).body(
        Pagination::new(blog, |page| BlogParams { page })
            .pages(page_count)
            .current(p.page),
    )
}
```

Pages count from 1. The first and the last page always show, with one page on each side of the current one; `.around(n)` shows more, up to 4. Pages left out become a gap, and a gap never hides a single page. The page number comes from the address, so the screen brings it into range before it loads data; the component does the same for what it marks. With one page or none it renders nothing.

The links are a `nav` named "Pagination" in the user's language. The current page carries `aria-current="page"`, every number is named "Page 3" for screen readers, previous and next carry `rel="prev"` and `rel="next"`, and at the ends they are plain text marked `aria-disabled` instead of links that go nowhere.

| Key | What it does |
|---|---|
| `Tab` | Moves through previous, the page numbers and next |
| `Enter` | Opens the page; focus and the scroll position follow `nav.js` |

## Chips

A row of small rounded links that wrap onto more lines. `Chips::links(label)` holds plain links to screens, such as the tags of an article, in a `nav` named by the label. `Chips::filter(label, screen, params)` makes toggle buttons over a screen param: each chip is a link to the same screen with its value added to or taken out of the param, so the server renders `aria-pressed` from the params it got and nothing is kept in the browser. Pressing a chip opens the new address through `nav.js` without a page reload, and focus and the scroll position stay on the chip. One value is pressed at a time unless `.multiple()` is called; values in the params that are not options are ignored. Keyboard: Tab moves between chips, Enter or Space presses one, and a link chip can have its own shortcut (see [Link shortcuts](#link-shortcuts)). A pressed chip also shows a check mark, so the state does not rest on color. The script (`chips.js`, under 1 KB) loads only on pages with filter chips.

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

A large section at the top of a page: a background image with a title, a line of text and actions such as buttons. `.link(label, screen, params)` adds a `LinkButton` to another page: the first one of a slide has the primary look, later ones the secondary look. It is static by default. `.slide(title)` adds another slide, and `.text`, `.image` and `.action` always set the last one. With several slides it follows the W3C APG carousel pattern: slide picker buttons, each slide a group named "2 of 3", and the picked slide announced politely. Slides change only when the user picks one unless the app calls `.rotate(seconds)` (4 to 30). Then a pause and play button comes first (WCAG 2.2.2), rotation stops while the pointer is over the hero or focus is inside it and while the tab is hidden, and it never starts when the user prefers reduced motion; the slides are not announced while they move. Without script the first slide shows.

The image follows the `Image` rules (a path of the app or an `https` address on a host of the app's `img-src`). It is decoration behind the text, so its alternative text is empty: put what matters into the title and the text, which sit on a dark scrim that keeps white text at a contrast of at least 4.5:1 over any picture. Link buttons keep their solid colors and get a white focus ring over the image. The first slide's image loads with high priority for a fast first paint, the others lazily. The script (`hero.js`, 1.6 KB gzip) loads only on pages with slides.

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .link(t!("home.menu"), menu, ())
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero

## Reactions

A row of reaction buttons chosen by the app, each with a key, a name and a symbol such as an emoji, and its count. `Reactions::of(action, target)` names the item the reactions are about; the target is set by the screen and never read from the browser. Pressing a button posts the reaction to `POST /_zeyn/data`, which checks CSRF, counts against the write rate limit, opens the screen, checks the action's permission and refuses a reaction outside the set before the action runs. The action gets a `ReactionPress` (target, reaction, pressed), stores it and returns the new `ReactionState` (counts and the user's own reactions), and the counts and `aria-pressed` states update in place. Each button is a toggle button named by the reaction's name and its count; the symbol is hidden from screen readers. `.state(...)` gives the counts for the first render, so they show without script. The script (`reactions.js`, 1.3 KB gzip) loads only on pages with reactions.

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

A moment shown as relative time, such as "3 minutes ago" or "in 2 days", in the page's language. The server writes the exact date and time in the user's time zone inside `<time datetime="...">`, so the HTML stays the same from minute to minute and a cached page is still right. The runtime rewrites the text with `Intl.RelativeTimeFormat` when the page loads, when `nav.js` swaps in a screen and once a minute; the exact time stays in the tooltip. Without script the exact time is the text.

```rust
RelativeTime::new(post.published)
```

## Share

A button that shares a link: the system share sheet where the browser has the Web Share API (most phones), and otherwise a copy to the clipboard with a toast that says so. It shares the page being shown, or a screen with `Share::screen(screen, params)`, whose params are checked by the compiler. `.title` sets the title the share sheet shows, and the label is "Share" in the user's language unless `.label` gives one. The server renders the button hidden and its script shows it, so a page without script has no button that does nothing. The script (`share.js`, under 1 KB) loads only on pages that hold a share button.

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage

## CopyButton

A button that copies a text to the clipboard: a tracking number, an API address, an invitation link. `.shown()` writes the text next to the button in a fixed width font, so it can also be read and selected; without it the text is in the page but hidden. The label is "Copy" in the user's language unless `.label` sets one, and a toast says whether the copy worked. It uses the same small script as `Share`, loaded only on pages that hold one, and is rendered hidden and shown by that script, so a page without script has no button that does nothing. The text is cut at 4096 bytes, and since it is part of the page it must be one the user may see, never a masked value.

```rust
CopyButton::new(order.tracking.clone()).shown()
CopyButton::new(invite_url).label(t!("team.copy_invitation"))
```

## AlphaIndex

A bar of letters, each a link to a screen with the letter in its params, such as people whose names start with it. The letters come from the user's language in its own order (`zeyn::ui::alphabet`): Turkish has `Ç Ğ I İ Ö Ş Ü` in place, Spanish has `Ñ`, Russian and Arabic have their own alphabets, and every other language gets `A` to `Z`. The letter of the screen being shown is marked with `aria-current="page"` (a Turkish `i` marks `İ`), and `.other()` adds a last `#` link for names that start with anything else. The links open without a page reload and wrap onto more lines on phones.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

Seven day columns of rows from an action, such as classes, shifts or events of a week. The action is the same kind a DataGrid uses and is checked the same way. `.day` maps a row to its day: a `Date`, an `Option<Date>` (rows without one are left out) or a weekday number from 1 (Monday) to 7 (Sunday); anything else does not compile. Each row is shown as a `ListItem`, so it can link to a screen. The week starts on the user's first weekday (Monday in Turkish, Sunday in American English), today is marked, days without rows say so, and on narrow screens the days stack. The server loads up to `limit` rows (100 unless set, at most 500) in the action's order; give the action the week's rows, for example with a filter on the date.

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
