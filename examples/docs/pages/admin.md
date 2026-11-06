# Admin views

Components for the back office: what changed in a record, read only code and data, barcodes, notifications, boards and calendars. Each one is rendered on the server and loads its stylesheet and script only on pages that use it.

## DiffView

`DiffView` shows the fields that changed between two values of a record: the old value struck out, the new one marked as added. Build it from an audit record with `DiffView::changes(entry.changes)`, or from two values of a model with `DiffView::of(&before, &after)`, which compares them like the audit trail does (row versions are left out).

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` gives the fields their labels in the user's language and choices their option labels. The side by side layout is a table with a column for the old values and one for the new values; `DiffLayout::Inline` puts the new value after the old one, for narrow places such as a window. At most `MAX_DIFF_FIELDS` (200) fields are shown, with a note for the rest.

Masked fields stay masked. The values are the masked text the audit trail holds, and a field the model marks `#[mask]` is masked again before it is shown, so even a change list built by hand never puts a raw value into the page. The component needs no script; `diff.css` (1.9 KB) loads only with it.

## CodeView and JsonView

`CodeView` shows source text read only: highlighted on the server, with line numbers, folding and a copy button. `JsonView` shows a JSON value indented the same way.

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

The highlighter (`zeyn::ui::highlight`) is written by hand for Rust, JSON, TOML, SQL, JavaScript and shell; `CodeLang::Plain` and any other name show escaped text. It wraps keywords, strings, numbers, comments, types and keys in `z-hl-*` spans whose colors are theme tokens, so both themes work and the browser does no work. Blocks between brackets (`{}` and `[]`) that span lines fold with a native `<details>` element, which needs no script: the first line is the button (Enter or Space), and a folded block ends with an ellipsis.

`JsonView::of(value, &ctx)` serializes any value the way the endpoints send it to the browser, so masked fields stay masked. At most `MAX_CODE_LINES` (10,000) lines and `MAX_CODE_BYTES` (1 MB) are shown; a longer text is cut at a line end with a notice that says how many lines are shown. The copy button (`share.js`, which also serves `Share`) copies the lines shown and confirms in a toast; it is hidden until its script runs. `code.css` (2.7 KB) loads only with the component.

| Key | What it does |
|---|---|
| Tab | Moves to the text (one stop, it scrolls with the arrow keys), then through the fold buttons and the copy button |
| Enter, Space | On a fold button: folds or unfolds the block |

## Barcode and QrCode

`Barcode::code128(value)` and `QrCode::new(value)` draw a symbol as SVG on the server. Both encoders are written by hand after the public standards (Code 128 and QR Code), so no crate and no script is added.

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode` encodes 1 to 80 printable ASCII characters (`MAX_BARCODE_CHARS`) and packs runs of digits two to a symbol (code set C), with the check symbol and 10 modules of quiet zone on each side. `QrCode` encodes the text as UTF-8 bytes at the smallest version (1 to 40) that holds it at the chosen level (`QrLevel::Low`, `Medium` by default, `Quartile`, `High`), with Reed Solomon error correction, the mask with the lowest penalty and 4 modules of quiet zone; at most `MAX_QR_BYTES` (2,953) bytes. A value that cannot be encoded shows a short notice instead of a broken symbol.

Each symbol is a figure that screen readers hear as one image named by the text under it: the value, or `.label(text)` when it is set. The bars stay black on a white plate in both themes, because scanners need dark bars on a light ground. `barcode.css` (1 KB) loads only with the components; `.height(px)` and `.size(px)` set the drawn size, and the symbol shrinks with a narrow container.

## Inbox and NotificationCenter

`Inbox` lists the user's notifications, newest first, grouped by day in the user's time zone (today, yesterday, then dates), with unread ones marked. Rows come from the same kind of action a DataGrid uses (`DataQuery` in, `Page` out); the inbox sorts by the time field and asks for at most `.limit(n)` rows (50, at most `MAX_INBOX`, 200). `NotificationCenter` is the bell of the top bar: a link to the inbox screen with the unread count.

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

With `.mark_read(action)` and `.archive(action)` every row gets a check box and the inbox a toolbar with "Select all". The buttons send the chosen keys as an `InboxSelection` through `POST /_zeyn/data`, which checks CSRF, counts against the write limit and checks the screen's access and the action's permission; at most 200 keys of 200 characters each are taken. The keys come from the browser, so the action changes only notifications of `ctx.identity()`. Buttons the user may not run are left out, check boxes too.

After a bulk action the inbox names its topic (`"notifications"`, `.topic(..)` to change it): the inbox and every node that `.listens` to it render again in place, and the bell loads its count again. The bell counts with an action that takes `UnreadCount { limit }` and stops at the limit (100), one bounded query; above 99 it shows `99+`, and the link's name says the count for screen readers. It loads the count with one request after the page shows and again when its topic is named by an inbox, by `ui.refresh("notifications")` or, with `.live()` and the `realtime` feature, by `hub.notify(&ctx, "notifications")` from any action (declare the topic with `Realtime::topic`). `inbox.js` (6.1 KB, 2.3 KB gzip) and `inbox.css` (3.1 KB) load only with the components.
