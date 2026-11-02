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
