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
