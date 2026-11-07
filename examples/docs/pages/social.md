# Social

Components for social pages. Each one is rendered on the server and loads its stylesheet and script only on pages that use it.

## StoryRing

`StoryRing` is an avatar inside a ring that tells whether a person's stories are new to the viewer. `StoryState::Unseen` draws a thick ring in the brand colors, `StoryState::Seen` a thin neutral one. The state comes from the server, from what the app stored about the viewer; the browser never changes it. `StoryRow` puts rings in a row that scrolls sideways, like a rail.

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

The ring is a link to the screen that shows the stories when `.link(screen, params)` is set. Its accessible name is the person's name and the state, such as "Ada Lovelace, unseen stories"; the visible name under the ring starts the same way, so voice control users can say it. The two states differ in the width of the ring as well as its color, and in forced colors mode the seen ring is dashed. The picture follows the `Avatar` rules (a path of the app or an allowed `https` address); without one the initials are shown.

The ring is drawn with CSS only: no script and no image. `stories.css` (2.2 KB) loads only on pages that hold a ring. A row holds at most `MAX_STORY_RINGS` (100) rings. Touch, trackpad and Shift+wheel scroll it, the rings snap into place, and a ring that gets keyboard focus scrolls into view. When no ring of a row has a link, the row itself takes focus, so the keyboard can still scroll it.

| Key | What it does |
|---|---|
| Tab | Moves to the next ring and scrolls it into view |
| Enter | Opens the stories of the focused ring |
| Left, Right | When the row itself has focus: scrolls it |


## Lightbox

Any `Image` opens full screen with `.lightbox()`: a modal dialog with the image as large as the screen allows, its caption and zoom. `.full(src)` gives the larger file the lightbox shows while the page keeps the small one, `.caption(text)` a caption under it (both turn the lightbox on), and `.video(source, file)` makes the image the poster of a video from a media source, which the lightbox plays. The images of one post, or of one container such as a `Row`, `Stack` or `AutoGrid`, form a set: the arrow keys and swipes move between them.

```rust
AutoGrid::new().min(140)
    .add(Image::new(p.thumb.clone()).alt(p.title.clone()).size(300, 300)
        .full(p.photo.clone())
        .caption(p.title.clone()))
    .add(Image::new(clip.poster.clone()).alt(clip.title.clone()).size(320, 180)
        .video("films", clip.file.clone()))
```

Without script the image is a link to its file, so it still opens. With script, `lightbox.js` (13 KB, 4.4 KB gzip) and `lightbox.css` (4 KB) load only on pages that hold a lightbox image, and the dialog is built the first time one opens. Only the open item is in the dialog; the larger files of its neighbours are fetched ahead as images, videos never are. The thumbnail shows at once, blurred, and the larger file covers it when it has loaded. Opening takes about 16 ms on a desktop.

The larger file follows the image's address rules (a path of the app or an allowed `https` address); when it breaks them, the image's own file is used. A video file must be a valid media id of the source, which checks its permission and serves ranges on every request. The dialog follows the W3C APG dialog pattern: focus moves to the close button, Tab and Shift+Tab stay inside, and closing returns focus to the image that opened it (or to its post, when the feed has unloaded the image meanwhile). Each image link is named by its alt text, or "Open image" when that is empty. The position ("2 of 4") is announced. Motion follows `prefers-reduced-motion`, and the page behind does not scroll while the dialog is open.

| Key | What it does |
|---|---|
| Enter | On an image: opens the lightbox |
| Right, Left | Next and previous item (reversed right to left); pans a zoomed image |
| Up, Down | Pans a zoomed image |
| Home, End | First and last item |
| +, -, 0 | Zooms in, zooms out, resets the zoom |
| Escape | Closes the lightbox and returns focus |
| Tab, Shift+Tab | Moves between the buttons of the dialog without leaving it |

Double click, a double tap and the zoom button zoom too, and a zoomed image pans by dragging.

## Feed

`Feed` is an endless stream of posts, newest first. It loads its rows through the same kind of action a `CardGrid` uses (a `DataQuery` with a cursor in, a `Page` out); the action's order is the feed's order, the server sets the batch size (`.batch(n)`, 10 unless set, at most `MAX_FEED_BATCH`, 50) and `.scope(field, value)` fixes a filter the client cannot change. Each row becomes a `Post`: the author with a picture and a handle, the time, the text, up to `MAX_POST_MEDIA` (4) images that open in the lightbox, the feed's reactions with the post's counts, the comment count and a share button.

```rust
Feed::of(search_posts)
    .title(t!("social.feed"))
    .post(|p: &Story| Post::new(p.author.clone(), p.at)
        .avatar(p.avatar.clone())
        .text(p.text.clone())
        .media(Image::new(p.thumb.clone()).alt(p.alt.clone()).size(300, 300).full(p.photo.clone()))
        .reactions(p.reactions())
        .comments(p.comments)
        .link(story, StoryParams { id: p.id }))
    .reactions(react_to_post)
    .reaction("like", t!("social.like"), "\u{1f44d}")
    .topic("posts")
```

The feed follows the W3C APG feed pattern: a `role="feed"` region of `<article>` elements, each focusable, named by its author, described by its text and numbered with `aria-posinset` (`aria-setsize` is -1 while more can load). `aria-busy` is set while a batch loads. Older posts load when the reader nears the end, with "Show more" as the fallback. Every text is escaped and the text of a post keeps its line breaks.

New posts never move what the reader sees. When a topic of `.topic(..)` is refreshed (an action ending with `ui.refresh("posts")`, or the realtime hub with `.live()`), the feed asks for its first batch and holds the posts above its newest one. A button over the feed says "3 new posts", and a status region says it too; pressing it inserts them above with the scroll position kept and moves focus to the first new post. When more new posts arrived than one batch holds, the button starts the feed again from the newest.

Memory stays bounded. Only the posts near the viewport are in the page; the others are kept as HTML with the height they had, and the space they take is padding, so nothing jumps when they come back. A post keeps its element while it is in the page, so focus and its row key stay put, and a focused post that scrolls far away stays in the page over its own slot. Above 1,000 posts the oldest ones far behind the reader are dropped and the button offers a way back to the newest.

A reaction press sends the post's key. The server runs the reaction action (with its permission, the CSRF check and the write rate limit) only after the feed's own action returned that post to the user with the feed's scope, so nobody can react to a post they cannot read. The action gets the key as `ReactionPress::target`. `feed.js` (15.7 KB, 5.3 KB gzip) and `feed.css` (3.6 KB) load only on pages that hold a feed.

| Key | What it does |
|---|---|
| Tab | Moves into a post, then through its links and buttons |
| Page Down, Page Up | Moves focus to the next or the previous post, loading more at the end |
| Control+End | Moves focus to the first element after the feed |
| Control+Home | Moves focus to the last element before the feed |

Measured on the component catalog's social demo (10,000 posts derived from their ids, a third with photos) in Chrome at 1440 by 900, scrolling through 2,000 posts while they load in batches of 10: at full CPU 49 fps with a p95 frame of 44 ms, at most 17 posts and 883 elements in the document, 4.6 MB of JavaScript heap.
