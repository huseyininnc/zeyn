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
