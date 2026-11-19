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
| `.settings()` | Settings layout: label and help beside each control, and a save bar that sticks to the bottom once something changes |
| `.step(title)` | A wizard step; the form shows one step at a time |
| `.range(M::from, M::to)` | Two dates as one range picker |
| `Form::auto(value)` | Every field, in declaration order |

Forms are at most 720 px wide, and a card or panel that holds only a form is as wide as the form, so no half of the card stays empty. Labels sit 6 px above 36 px inputs; fields are 20 px apart. A focused input gets a border in the focus color and a soft 3 px ring; an invalid one a danger border. Units, currency and `%` are drawn muted inside the input, and number fields put their minus and plus buttons (28 px) inside the end of the field; arrow keys still step.

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

A dropped folder adds the files inside it and its subfolders, read in pages of at most 100 entries until the folder is done; files of a type the field does not take are skipped and counted in the live region. Files pasted while the field has focus upload too. A file whose extension or type is not in `accept` is refused in the browser before any byte is sent, and the server checks the content again. At most three files of a field upload at the same time; the others wait in line with a busy progress bar.

| Key | What it does |
|---|---|
| Tab | Moves to the file input, then to each file's download link and its Cancel, Retry or Remove button |
| Enter or Space on the file input | Opens the system file picker (the keyboard path for drag and drop) |
| Ctrl+V or Cmd+V on the file input | Uploads the files on the clipboard, such as a screenshot |
| Enter or Space on Cancel, Retry, Remove | Runs it; after Cancel focus moves to Retry, after Remove to the next file or back to the input |

What the server does for `POST /_zeyn/files/{store}`:

1. checks the CSRF header, the store's permission and the feature's guards, and runs the interceptors (`CallKind::Upload`, audited with the new file id); uploads count against the write limit;
2. refuses a declared `Content-Length` over the limit (`413`), then reads the first 4 KiB and finds the type from the content; the extension must belong to the same type and the type must be in `accept`, otherwise `415`. HTML, SVG, XML and scripts are never accepted. The browser's content type is ignored;
3. streams the rest to the store in chunks of at most 64 KiB through a channel of four, counting bytes; going over the limit, a broken connection or 30 seconds without data stops the upload and the store removes what it wrote;
4. runs the feature's `FileScanner`; a refused file, or a scanner that fails, is deleted and the answer is `422`;
5. answers `201` with `{id, name, size, type}`. The id is 128 random bits from the server; names are cleaned of paths and control characters.

With the `image` feature, `FileRules::clean_images()` adds a step after the store has the file: PNG, JPEG and WebP uploads are decoded and encoded again in the same format, so EXIF (with GPS), XMP, ICC profiles, text chunks and bytes hidden after the image data never stay on the server. The EXIF orientation is applied to the pixels first, JPEG is written at quality 90 and PNG and WebP losslessly. Decoding stops at `MAX_IMAGE_SIDE` (12,000) pixels on a side and `MAX_IMAGE_MEMORY` (256 MiB); an image that does not decode is deleted and the answer is `422`. Both limits are checked against the size in the image header before any pixel is decoded, so a small file that claims a huge image is refused without the memory, and an animated image keeps only its first frame. The cleaned file replaces the original under a new id, and the scanner then checks the cleaned file. Without the feature the method does not exist, so a store cannot think it is cleaned when it is not.

Downloads (`GET /_zeyn/files/{store}/{id}`) check the id's shape, the permission, the guards and the tenant, and go through the media serving (see `docs/media.md`): ranges, entity tags and `nosniff`. Only raster images, audio and video are inline; everything else is sent as `application/octet-stream` with `Content-Disposition: attachment`, a quoted ASCII name plus the UTF-8 name, under `Content-Security-Policy: sandbox`.

`DiskStore` keeps `{id}.bin` and `{id}.json` in one folder and writes uploads to `{id}.part` first. `MemoryStore` holds files in memory with a byte cap, for tests and demos. The form value only names files: a save action that must be sure a file exists calls `store.meta(id)`, as the showcase does.

`FileRules::quota(bytes, window)` caps what one user uploads to a store per window, counted with the app's rate store; the upload that crosses it still lands and the next one answers `429`. `DiskStore` removes `.part` files older than an hour (uploads stopped by a crash) at most once an hour. `ZipGuard` is a scanner for ZIP based uploads (ZIP, Word, Excel, PowerPoint): it refuses archives whose central directory declares more than 10,000 entries, more than 1 GiB in all or more than 200 times the file's size, and ZIP64 archives; code that unpacks an accepted archive must still stop at those limits.

Budget: a 256 MiB upload grows the server heap by about 0.4 MiB (`tests/files_memory.rs`); the control renders 10 DOM nodes plus 5 per file; `files.js` adds 17 KB raw and 6 KB gzip, `files.css` 3 KB raw and 1 KB gzip.

## File drop

`FileDrop::new(label, rules)` is the same upload control on its own, outside a form, for screens where files are the whole task, such as a document inbox. It takes folders, pasted files and the accept list like the form field, and uploads into the store of `rules`, which `Feature::files` registers with its permission. `.single()` takes one file at a time. `.on_upload(action)` runs an action with each stored `File`; the server first checks that the id has the shape it generates, cleans the name again and keeps only a content type of the rules, so the action gets nothing the browser made up. When the action refuses, the item shows the reason. The file picker inside the zone is the keyboard and single pointer path.

```rust
#[action(permission = "support.ticket.files")]
pub async fn attach(file: File, tickets: Tickets) -> Result<()> {
    tickets.attach(file).await
}

FileDrop::new(t!("ticket.attachments"), ATTACHMENTS).on_upload(attach)
```
