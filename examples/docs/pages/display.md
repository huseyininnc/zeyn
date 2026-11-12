# Display

Components that show state: messages, badges, progress, key values, history and placeholders. They render on the server and need no requests of their own.

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

`Alert::info`, `success`, `warning` and `danger`, or `Alert::new(tone, text)`. `.title(text)` adds a heading and `.dismissible()` a close button. The tone is also spoken before the message, so it is never given by color alone.

## Badge

`Badge::new(text).tone(Tone::...)`. Tones: `Neutral`, `Info`, `Success`, `Warning`, `Danger` and `Brand`. A toned badge shows the tone's text on its subtle tint; `Neutral` is a muted label with one outline, for plain values such as a plan. `.dot()` draws an 8 px dot in the tone before plain text, the quieter form for status in lists; `.count()` draws a round pill for a number. Grids show values as badges with `.badge(|v| tone)` and as dots with `.status(|v| tone)`.

## Progress

`Progress::new(label, value, max)` with an optional `.tone(...)`. It is a real `progressbar` with its label and value.

## ProgressRing

`ProgressRing::new(label, value, max)` shows the same progress as a ring with the percent in its middle and the label under it, for a share of a whole in little room: a quota used, a profile filled in. `.tone(...)` colors the ring and `.size(px)` sets its diameter, 64 unless set and kept between 32 and 160. The ring is an inline SVG drawn by the server, so it needs no script and stays sharp at every density. It is a real `progressbar` named by its label; a value above the maximum counts as the maximum.

```rust
ProgressRing::new(t!("account.storage_used"), used, quota).tone(Tone::Warning).size(96)
```

@live rings

## Meter

`Meter::new(label, value, min, max)` shows a measurement within a known range, such as disk space used or a stock level. Unlike `Progress` it is not a task moving to its end: the value can rise and fall, and `.low(n)` and `.high(n)` mark where it becomes low or high. The bar takes its color from that state: with no direction set, the range between the thresholds is good and both ends are a warning; `.better(Better::Higher)` makes a high value good and a low one bad, and `Better::Lower` the other way round. The state is never given by color alone: "low" or "high" is written next to the value in the user's language and is part of what screen readers hear from the ARIA `meter`. Values are whole numbers, grouped as the user's language groups them, and `.unit(text)` adds a unit.

```rust
Meter::new(t!("server.disk_used"), used_gb, 0, total_gb)
    .high(total_gb * 8 / 10)
    .better(Better::Lower)
    .unit("GB")
```

## Indicator

`Indicator::on(node)` puts a count or a dot on the corner of another component: unread messages on an avatar, something new on a link. `.count(n)` shows the number, "99+" above 99 and nothing at zero; `.dot()` shows a dot; `.tone(...)` sets the color, `Danger` unless set. The mark itself is hidden from screen readers, because a bare "3" after a name says little: `.label(text)` gives the words they hear instead, such as "3 unread messages". Give one whenever the mark carries meaning the component under it does not already state; without a label a count is read as its number and a dot is not read.

```rust
Indicator::on(Avatar::new(user.name.clone()))
    .count(unread)
    .label(t!("inbox.unread_count").arg("n", unread.to_string()))
```

@live meters

## DescriptionList

Key value pairs, from a record's fields or by hand:

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

Values are formatted like grid cells: money, dates and choices in the user's language.

## Timeline

`Timeline::new().event(moment, title, detail, tone)` lists events. Moments show in the user's time zone and language.

## EmptyState and Skeleton

`EmptyState::new(title).text(text)` fills an empty area with a next step: `.link(label, screen, params)` goes to another page through a `LinkButton`, and `.opens(label, screen, params)` opens a screen of the app as a window. `Skeleton::lines(n)` is a placeholder while something loads.

## LinkButton

