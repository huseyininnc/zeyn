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

`Badge::new(text).tone(Tone::...)`. Tones: `Neutral`, `Info`, `Success`, `Warning`, `Danger` and `Brand`. Grids show values as badges with `.badge(|v| tone)`.

## Progress

`Progress::new(label, value, max)` with an optional `.tone(...)`. It is a real `progressbar` with its label and value.

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

`EmptyState::new(title).text(text).opens(label, screen, params)` fills an empty area with a next step. `Skeleton::lines(n)` is a placeholder while something loads.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: links are typed screen references, so a renamed screen does not leave a broken link.

## ActionButton

`ActionButton::new(label, action, input)` runs an action. `.confirm(text)` asks first, `.tone(Tone::Danger)` marks a destructive one, and `.on_success(|result, ui| ...)` runs effects. The input is fixed on the server and never comes from the client; the action's permission is checked on the server like any form.

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

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
