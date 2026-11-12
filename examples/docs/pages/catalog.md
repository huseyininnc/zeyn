# Cards, rails and search

Public pages such as a shop show rows as cards rather than a table. `CardGrid`, `Rail` and `SearchBox` load their rows through the same kind of action a DataGrid uses: it takes a `DataQuery` and returns a `Page`. Paging, sorting and the search text run on the server, and reading needs the action's access, as for a DataGrid.

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## Card grid

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name)
    .sort_option(t!("shop.sort.cheapest"), Product::price)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **Cards.** `Card::image(image, title)` makes one card from a row, with `text`, `badge`, `alt` and `link` (a screen and its params; the whole card is one link and opens without a page reload). Images are lazy, with the width and height from `image_size` so the page does not jump. The image's alternative text is empty, because the title next to it names the card; set `alt` when the picture says more. Image addresses other than a path, `https:` or `data:image/` are left out.
- **More on a card.** `subtitle` adds a quieter second line under the title (an author, a category). `corner(badge)` puts a badge over the image's ending corner, next to `badge` at the starting one; `badges([...])` adds a row of badges under the text, such as tags. `progress(fraction, label)` draws a thin bar along the bottom of the image for how much is done: the fraction runs from 0 to 1, and screen readers hear the label and the percentage (`role="progressbar"` with `aria-valuenow`). `horizontal()` lays the card out in a row, a small image at the start and the text beside it; a card grid or rail with such cards gives them wider columns. All of them work in a `CardGrid` and a `Rail`.

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **Fast product grids.** `.placeholder(...)` on the grid fills every card image while it loads, and a card can set its own. On a card, `.low(src)` shows a small file first and `.fallback(src)` tries another file once, before the card shows its alt text with an icon. `.priority(n)` loads the images of the first `n` cards (at most 12) at once with `fetchpriority="high"`; every other card image stays lazy, and pages loaded later never come first. They work like the options of [`Image`](display.md#image), with one listener for every card.

```rust
CardGrid::of(catalog_products)
    .card(|p: &Product| {
        Card::image(p.photo.clone(), p.name.clone())
            .low(p.photo_small.clone())
            .fallback("/static/no-photo.webp")
    })
    .placeholder(Placeholder::color("--z-surface-2"))
    .priority(4)
```
- **Long grids.** Cards and list rows off screen skip layout and paint (`content-visibility: auto`), with their size reserved from the image size, so a long grid costs little until it is seen. A card grid with 200 cards or more keeps only the rows near the viewport in the page, as a virtual [DataGrid](datagrid.md) does: the other rows are space of the same height, every row has the height of the tallest of the first cards, and each card tells screen readers its position (`aria-posinset`, `aria-setsize`). Tab and Shift+Tab reach every card, a focused card stays while it scrolls out of view, and "Show more" adds to the same window. During a fast fling the cards that only fly by fetch no images; they load once the scroll rests. Find in page only sees the cards near the viewport; use the grid's search for the others.
- **First page on the server.** The first page (24 cards in a card grid, 12 in a rail, or the size given to `paging`) is part of the page's HTML, so search engines and visitors without JavaScript see the cards and their links, and the images carry their size so nothing moves while they load. Later pages come from the browser. A public page stays the same HTML for every visitor, so it can be cached. A card linked to a screen with a route links to its canonical address (`/items/{slug}`), never to a redirect.
- **Paging.** `Paging::infinite(n)` (the default, 24) shows a "Show more" button that also fires when it scrolls into view; focus moves to the first new card when the button is pressed. `Paging::pages(n)` shows previous and next buttons and the page number: "Page 2 of 5" when the action returns a total, "Page 2" when it does not, and then Next works as long as the action returns a cursor. The server sets the page size; the client only says where to continue.
- **Sorting.** Each `sort_option` is an order users can pick from a select; the first is the order the cards start in. The server accepts only these orders, and drops filters and search text sent by the client.
- **Scope.** `.scope(field, value)` keeps only the rows whose field has that value, on every request, like [`DataGrid::scope`](datagrid.md): `.scope(Article::author_id, author.id)` for the articles of one author. Card grids, rails and data bound lists drop every filter the client sends, so the client can neither change nor remove the scope. Text compares ignoring case.
- **Search.** `.search(text)` shows only rows that match the free text ([`DataQuery::q`](#free-text)), such as the text of a results screen.
- **Masked fields.** Cards are built by a closure that sees the raw row. For a model with `#[mask]` fields, only users who may reveal them get the cards; everyone else gets `Forbidden`. Use a model without masked fields for public cards.

### Hover previews

`.preview(screen, params)` on an `ImageCard` or a `ListItem` that has a link shows that screen in a panel next to the card: after about half a second of hover with a mouse, or at once when the card gets keyboard focus. The screen loads only then, from `GET /_zeyn/screen/...` with the same access checks as a window, and each preview is kept for the page's life (24 at most, cleared when `nav.js` opens another screen). Frames, objects, media players and scripts are removed before the panel shows, so a preview never loads an iframe. The panel lives in the top layer, so rails and grids that clip their content do not cut it; it opens on the side with more room, flips at the screen edges, scrolls inside itself and never scrolls the page. It follows WCAG 1.4.13: the pointer can move onto the panel, Escape closes it, and it stays until the pointer or focus leaves. On touch screens and coarse pointers there is no preview and a tap opens the link; the script (`preview.js`, 2.5 KB gzip) never loads there, and elsewhere it loads on the first hover or focus of such a card. With reduced motion the panel appears without its short grow animation.

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview

### Builders

`.card` and `.item` stay the default: they give links, images and screen reader text for free. When a row needs another shape, `.builder(|row| node)` builds each row as any node, like Flutter's `ListView.builder`, on `List::of`, `CardGrid` and `Rail`. `.separator(node)` puts a node between rows (hidden from screen readers, which count the rows), and `Repeat::of(items, |item| node)` does the same for a list the screen already holds in memory, up to 500 items.

```rust
List::of(search_people)
    .builder(|p: &Person| Row::new().compact().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    .separator(Paragraph::new("·"))

Repeat::of(order.steps.iter(), |s| Badge::new(s.name.clone())).separator(Paragraph::new("→"))
```

- **Keys.** Built rows and the rows of a virtual list carry the model key in `data-zeyn-key`, so a refresh of the list ([Topics](effects.md#topics)) keeps focus and open details on the same record, wherever it moved.
- **Built rows are for showing.** Their nodes are not addressed by node paths, so a grid, a form or a lookup goes elsewhere. The nodes of `Repeat` are children (`p.0`, `p.1`, ...) and keep working.
- **Virtual scrolling.** A list with `.paging(Paging::virtual_scroll())` keeps only the rows in view in the page, inside a scroll area as tall as every row, and fetches blocks by offset as the user scrolls. Rows have one fixed height and no height per row: with a builder, `.row_height(px)` is required and `App::check_page` reports a missing one (`separator` is not used there). Each row carries `aria-posinset` and `aria-setsize`, so screen readers hear its place in the whole list; the scroll area is a focusable region named by the title.

## Rail

A rail is a card grid laid out in one row that scrolls sideways, with scroll snapping and previous and next buttons that scroll by a view's width (smoothly unless the user asked for reduced motion). It loads the next batch, 12 at a time, as the user scrolls near its end. The cards are links in the tab order and scroll into view as they get focus.

The scrollbar is hidden; touch, trackpad and Shift+wheel still scroll, and `.scrollbar()` keeps a thin bar. A mouse can press and drag the rail sideways. After a move of more than 5 pixels the click on release is cancelled, so the card under the pointer does not open, and the rail settles on the nearest card; a smaller move is a normal click. While dragging, text is not selected, images are not dragged out and no card preview opens. Touch keeps the browser's own scrolling. The previous and next buttons are always there, disabled at the ends, because a hidden scrollbar is no way to scroll for a keyboard. Without JavaScript the buttons are hidden and the rail scrolls natively.

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option_desc(t!("shop.sort.newest"), Product::id)
```

## Search box

A search field with suggestions, following the W3C APG combobox pattern with a list box popup. As the user types, it waits `instant` milliseconds and asks the server for suggestions: the action runs with the text as `DataQuery::q`, and each row's suggestion text comes back (its first unmasked text field unless `suggestion` says otherwise). Enter, or picking a suggestion, opens the results screen with the text, without a page reload.

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

Keyboard: Down and Up move through the suggestions (Down opens them), Enter searches with the highlighted suggestion or the typed text, Escape closes the suggestions and then clears the field. A live region tells screen readers how many suggestions there are.

The results screen takes `SearchParams` (`q`, empty when the screen is opened from a menu). The server cuts the text to `MAX_SEARCH_QUERY` (100) characters, decides how many suggestions to send (8 unless `limit` says otherwise, at most 20), checks the screen's access, the feature's guards and the action's permission, and counts every suggestion call against the write rate limit (`App::write_limit`).

`.drives(topic)` searches in place instead of opening a results screen: Enter, picking a suggestion or clearing the field puts the text into the screen's `q` param and refreshes the nodes that listen to the topic (see [Topics](effects.md#topics)). The screen's params need a `q: String` field, as `SearchParams` has.

### Rich suggestions

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))` shows each suggestion as a row with a small image (40 by 56 pixels, loaded lazily, following the `img-src` rules, with an empty alternative text because the title names the row; no placeholder without one), the title and the subtitle. The server answers `[{text, subtitle, image, href}]` with the record's canonical address. Picking a row with a click or Enter opens its link through `nav.js`, and since each row is a real link, middle click and "open in new tab" work. Enter with no highlighted row still opens the results screen. The combobox pattern stays the same: each row is an option named by its title and subtitle, rows are at least 44 pixels tall. Without `.item` the suggestions are plain text as above.

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## Free text

`DataQuery::q` is free text: rows whose text fields contain it. Both the text and the fields are compared in the form `zeyn::text::fold` gives: lower case, the Turkish dotted and dotless `i` (`İ`, `ı`) as `i`, accented Latin letters without their accents, and `ß`, `æ`, `œ`, `þ` written out. So "istanbul" finds "İSTANBUL", "nino" finds "Niño" and "strasse" finds "Straße". Fields with `#[mask]` are never searched, blank text matches every row and text longer than 100 characters is ignored. `DataQuery::apply` and the SQL data layers (`TRANSLATE`, `LOWER` and `REPLACE` over the column, `LIKE` with a bound parameter) read it the same way.

```rust
use zeyn::text::fold;

assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
assert_eq!(fold("Año Pequeño"), "ano pequeno");
assert_eq!(fold("Straße MÜLLER"), "strasse muller");
```

Use `fold` also for a search key column that the app writes itself, such as the trigram column of a [hand written query](database.md), so the key matches what users type. Punctuation and other scripts stay as they are; `Slug::from_title` is the form for addresses.
