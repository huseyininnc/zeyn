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
