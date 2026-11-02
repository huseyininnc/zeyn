# Forms

`Form::of(value)` edits a model or an entity. Fields come from field references, labels from the locale files and validation from the `#[rule]` attributes, so the browser and the server check the same rules.

## A form

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

Leave the name empty or type a wrong email and press *Send*: the browser shows the error at once, and the server would refuse the same input with the same message.

## Layout

| Call | Result |
|---|---|
| `.field(M::f)` | One field on its own line |
| `.row(\|r\| r.field(a).field(b))` | Fields side by side; they stack on narrow screens |
| `.section(title)` | A titled group of the fields after it |
| `.step(title)` | A wizard step; the form shows one step at a time |
| `.range(M::from, M::to)` | Two dates as one range picker |
| `Form::auto(value)` | Every field, in declaration order |

## Saving

`.submit(label, action)` names the action that saves. The action must take `Valid<M>` for the form's model; anything else does not compile. The runtime sends the values, the server runs every rule, then the action.

- Field errors from the rules or from `ValidationErrors` in the action appear next to the fields.
- `Error::Conflict` (an old `#[version]`) shows a message that someone else changed the record.
- `.on_success(|result, ui| ...)` runs effects after a successful save (see [Effects and windows](effects.md)).

## Controls

Each field type has a default control: text inputs, number inputs, money with the currency, date and time pickers, checkboxes for `bool` and selects for choices. `.control(M::f, Control::...)` picks another one, and `.lookup(M::f, action)` searches records on the server. See [Controls and pickers](controls.md).

## Read only fields

`#[readonly]` fields are shown in forms but cannot be edited, such as a stock level that only movements change.
