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
| `Decimal` | Exact number in the user's format, with the field's `scale` (see [Decimal input](#decimal-and-percent)) |
| `bool` | Checkbox |
| `#[choice]` enum | Select: a button that opens a searchable list |
| `Vec` of a `#[choice]` enum | Multi select: chips and a list where several options are chosen |
| `Date`, `Time`, `DateTime`, `LocalDateTime` | Date and time pickers |
| `Vec<M>` of a model | Line editor (see [Line editor](lines.md)) |

## Other controls

| Control | For |
|---|---|
| `Control::Radio` | A choice with a few options, all visible |
| `Control::Segmented` | A choice with two to five short options |
| `Control::Listbox` | A long choice list with type to search |
| `Control::Switch` | A `bool` that takes effect as a setting |
| `Control::Percent` | A `Decimal` or whole number shown with a percent sign |
| `Control::Password`, `Control::NewPassword` | A text field typed as a password, with a show and hide button |
| `Control::PasswordMeter` | A new password with a strength meter |

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

`.lookup_columns(M::f, action, [t!("code"), t!("name"), t!("city")])` shows several columns in the list. The search still goes to the action on the server, with its permission checked on every keystroke's search; each item gives its values with `LookupItem::cells`, in the order of the headers. `LookupQuery::apply` searches the cells too. Screen readers hear each option as "Code: C-2, Name: Mert Kaya, City: Ankara". On narrow screens the columns stack inside each option.

```rust
#[action(permission = "sales.customer.read")]
pub async fn find_customers(q: LookupQuery, store: CustomerStore) -> Result<Page<LookupItem>> {
    let items = store.all()?.into_iter().map(|c| LookupItem::new(c.id.0, c.name.clone()).cells([c.code, c.name, c.city]));
    Ok(q.apply(items.collect()))
}
```

## Multi select

A `Vec` of a `#[choice]` enum is edited with a multi select: the chosen options show as chips with a remove button, and a combobox opens a list where options are turned on and off (APG listbox with `aria-multiselectable`, inside a combobox). Underneath is a native `<select multiple>`, so the field works without the runtime. The runtime posts the values joined with commas (`Email,Sms`); the server reads them one by one, refuses unknown values and drops repeats. In JSON (APIs, AI tools, the database) the field is an array of the option values.

```rust
#[model]
pub struct Contact {
    #[rule(required)]
    pub channels: Vec<Channel>,
}

Form::of(contact).field(Contact::channels)
```

@live multi

| Key | Does |
|---|---|
| Arrow Down, Arrow Up, Enter, Space | Opens the list on the closed combobox |
| Home, End | Opens the list on the first or last option |
| Arrow Down, Arrow Up | Moves through the open list |
| Shift with Arrow Down or Arrow Up | Moves and turns the next option on or off |
| Space, Enter | Turns the active option on or off; the list stays open |
| Letters | Jump to the next option that starts with them |
| Backspace, Delete | On the closed combobox: removes the last chosen option |
| Escape, Alt with Arrow Up | Closes the list; focus stays on the combobox |
| Tab | Closes the list and moves on |

Ctrl+A (select all in the APG pattern) is left to the browser, as the shortcut policy requires; the chips' remove buttons are in the tab order. The list, the chips and the lookup columns need about 1.7 KB gzip of script and no request; the field is 3 nodes plus one per chosen option.

## Decimal and percent

`Decimal` fields hold exact numbers: no floating point anywhere between the input, the server, the database and the export. The input formats the number in the user's language while typing (`1.234,5678` in Turkish, `1,234.5678` in English) and allows the field's `scale` of decimals; the server reads it in the same language and checks `scale`, `precision`, `min` and `max` again.

```rust
#[model]
pub struct Terms {
    #[rule(precision = 12, scale = 4, min = 0)]
    pub rate: Decimal,
    #[rule(scale = 1, min = 0, max = 100)]
    pub discount: Option<Decimal>,
}

Form::of(terms).field(Terms::rate).field(Terms::discount).control(Terms::discount, Control::Percent)
```

`Control::Percent` keeps the value in percent points: `12.5` is shown as `12.5 %` in English and `%12,5` in Turkish and stored as `12.5`. Screen readers hear the word "percent" through the input's description.

## Masked input

A text field with the `mask` rule shows its pattern while the user types and submits only the raw characters. The pattern is the rule: the server reads typed literals away (`(532) 123 45 67` becomes `5321234567`), upper cases `A` slots and refuses a value that does not fill every slot. API calls and AI tools send the raw value and get the same check.

```rust
#[model]
pub struct Contact {
    #[rule(required, mask = "(999) 999 99 99")]
    pub phone: String,
    #[rule(mask = "AA99 9999 9999 9999 9999 9999 99")]
    pub iban: Option<String>,
}
```

| Pattern character | Means |
|---|---|
| `9` | A digit |
| `a` | A letter |
| `A` | A letter, written upper case |
| `*` | A letter or a digit |
| a backslash | The next character is a literal, such as `\T` |
| anything else | A literal: shown, never stored |

Without the runtime the field is a plain text input with the raw value; users may type it with or without the literals.

## Password

`Control::Password` (sign in), `Control::NewPassword` (sign up, change) and `Control::PasswordMeter` (a new password with a strength meter) are password inputs that are never filled from the server and never kept in drafts. A show and hide button follows the APG toggle button pattern: its name stays "Show password" and `aria-pressed` tells whether the text shows. The form switches the password back to hidden when it submits.

The meter is a hint computed in the browser from the length, the kinds of characters and obvious patterns, shown as four bars and a text ("Strength: good") in a polite live region. What the server accepts is decided by the field's rules (`min_len`, a model `validate` function), never by the meter.

@live inputs

### Keyboard

| Key | Where | Does |
|---|---|---|
| Arrow Up, Arrow Down | decimal and percent inputs | Step by one whole unit inside `min` and `max` |
| Page Up, Page Down | decimal and percent inputs | Step by ten |
| Backspace | masked input | Deletes the character before the caret; over a literal it deletes the character before the literal |
| Tab | password input | Moves to the show and hide button |
| Enter, Space | show and hide button | Shows or hides the password; focus stays on the button |

### Budget and checks

| | |
|---|---|
| Added script | about 2 KB gzip (7 KB raw) in `components.js` for masks, the toggle and the meter; the decimal and percent inputs use the number control |
| DOM nodes | 3 for a masked input, 4 for a password with its toggle, 11 with the meter |
| Requests | none while typing |
| Server checks | `scale`, `precision`, `min`, `max` and `mask` run on every save, from the page, the API and AI tools alike |

## Date and time pickers

The pickers take typed input in the user's format and open a calendar or a time list. `DateOptions` sets limits: `.no_weekends()`, `.week_numbers()`, `.step_minutes(15)`. `.range(from, to)` joins two date fields into one range picker with presets such as *This month*. The live example is on [Time and languages](time-and-languages.md#pickers).
