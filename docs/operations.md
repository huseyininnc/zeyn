# Operations

Running a Zeyn app in production: time zone data, signed licenses, shared state for several
servers, and logs and traces.

## Time zones

`TimeZone::named` accepts every IANA zone (`Europe/Istanbul`, `America/New_York`,
`Australia/Sydney`, in any case) and fixed offsets (`UTC+03:00`, `+05:30`).

- Zone data is the IANA time zone database built into the binary through `jiff` (about 200 KB).
  Answers are the same on every platform, Windows included, and no system zoneinfo is read.
- History is modeled: Turkey's daylight saving until 2016 and its permanent `+03` since,
  the United States rules before 2007, and every other recorded change.
- A wall clock time that does not exist (clocks moved forward) is refused with
  `LocalTimeError::Skipped`. One that happens twice (clocks moved back) means the earlier moment.
- A zone is read from the bundle the first time its name is used and kept for the life of the
  process, a few kilobytes each. Unknown names are never kept, so clients cannot grow the cache.
- `TimeZone::names()` is a short list of common zones for the shell's zone picker. A user's zone
  that is not on the list is added to the picker for that user.
- New zone rules arrive with a newer `jiff` release (`cargo update -p jiff-tzdb`). Rebuild and
  deploy when a country changes its rules.

## Signed licenses

A license file is signed with an ed25519 key that only the vendor has. The app embeds the public
key, so a customer can read a license but cannot change it or make a new one. Turn it on with the
`signed-licenses` Cargo feature.

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json` holds the license fields:

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- The file is JSON with the license as text in `payload` and the signature in `signature`. What
  is signed is `zeyn-license-v1\n` followed by the payload text, so the signed bytes are exactly
  the stored bytes.
- The private key stays on the signing machine. `keygen` never overwrites a key file, and `*.key`
  is in `.gitignore` of new projects. `sign` reads the key from `--key`, from the
  `ZEYN_LICENSE_KEY` variable or from `zeyn-license.key`, and refuses a license without `holder`
  or with a `valid_until` that is not `YYYY-MM-DD`.
- A changed file, a file signed with another key or a file that is not a license is refused:
  `License::from_signed` returns `LicenseError` (`BadSignature`, `InvalidPublicKey`,
  `Malformed`), the server logs why, and every `Licensed` guard denies with "Your license does not
  cover this." A file that cannot be read keeps the last license, like any failing license source.
- Expiry is checked by the guard on every request against the user's today, so an expired
  license locks the app with "Your license has expired." without a restart. A renewed file is
  read at the next refresh.
- ed25519 cannot tell a changed file from one signed with another key; both are a signature that
  does not match, and the message says so.
- `max_users` is carried and signed but not enforced by the framework; the app decides what it
  counts as a user.
