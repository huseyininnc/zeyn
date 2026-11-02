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
