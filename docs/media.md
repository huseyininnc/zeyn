# Media

Zeyn serves media files with HTTP range requests and plays them in the `Video` and `Audio` components. The native `<video>` and `<audio>` elements do the decoding; Zeyn draws its own accessible controls on top.

## Serving files

Register a `MediaSource` on a feature with a permission:

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

The browser fetches `GET /_zeyn/media/{source}/{id}`. Before the source sees anything, the server:

1. refuses ids that are not 1 to 128 of `A-Z a-z 0-9 - _ .`, start with a dot or contain `..` (`404`);
2. checks the source's permission (`403`) and the feature's guards;
3. runs the interceptors (`CallKind::Media`, with the source name in `screen`).

`open(ctx, id)` looks the id up in the app's own catalog and returns a `Media`: `Media::file(path)` for a file on disk (type from the extension, entity tag from size and modification time), `Media::bytes(data, type)` for data in memory, or `Media::new(reader, len, type)` for any `Read + Seek`. The id is a name, never a path: build the path from your lookup, not from the id.

Responses:

| Request | Answer |
|---|---|
| no `Range`, another unit, or several ranges | `200` with the whole file |
| `bytes=a-b`, `bytes=a-`, `bytes=-n` | `206` with `Content-Range: bytes a-b/len` |
| a bytes range that is malformed or starts past the end | `416` with `Content-Range: bytes */len` |
| `If-Range` that does not match the entity tag | `200` with the whole file |
| `If-None-Match` that matches the entity tag | `304` |

Every answer has `Accept-Ranges: bytes`, `Cache-Control: private, no-cache` and `X-Content-Type-Options: nosniff`. Only audio, video, raster images, WebVTT and streaming playlists are served inline; any other type (HTML, SVG, scripts) is sent as `application/octet-stream` with `Content-Disposition: attachment`, so a file can never run as a page of the app. Files are read in 64 KiB chunks on the blocking pool with at most four chunks queued, so memory stays flat for any file size and a client that leaves stops the read. Rate limits apply as for other requests; media reads are not writes.

The content security policy needs no change: `default-src 'self'` already allows media from the app. An adaptive streaming library that uses Media Source Extensions needs `blob:` in `media-src`, which Zeyn does not add.

## Public files: images, logos, downloads

Files that anyone may see, signed in or not, go in a public source:

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}` skips the permission check (ids, guards and interceptors still run), sets no cookie and answers with `Cache-Control: public, max-age=86400` (at most `MAX_PUBLIC_MEDIA_CACHE`, one year), so browsers and a CDN in front of the app keep it. The entity tag still lets them revalidate with `304`. Use the path as the `src` of an `Image`. For many large images a separate image host behind a CDN is still the cheaper choice; allow it with `App::csp(|c| c.img_src([...]))`.

- `open` must return the same bytes for every visitor. Never vary a public file by user, role, tenant or language: a CDN or proxy keeps the first response and hands it to everyone. Files that differ per user belong in a source registered with `Feature::media`.
- With a long `max_age`, give ids that change with the content, such as a content hash (`poster-3f9a2c.jpg`). A changed file then gets a new address, instead of old copies living on in caches until they expire.
- Reads of public sources (`GET` and `HEAD`) do not count against `App::rate_limit`: one page can load dozens of posters, and many mobile visitors share one address behind carrier NAT. Private sources still count, and IP allow and deny rules still apply to both. Limiting bandwidth abuse is the job of a CDN or reverse proxy in front of the app.

## Players

```rust
Video::new("films", t!("films.harbor"))
    .subtitle(t!("films.harbor_sub"))
    .file("harbor.webm")
    .file("harbor.mp4")
    .hls("harbor.m3u8")
    .poster("harbor.jpg")
    .captions("harbor.en.vtt", "en", "English")
    .captions("harbor.tr.vtt", "tr", "Türkçe")

