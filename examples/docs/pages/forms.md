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

## Dependent lookups

`.depends_on(field, source)` makes the options of the lookup `field` depend on another field, such as the cities of a country. Every search sends the value of `source` in `LookupQuery::parent` (`q.parent_id()` reads it as an id), and changing `source` clears `field`, so its options load again for the new value.

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

The browser cannot be trusted with the pair, so on submit the server asks the lookup action again for the chosen city with the submitted country, with the action's permission, and refuses a city it does not return with a field error. Filter by the parent for id queries too, as `q.apply` after filtering does. `App::check_page` reports a `depends_on` whose field is not a lookup of the form or whose source is not another field of it.

## Filter forms

A form without a submit button can filter the screen: `.drives(topic)` makes its fields screen params. When a field changes (a select, a date range, a text field on Enter), its value goes into the screen's param of the same name, the address is kept in step, and the nodes that `.listens(topic)` refresh in place and read the value from their params (see [Topics](effects.md#topics)). Numbers and booleans are sent as such and empty fields as `null`; the server reads the params as the screen's typed params, so a value that does not fit is refused.

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

A form with `submit` cannot drive; refresh after saving with `ui.refresh(topic)` in `on_success` instead.

## Read only fields

`#[readonly]` fields are shown in forms but cannot be edited, such as a stock level that only movements change.

## File uploads

`File`, `Option<File>` and `Vec<File>` fields hold files kept by a `FileStore`. Declare the rules once as a `const`, register the store on the feature with a permission, and use the same rules on the form:

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

A permission name lets the same users upload and download. To let readers open files they may not add to, split it: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`.

The control is a native file input inside a drop zone. Each file uploads on its own with a progress bar, can be canceled while it runs, retried when it failed and removed when it is done. Images show a preview made from an object URL of the chosen file. Progress and results are announced in a polite live region. The showcase has a working example at `/uploads`; this documentation site has no signed in user, so it shows no live upload.

| Key | What it does |
|---|---|
| Tab | Moves to the file input, then to each file's download link and its Cancel, Retry or Remove button |
| Enter or Space on the file input | Opens the system file picker (the keyboard path for drag and drop) |
| Enter or Space on Cancel, Retry, Remove | Runs it; after Cancel focus moves to Retry, after Remove to the next file or back to the input |

What the server does for `POST /_zeyn/files/{store}`:

1. checks the CSRF header, the store's permission and the feature's guards, and runs the interceptors (`CallKind::Upload`, audited with the new file id); uploads count against the write limit;
2. refuses a declared `Content-Length` over the limit (`413`), then reads the first 4 KiB and finds the type from the content; the extension must belong to the same type and the type must be in `accept`, otherwise `415`. HTML, SVG, XML and scripts are never accepted. The browser's content type is ignored;
3. streams the rest to the store in chunks of at most 64 KiB through a channel of four, counting bytes; going over the limit, a broken connection or 30 seconds without data stops the upload and the store removes what it wrote;
4. runs the feature's `FileScanner`; a refused file, or a scanner that fails, is deleted and the answer is `422`;
5. answers `201` with `{id, name, size, type}`. The id is 128 random bits from the server; names are cleaned of paths and control characters.

Downloads (`GET /_zeyn/files/{store}/{id}`) check the id's shape, the permission, the guards and the tenant, and go through the media serving (see `docs/media.md`): ranges, entity tags and `nosniff`. Only raster images, audio and video are inline; everything else is sent as `application/octet-stream` with `Content-Disposition: attachment`, a quoted ASCII name plus the UTF-8 name, under `Content-Security-Policy: sandbox`.

`DiskStore` keeps `{id}.bin` and `{id}.json` in one folder and writes uploads to `{id}.part` first. `MemoryStore` holds files in memory with a byte cap, for tests and demos. The form value only names files: a save action that must be sure a file exists calls `store.meta(id)`, as the showcase does.

Budget: a 256 MiB upload grows the server heap by about 0.4 MiB (`tests/files_memory.rs`); the control renders 10 DOM nodes plus 5 per file; `files.js` adds 12 KB raw and 4 KB gzip, `files.css` 3 KB raw and 1 KB gzip.
