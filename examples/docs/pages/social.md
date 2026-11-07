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