Audio::new("music", t!("songs.dawn")).subtitle(t!("songs.artist")).file("dawn.mp3")
```

A player fetches nothing until it comes into view: it has `preload="none"`, and its sources and poster are set when it first scrolls into view (one observer serves every player of the page). `.poster_low(id)` shows a small poster first, `.placeholder(...)` colors the stage before that (the `Placeholder` of `Image`), `.on_error(text or component)` replaces the default message when the media cannot play, and `.sources([(id, media_query)])` adds files for other screens, such as `("clip-480.mp4", "(max-width: 600px)")`. For many qualities use HLS or DASH (below).

For lists, `.preview_when_visible()` makes the player a short preview: it plays muted and looping while at least a quarter of it is visible and pauses when it leaves. It never starts by itself when the visitor prefers reduced motion or saves data (`prefers-reduced-data`, Save-Data), never with sound, and once the visitor uses its controls it no longer starts or stops by itself.

Controls: play and pause, a seek slider, the time, mute, a volume slider, a playback speed menu, a captions button and (with more than one captions file) a language menu, and for video picture in picture (where the browser supports it) and full screen. Sliders follow the WAI-ARIA slider pattern: arrows move 5 seconds (or 5 percent of volume), Page Up and Page Down move a tenth, Home and End go to the ends. Menus follow the menu button pattern. While focus is inside a player, `k` or Space plays and pauses, `j` and `l` jump 10 seconds, `m` mutes, `c` toggles captions and `f` toggles full screen; the keys do nothing elsewhere on the page.

Captions are WebVTT files served by the same source. The player draws the active cue as plain text (cue markup is dropped), over the picture for video and under the title for audio, so captions look the same in every browser and in full screen.

## Resume

The player saves where the user stopped: every 15 seconds while playing, on pause, when the page is hidden or closed, and `0` when the media ends. It resumes when the saved point is more than 5 seconds from either end.

- When the source implements `position` and `save_position`, positions go to the server: `GET /_zeyn/media/position?source=&id=` answers `{"at": seconds, "kept": true}`, and `POST /_zeyn/media/position` with `{source, id, at}` saves (`204`). Both check the id, the permission and the guards and need a signed in user; the POST also needs the CSRF header and counts as a write for rate limits. Positions over `MAX_POSITION_SECONDS` or below zero are `422`.
- Otherwise (the source's default methods, or an anonymous user) the read answers `{"at": null, "kept": false}` and the player keeps the position in the browser's local storage under `zeyn.media.{source}/{id}`. Storage errors (private windows, blocked storage) are ignored.

Server positions are the better choice for signed in users: they follow the user to every device and the app can read them for a "continue watching" row. Local storage needs no code but stays on one browser and is shared by everyone who uses it.

## HLS and DASH

`hls(id)` and `dash(id)` add an adaptive stream. Safari plays HLS natively, so it uses the playlist. Other browsers need a JavaScript library (such as hls.js or dash.js), which Zeyn does not ship. Before falling back to the progressive files, the player dispatches a cancelable `zeyn:media-adaptive` event on its root:

```js
document.addEventListener('zeyn:media-adaptive', (e) => {
  const { media, kind, url } = e.detail;
  if (kind !== 'hls' || !window.Hls?.isSupported()) return;
  e.preventDefault();
  const hls = new window.Hls();
  hls.loadSource(url);
  hls.attachMedia(media);
});
```

Load that script from a file of your app (inline scripts are blocked by the content security policy). When no listener calls `preventDefault()`, the `file` entries play in order. Playlist and segment files are served by the same source, so their relative URLs work as long as their ids are listed in it.

## Uploaded files

Files uploaded through `Form::file` are served by `GET /_zeyn/files/{store}/{id}` with the same responses as media files (ranges, entity tags, `nosniff`, inline only for raster images, audio and video), after the store's permission, the feature's guards and a tenant check. See the file uploads section of `examples/docs/pages/forms.md`.