Going somewhere is a link; doing something is a button. `LinkButton::new(label, screen, params)` is a real `<a href>` to the screen's canonical path (such as `/products/42`) that looks like a button, so middle click, "open in new tab" and search engines treat it as the link it is. It opens through `nav.js` without a page reload, and Enter follows it. The look is secondary unless `.primary()` asks for the filled look of the main action. The screen must have a route, or the code does not compile. `EmptyState::link` and `Hero::link` use it.

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)` is a button that removes every client preference this browser keeps for the app, such as on a privacy page beside the list from `App::client_prefs_table()`. A toast confirms it. Cookies the server sets (theme, language, session) are not touched. Scripts can do the same with `zeyn.prefs.clear()`.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: links are typed screen references, so a renamed screen does not leave a broken link.

## ActionButton

`ActionButton::new(label, action, input)` runs an action. `.confirm(text)` asks first, `.tone(Tone::Danger)` marks a destructive one, and `.on_success(|result, ui| ...)` runs effects. The input is fixed on the server and never comes from the client; the action's permission is checked on the server like any form. `.tone(Tone::Brand)` draws it as the main action of its area, such as a card.

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

`Markdown::new(text)` shows formatted text, such as a comment, a product description or help. The text is a string or a locale key. It needs the `markdown` feature (`zeyn = { features = ["markdown"] }`), which adds no dependencies.

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- Headings, paragraphs, lists, tables, code blocks, quotes, rules, inline code, links, images, bold and italic.
- Raw HTML is never passed through: `<script>` shows up as text, and code blocks are escaped too.
- Links may only point to `http`, `https`, `mailto`, an anchor or a relative path. Others, such as `javascript:` or `data:`, keep only their label. User links get `rel="noopener noreferrer nofollow ugc"`.
- Images load only from the app itself (a relative path); other images show their alt text.
- `.trusted()` is for text the app wrote: headings get ids for links to them and links lose `nofollow ugc`. The escaping and the link rules stay the same.
- Text over 64 KB (`MAX_MARKDOWN`) is cut, and nesting is capped, so hostile input stays cheap to render.

## Embed

`Embed` shows a page from another site, such as a video player or a map, in a locked frame. A title is required (screen readers announce frames by it); without `.title(...)` the embed cannot be placed and the compiler says so.

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- The host must be in the app's `frame-src`: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` (see [Security](security.md#csrf-csp-and-headers)). When the address is not `https` or its host is not listed, no frame is rendered at all; a warning says the content cannot be shown.
- Every frame gets `sandbox="allow-scripts allow-same-origin allow-presentation"`, `referrerpolicy="no-referrer"` and `loading="lazy"`. Popups, top navigation, forms and downloads can never be allowed. `.allow(...)` adds browser features: `Fullscreen`, `PictureInPicture`, `Autoplay` and `EncryptedMedia`.
- `.click_to_load(poster)` shows a button first, with the poster image (a path on the app or a host of `img-src`; `""` for none) and the other site's name. The other site is not contacted until the button is pressed; then the frame takes its place and receives focus. The script is part of `media.js`, so it costs no extra request.
- `.notice(t!(...))` puts a text of the app on that button, such as a content warning. It is escaped, translated like every text and part of the button's accessible name, so screen readers read it before the user decides. Without `.click_to_load` it turns the button on with no poster.
- `.remember(EXTERNAL_VIDEO)` asks once in the key's lifetime instead of on every page. The key is a consent key declared with `ClientPref::consent` and registered with `App::client_prefs` (see [Client preferences](security.md#client-preferences)); embeds bound to the same key share the answer. Later pages, in-app navigations and other tabs load those frames by themselves, without moving focus (WCAG 2.2 3.2.1 On Focus, 2.4.3 Focus Order); focus moves to the frame only after a real press. The button comes back when the record has ended or fails its checks, when the app did not register the key, when storage is blocked and the page is new, and without JavaScript. A plain string or a choice key does not compile.
- A field that holds an address to embed takes `#[rule(embed)]`. Forms accept a URL or pasted embed code, keep only the `src`, require `https` and a listed host, drop tracking parameters (`utm_*`, `fbclid`, `gclid` and others) and store the canonical address, never the HTML. API inputs must send the canonical address.

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

`Image` needs its alt text and its size; without `.alt(...)` and `.size(width, height)` it cannot be placed and the compiler says so. The size keeps the space while the file loads, so the page does not move.

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` is a path on the app (`/static/logo.png`) or an `https` address on a host of the app's `img-src` (`App::csp(|c| c.img_src([...]))`). Any other image is not rendered and its alt text is shown instead, so the browser never logs a blocked request.
- Every image gets `decoding="async"`; `.lazy()` adds `loading="lazy"` for images below the fold. `srcset` takes density (`2x`) or width (`800w`) descriptors; entries on other hosts or with other descriptors are left out. Width descriptors add `sizes` from the image width.
- A purely decorative image takes an empty alt text: `.alt(Text::from(""))`.

### Long lists of photos

For a catalog with thousands of product photos, an image can show something while it loads and recover when it fails. It all uses native browser features; one listener of the media module serves every image on the page.

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

- `.placeholder(...)` fills the box while the file loads, without script: `Placeholder::color("#d9dde3")` or a theme token (`"--z-surface-2"`), `Placeholder::blur(data_uri)` for a tiny image the app made (a base64 `data:image/webp`, `avif`, `jpeg` or `png` URI of at most 600 bytes, never SVG), or `Placeholder::image(src)` for one file shared by every image. A placeholder that breaks a rule is left out.
- `.low(src)` shows a small, low resolution file at once under the full one. The full file loads at the same time (lazily when the image is lazy) and fades in when it is ready; with reduced motion it appears without the fade.
- `.fallback(src)` tries another file once when the image fails. After that `.on_error(text or component)` is shown; without it, the alt text with an icon on the placeholder. Nothing is tried again.
- `.priority()` marks the first image a visitor sees (the largest paint): `loading="eager"` and `fetchpriority="high"`, never lazy. Use it once per page.
- `.sizes(...)` says how wide the image is in the layout, so the browser picks the right `srcset` width. Without it a lazy image gets `sizes="auto"` and the browser measures the slot.
- `.fit(Fit::Cover)` fills the box and cuts what overflows; `Fit::Contain` shows the whole file.
- `low`, `fallback` and `Placeholder::image` follow the same address rules as `src`; others are left out. The box always keeps the image's size, so nothing moves while files load or fail.

## ImageCompare

Two pictures of the same size on top of one another with a slider that uncovers one over the other, such as before and after. `.before(src, alt)` and `.after(src, alt)` give the pictures, which follow the rules of `Image`, and `.size(width, height)` gives the box its shape. The slider is a range input named by the label, so the arrow keys move it; a small script, loaded only on pages that hold one, shows it, and without script the pictures show half and half.

```rust
ImageCompare::new(t!("repair.before_after"))
    .before(job.before.clone(), t!("repair.before"))
    .after(job.after.clone(), t!("repair.after"))
    .size(1200, 800)
```

## ExternalLink

`ExternalLink::new(url, text)` links to another site with `rel="noopener noreferrer"`, so the other site gets neither the page address nor a handle on the window. Screen readers hear "external site" after the text; an arrow marks it visually.

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()` adds `nofollow ugc` for addresses users typed. `.new_tab()` opens a new tab and says so to screen readers; links stay in the same tab unless set. Only `https` and `http` addresses become links; anything else, such as `javascript:`, is shown as plain text.

## Tooltip, popover and busy overlay

A tooltip is a short description of one control: `ActionButton::tip(text)`, `ToolButton::tip(text)` in a screen toolbar and `Form::tip(field, text)`, which adds an info button after the field's label. The text is the control's `aria-describedby`, so screen readers hear it with the control. It shows after a short hover and at once on focus, stays while the pointer is over it (WCAG 1.4.13), hides with Escape without moving focus, and shows on touch after a long press (a tap on the info button). A tooltip never carries information the user must have: put that in the label, the field's rules or the page.

`Popover::new(label).title(title).add(node)` is a button that opens a non-modal dialog next to it, for small details. Focus moves into it; Escape, the close button or a click outside close it and focus returns to the button. On narrow screens it opens as a bottom sheet. Its children are ordinary nodes, so a form inside it posts to its own node path with every server check.

`Busy::new().add(node)` covers its content while a request inside it runs: a form saving, an action button, a grid or tree loading. The container gets `aria-busy` at once; the spinner and the text "Working" (a status message) show only after 400 ms, or `.delay_ms(n)`, so fast answers do not flash, and stay at least 300 ms once shown. The overlay blocks the pointer, not the keyboard, and focus stays where it was.

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| Key | Where | Does |
|---|---|---|
| Tab | a control with a tooltip | Focus shows the tooltip; leaving hides it |
| Escape | anywhere while a tooltip shows | Hides the tooltip; focus stays |
| Enter, Space | popover button | Opens the popover; focus moves to its first control |
| Escape | inside the popover | Closes it; focus returns to the button |
| Tab | inside the popover | Moves through its controls; leaving it closes it |

The three need about 2.2 KB gzip of script and no request of their own; a tooltip is 1 node, a popover 6 plus its content, the busy overlay 3.

## Drop down and split buttons

`MenuButton::new(label)` opens a menu of commands; `MenuButton::split(label, action, input)` runs one action from its main part and opens the menu from its arrow. Commands are `.action(label, action, input)` (or `.action_then(...)` with effects after success), `.item(label, effect)` for effects such as opening a window, and `.separator()`. `.confirm(text)` asks before the last action command runs.

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

Action commands are nodes with their own path and post to the form endpoint like an `ActionButton`: the screen's access, the feature's guards, the action's permission, the interceptors, CSRF and the write rate limit are checked on every call, and the input stays on the server. Commands the user may not run are left out of the menu (a split button's main action is disabled instead), and the server still refuses them if called.

@live menus

| Key | Where | Does |
|---|---|---|
| Enter, Space, Arrow Down | the button | Opens the menu on the first command |
| Arrow Up | the button | Opens the menu on the last command |
| Arrow Down, Arrow Up | the menu | Next and previous command, wrapping |
| Home, End | the menu | First and last command |
| Letters | the menu | The next command that starts with them |
| Enter, Space | a command | Runs it and closes the menu |
| Escape | the menu | Closes it; focus returns to the button |
| Tab | the menu | Closes it and moves on |

The menu needs about 0.8 KB gzip of script and no request until a command runs; it is 4 nodes plus 2 per command.

## DocumentViewer

`DocumentViewer::of(page_action, page_count)` shows a paged document, such as a report preview, whose pages are drawn on the server. The page action takes a `DocPage` (`document` from `.document(id)`, never from the browser, and `page`) and returns one page as an SVG or HTML fragment. Every page has the size of `.page_size(..)` (`PageSize::A4`, `PageSize::LETTER`, `PageSize::pt(w, h)` or `PageSize::px(w, h)`), so the scroll area is as tall as the whole document from the start.

```rust
DocumentViewer::of(report_page, report.pages)
    .document(report.id.to_string())
    .page_size(PageSize::A4)
    .outline(vec![OutlineItem::new("Summary", 1).child(OutlineItem::new("Totals", 2))])
    .search(search_report)
    .download("PDF", format!("/reports/{}/pdf", report.id))
```

- Only the pages in view and one on each side are in the page. They are fetched through `POST /_zeyn/data` once scrolling pauses, so pages scrolled past quickly are never fetched, nearest first, at most 3 at a time, and a fetch whose page has scrolled away is aborted. At most 12 rendered pages are kept in the browser.
- Zoom scales the pages without fetching them again: fit width, fit page and 50 to 400 percent. The page box goes to a page, and the current page is announced ("Page 3 of 500") once scrolling stops.
- `.outline(items)` adds an outline panel, `.search(action)` a search box whose action gets a `DocSearch` and returns `PageHit`s (at most 50 are shown), and `.download(label, href)` a download link.
- The server checks the screen's access, the page action's permission, CSRF and the write limit on every page, refuses page numbers outside 1 to the page count and pages over 2 MB (`MAX_PAGE_BYTES`), and cleans every page with an allow list (`sanitize_markup`): scripts, event handlers, `foreignObject`, frames, external links and images, and `url()` in styles are removed, and ids get a prefix per page. `.trusted()` skips the cleaning for pages the app draws from data it controls; user text that reaches such a page unescaped runs as script for everyone who opens it.
- `document.js` (13 KB, 4.7 KB gzip) and `document.css` (3.6 KB) load only on pages with a viewer.

| Key | Where | Does |
|---|---|---|
| Page Down, Page Up | the pages | Next and previous page |
| Home, End | the pages | First and last page |
| `+`, `-` | the pages | Zoom in and out |
| Enter | the page box | Goes to the page typed |
