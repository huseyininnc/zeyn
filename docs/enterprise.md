# Enterprise Features

Cross cutting features for business apps: data audit, personal data protection, safe deploys,
health checks, session safety, CSV import, background jobs and approval workflows. Everything is
checked on the server; the browser runtime only makes it pleasant to use. KVKK and GDPR are the
privacy laws kept in mind, but nothing here is specific to a country or an industry.

## Field level data audit

`AuditLog` writes one `AuditEntry` per form submit and per call to an action marked `audit`. For
saves the entry also names the record and every field that changed:

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- Forms do this on their own: the old values come from the record the screen loaded on the
  server, the new values from the validated `Valid<M>`. Nothing the client sends is used as an
  old value, and fields the form does not take from the client never show up as changed.
- API saves record the model and key from their `Valid<M>` input. Only the action knows what is in
  the database, so it reports the difference itself:

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  Doing the same in form save actions is fine: the action's report replaces the form's.
- Values are plain, language independent strings (`1234.56 TRY`, `2024-05-01`, choice values).
  Row versions are left out. Masked fields stay masked (see below).
- A failed save still names the record, with no changes.

## Field masking

Mark text fields that hold personal data with `#[mask]`. Here the national id shows as `•••• 8901`, the email as `p••••@example.com` and the notes as `••••`:

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- Masking happens on the server, in grid cells, forms, API output, agent tools and therefore in
  CSV exports, which are built from the cells. The raw value never reaches a browser or an API
  client without the permission.
- The unmask permission is `<model>.<field>.unmask` in snake case (`person.national_id.unmask`)
  unless the attribute names one.
- Users with the permission still see the masked text first, with a reveal button. Revealing calls
  `POST /_zeyn/reveal`, which checks the permission again and writes an audit record (kind
  `reveal`, field in `revealed`). Refused attempts are audited too.
- API clients with the permission get raw values; each such field is listed in `revealed` of the
  call's audit record.
- Forms show masked fields as read only text. The stored value is kept on save unless the user
  may reveal it and changed it after revealing.
- Grid filters and sorts on a masked field are ignored for users who may not reveal it, so a value
  cannot be guessed by searching. Do not add `.filter()` to masked columns.
- Grid reveal looks the row up through the grid's action with a filter on the key, so the action
  must honor filters on its key field.
- `#[mask]` only compiles on `String` and `Option<String>`.
