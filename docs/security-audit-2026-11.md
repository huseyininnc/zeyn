# Security audit, November 2026

A static review of the framework's HTTP surface before a release, followed by a dynamic pass that
reproduced each finding against a local build of the showcase or against the router in an
integration test. Every reproduced finding has a fix and a regression test that fails without it.

## Summary

| Id | Severity | Area | Reproduced | Fix commit | Regression test |
|---|---|---|---|---|---|
| S-01 | Medium | Pages of signed in users carried no `Cache-Control` | yes | `6c48fb39` | `public_pages::signed_in_pages_without_cache_and_screen_fragments_are_never_stored` |
| S-02 | Medium | Collaborative documents had no per document authorization | yes | `c5698498` | `realtime::the_document_store_decides_who_opens_and_changes_each_document` |
| S-03 | Low | `GET /_zeyn/devtools` listed every user's requests (debug builds) | yes | `c28138a8` | `devtools::requests_are_recorded_only_while_the_devtools_are_on` |
| S-04 | Low | Some costly `POST` endpoints were outside the write limit | partly | `7652ecd3` | `enterprise::reasons_socket_tickets_and_enrollment_count_against_the_write_limit` |
| S-05 | Low | Parallel uploads passed the upload quota together | yes | `05398a6a` | `files::uploads_at_the_same_time_cannot_pass_the_quota_together` |
| S-06 | Low | The access reason cookie was not bound to the user | yes | `94a9d7a6` | `enterprise::a_reason_given_by_one_user_does_not_open_the_screen_for_another` |
| S-07 | Low | Import previews ran outside the interceptors and the time limit | yes | `b4073e7a` | `enterprise::import_previews_run_through_the_interceptors` |
| S-08 | Info | A passkey challenge could be used more than once | yes | `bbedf185` | `accounts_passkeys::a_passkey_answer_with_its_ceremony_cookie_signs_in_only_once` |

## Findings

### S-01 Pages of signed in users carried no Cache-Control

`GET /dashboard` with a session answered `200 OK` without `Cache-Control`; `GET
/_zeyn/screen/{screen}` likewise. The browser could keep the page in its disk cache and show it
from history after sign out. Now every page and screen fragment of a request that is not an
anonymous visit to a public screen gets `Cache-Control: private, no-store` (error and reason pages
included). Public screens opened without a session keep `cache = N`.

### S-02 Collaborative documents had no per document authorization

`GET /_zeyn/live/doc/{kind}/{doc}` checked only the kind's `read` permission, so a user of the
tenant joined any document of the kind by its id, also a room another user held open (the test
saw `101 Switching Protocols` and the document state). `DocStore` gains `allow(ctx, key, write)`,
called on every join: `false` for reading answers `403`, `false` for writing makes the user a
reader. The default allows what the permissions allow, so existing stores keep working; stores
whose documents belong to people or records should implement it (`docs/realtime.md`).

### S-03 DevTools showed every user's requests

`GET /_zeyn/devtools` answered `200` with the request log of all users to any caller of a debug
build. It now answers `404` unless the caller is signed in with the `zeyn.devtools` permission,
and lists only that user's requests. `DevAuth` grants every permission, so the panel and the
`zeyn_devtools` MCP tool keep working in development.

### S-04 Costly POST endpoints outside the write limit

With the showcase's write limit used up (`POST /_zeyn/form` answering `429`), `POST
/_zeyn/reason` and `POST /_zeyn/socket/open` still ran. They now count, as does `POST
/_zeyn/auth/totp/enroll`. Not changed, by design: `POST /_zeyn/grid` and `POST /_zeyn/search`
read data and stay under the general rate limit (existing tests state this), and `POST
/_zeyn/socket/send` already has the per connection frame limit.

### S-05 Parallel uploads passed the quota

Three uploads of 100 bytes sent at once to a store with a 150 byte quota were all stored: the
quota was checked before the body and charged after it. An upload now takes its declared size
(the store's limit without one) from the quota before the body is read and gives back what the
file did not use, or all of it when the upload fails. `RateStore` gains `refund`; the memory and
Redis stores implement it, and a store that keeps the default gives nothing back.

### S-06 Access reason cookie not bound to the user

A reason given by one user opened the screen for the next user of the same browser for 15
minutes, and the audit recorded the first user's words for the second. The cookie now holds a hash
of the user and tenant and its own expiry, and the server ignores it for anyone else. The reason
stays readable to its own user only, as before.

### S-07 Import previews outside the interceptors

An interceptor that refused every import refused the commit but not the preview (`200`). Previews
now run through the interceptors as `CallKind::ImportPreview` (not audited, since nothing is
saved) with the app's time limit, and parse the file on a blocking thread instead of an async
worker. Both steps pass the caller's `via_agent` on.

### S-08 Passkey challenge not consumed on the server

With a passkey whose signature counter stays 0, the same assertion and ceremony cookie signed in
twice. The server now marks each challenge as used in the app's rate store until the ceremony
would expire, and refuses a second use. Capturing both values already needs control of the TLS
session, so this is defense in depth.

## Not found

CSRF coverage of the other `POST` handlers, node path handling, masked fields in grid filters,
sorts, search and exports, reveal, file downloads, the server sent events fallback, socket resume,
`return_to` handling, the HTML sanitizer and Markdown links were reviewed without findings.
