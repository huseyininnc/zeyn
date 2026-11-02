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
