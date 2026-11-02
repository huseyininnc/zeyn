# Controls and pickers

Form fields pick a control from their type. `.control(M::f, Control::...)` changes it and `.lookup(M::f, action)` turns a field into a search over the server. Every control is a native form element underneath, so values post the same way with or without the runtime, and every control follows its ARIA pattern for keyboard and screen reader users.

```rust
Form::of(Contact { quantity: 2, size: Size::Medium, ..Contact::default() })
    .lookup(Contact::product, find_samples)
    .row(|r| r.field(Contact::delivery).field(Contact::size))
    .control(Contact::delivery, Control::Radio)
    .control(Contact::size, Control::Segmented)
    .field(Contact::newsletter)
    .control(Contact::newsletter, Control::Switch)
    .field(Contact::note)
    .submit(t!("docs.live.send"), send_contact)
```

@live controls

## Default controls

| Field type | Control |
|---|---|
| `String` | Text input; a growing text area with a character counter when `max_len` is over 255 |
| `i32`, `i64` | Number input with the `min` and `max` rules |
| `Money` | Amount in the user's number format, with the currency |
| `bool` | Checkbox |
| `#[choice]` enum | Select: a button that opens a searchable list |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | Date and time pickers |
| `Vec<M>` of a model | Line editor (see [Line editor](lines.md)) |

## Other controls

| Control | For |
|---|---|
| `Control::Radio` | A choice with a few options, all visible |
| `Control::Segmented` | A choice with two to five short options |
| `Control::Listbox` | A long choice list with type to search |
| `Control::Switch` | A `bool` that takes effect as a setting |

## Lookup

A lookup searches records on the server as the user types. The action takes a `LookupQuery` and returns `Page<LookupItem>`; the field stores the id.

```rust
#[action(permission = "stock.product.read")]
pub async fn find_samples(q: LookupQuery, store: SampleStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|s| LookupItem::new(s.id.0, s.name).hint(s.code)).collect();
    Ok(q.apply(items))
}
```

`.lookup_many(M::f, action)` does the same for a `Vec<i64>` field and shows the chosen ones as removable tags.

## Date and time pickers

The pickers take typed input in the user's format and open a calendar or a time list. `DateOptions` sets limits: `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`. `.range(from, to)` joins two date fields into one range picker with presets such as *This month*. The live example is on [Time and languages](time-and-languages.md#pickers).
