//! Feed examples: a stream of posts with reactions, images in the lightbox and new posts behind
//! a button, and the social demo screen with 10,000 posts derived from their ids.
//!
//! The catalog is public, so the only things visitors change are kept in two small stores with
//! a fixed cap: posts published with the button (the newest `MAX_SAMPLE_POSTS`) and reaction
//! counts (the posts reacted to last, `MAX_SAMPLE_REACTIONS`). Older entries are dropped, and
//! every write counts against the catalog's write limit.

use crate::data::people;
use crate::examples::story_ring::story_state;
use crate::{Entry, ex};
use std::collections::VecDeque;
use std::sync::Mutex;
use zeyn::prelude::*;
use zeyn::ui::{ReactionPress, ReactionState};

/// How many posts the demo derives from their ids.
pub const GENERATED: i64 = 10_000;

/// Most posts published with the button that the demo keeps; the oldest is dropped.
pub const MAX_SAMPLE_POSTS: usize = 20;

/// Most posts whose reaction counts the demo keeps; the one reacted to longest ago is dropped.
pub const MAX_SAMPLE_REACTIONS: usize = 200;

/// The moment the newest generated post was written, in Unix milliseconds.
const NEWEST_MS: i64 = 1_793_000_000_000;

/// The reactions of the demo: key, name and symbol.
const SET: [(&str, &str, &str); 3] = [
    ("like", "Like", "\u{1f44d}"),
    ("love", "Love", "\u{2764}\u{fe0f}"),
    ("idea", "Insightful", "\u{1f4a1}"),
];

const LINES: [&str; 8] = [
    "Finished the first draft of the field guide. Thirty pages of notes became eight.",
    "Morning walk along the river. The fog lifted right as we reached the bridge.",
    "Trying a new layout for the workshop. Fewer tables, more room to move.",
    "Question for everyone: what is the one tool you would never lend out?",
    "The garden finally has tomatoes. Three of them, but still.",
    "Reading about old maps today.\nThe coastlines were wrong, the confidence was not.",
    "Shipped the release. Thank you to everyone who tested it at odd hours.",
    "Rain all day, so the cookbook got some attention.",
];

/// A post of the demo.
#[entity(table = "social_posts")]
pub struct SocialPost {
    #[key]
    pub id: SocialPostId,
    pub author: i64,
    pub text: String,
    pub at_ms: i64,
    pub photos: i64,
    pub comments: i64,
}

/// Published posts: id and moment, newest last.
static PUBLISHED: Mutex<VecDeque<(i64, i64)>> = Mutex::new(VecDeque::new());

/// Reaction counts by post id, the one reacted to last at the end.
static REACTED: Mutex<VecDeque<(i64, [u64; 3])>> = Mutex::new(VecDeque::new());

/// The post with id `id` written at `at_ms`, everything else derived from the id.
fn post(id: i64, at_ms: i64) -> SocialPost {
    SocialPost {
        id: SocialPostId(id),
        author: id % people().len() as i64,
        text: LINES[(id % LINES.len() as i64) as usize].to_string(),
        at_ms,
        photos: match id % 7 {
            0 => 3,
            1 | 4 => 1,
            2 => 2,
            _ => 0,
        },
        comments: (id * 37) % 23,
    }
}

/// The generated post `id` (1 to `GENERATED`), one every seven minutes back from `NEWEST_MS`.
fn generated(id: i64) -> Option<SocialPost> {
    (1..=GENERATED)
        .contains(&id)
        .then(|| post(id, NEWEST_MS - (GENERATED - id) * 420_000))
}

/// The post with id `id`, published or generated.
fn find(id: i64) -> Option<SocialPost> {
    let published = PUBLISHED
        .lock()
        .map(|p| {
            p.iter()
                .find(|(i, _)| *i == id)
                .copied()
        })
        .unwrap_or(None);
    match published {
        Some((id, at)) => Some(post(id, at)),
        None => generated(id),
    }
}

/// How many published posts the demo keeps now.
pub fn stored_posts() -> usize {
    PUBLISHED
        .lock()
        .map(|p| p.len())
        .unwrap_or(0)
}

/// How many posts with reaction counts the demo keeps now.
pub fn stored_reactions() -> usize {
    REACTED
        .lock()
        .map(|r| r.len())
        .unwrap_or(0)
}

/// Adds a post at the top, dropping the oldest published one past the cap. Returns its id.
pub fn publish_sample(now_ms: i64) -> i64 {
    let Ok(mut published) = PUBLISHED.lock() else {
        return 0;
    };
    let id = published
        .back()
        .map_or(GENERATED + 1, |(i, _)| i + 1);
    published.push_back((id, now_ms));
    while published.len() > MAX_SAMPLE_POSTS {
        published.pop_front();
    }
    id
}

/// The newest posts first, after the cursor (the id of the last post shown). A filter on the
/// id, as the feed sends to check a reaction, gives that post alone.
#[action(public)]
pub async fn social_posts(q: DataQuery) -> Result<Page<SocialPost>> {
    if let Some(f) = q
        .filters
        .iter()
        .find(|f| f.field == "id")
    {
        let id = f
            .from
            .as_deref()
            .or(f.equals.as_deref())
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        return Ok(Page::new(find(id).into_iter().collect()));
    }
    let (cursor, limit) = match &q.page {
        zeyn::PageRequest::After { cursor, limit } => (
            cursor
                .as_ref()
                .and_then(|c| c.0.parse::<i64>().ok()),
            *limit,
        ),
        zeyn::PageRequest::Offset { limit, .. } => (None, *limit),
    };
    let below = cursor.unwrap_or(i64::MAX);
    let published: Vec<(i64, i64)> = PUBLISHED
        .lock()
        .map(|p| p.iter().rev().copied().collect())
        .unwrap_or_default();
    let items: Vec<SocialPost> = published
        .into_iter()
        .map(|(id, at)| post(id, at))
        .chain(
            (1..=GENERATED)
                .rev()
                .filter_map(generated),
        )
        .filter(|p| p.id.0 < below)
        .take(limit.clamp(1, 50) as usize)
        .collect();
    let mut page = Page::new(items);
    page.next = page
        .items
        .last()
        .filter(|p| p.id.0 > 1)
        .map(|p| zeyn::Cursor(p.id.0.to_string()));
    Ok(page)
}

/// The counts of post `id`: derived from the id, plus what visitors pressed while the post is in
/// the store.
fn counts(id: i64) -> [u64; 3] {
    let stored = REACTED
        .lock()
        .map(|r| {
            r.iter()
                .find(|(i, _)| *i == id)
                .map(|(_, c)| *c)
        })
        .unwrap_or(None);
    stored.unwrap_or([
        (id * 13 % 40) as u64,
        (id * 7 % 12) as u64,
        (id * 5 % 6) as u64,
    ])
}

/// The reaction state of a post for the page.
fn state(id: i64, mine: Option<&str>) -> ReactionState {
    let c = counts(id);
    let s = SET
        .iter()
        .zip(c)
        .fold(ReactionState::default(), |s, ((key, _, _), n)| {
            s.count(*key, n)
        });
    match mine {
        Some(m) => s.mine(m),
        None => s,
    }
}

/// Stores a press of reaction `i` on post `id`, keeping at most `MAX_SAMPLE_REACTIONS` posts: the
/// one reacted to longest ago is dropped.
pub fn count_press(id: i64, i: usize, pressed: bool) {
    let mut c = counts(id);
    c[i] = match pressed {
        true => c[i].saturating_add(1),
        false => c[i].saturating_sub(1),
    };
    if let Ok(mut store) = REACTED.lock() {
        store.retain(|(p, _)| *p != id);
        store.push_back((id, c));
        while store.len() > MAX_SAMPLE_REACTIONS {
            store.pop_front();
        }
    }
}

/// Counts a press on a post, keeping at most `MAX_SAMPLE_REACTIONS` posts. The catalog keeps no
/// users, so "mine" is the reaction just pressed.
#[action(public)]
pub async fn react_to_post(press: ReactionPress) -> Result<ReactionState> {
    let id = press
        .target
        .parse::<i64>()
        .map_err(|_| Error::NotFound)?;
    let i = SET
        .iter()
        .position(|(k, _, _)| *k == press.reaction)
        .ok_or(Error::NotFound)?;
    count_press(id, i, press.pressed);
    Ok(state(
        id,
        press
            .pressed
            .then_some(press.reaction.as_str()),
    ))
}

/// Publishes a sample post; the feed then offers it behind its "new posts" button.
#[action(public)]
pub async fn publish_post(_: ()) -> Result<i64> {
    Ok(publish_sample(DateTime::now().unix_ms()))
}

/// A post of the demo as the feed shows it: up to three photos of the large catalog, each with a
/// low resolution file first and the lightbox on.
fn as_post(p: &SocialPost) -> Post {
    let who = &people()[p.author as usize];
    let photos = (0..p.photos).map(|k| {
        let n = (p.id.0 * 3 + k) % crate::data::ITEMS + 1;
        Image::new(format!("/site/photo/{n}.svg"))
            .alt(format!("Photo {n} of {}", who.name))
            .size(300, 300)
            .lazy()
            .low(format!("/site/photo/{n}-low.svg"))
            .placeholder(Placeholder::color("--z-surface-2"))
    });
    photos.fold(
        Post::new(who.name.clone(), DateTime::from_unix_ms(p.at_ms))
            .avatar(crate::data::picture(who.id.0))
            .handle(format!(
                "@{}",
                who.name
                    .split(' ')
                    .next()
                    .unwrap_or("")
                    .to_lowercase()
            ))
            .text(p.text.clone())
            .reactions(state(p.id.0, None))
            .comments(p.comments as u64),
        |post, image| post.media(image),
    )
}

/// The feed of the demo, ten posts a batch, new posts on the topic `posts`.
fn social_feed() -> Feed<SocialPost> {
    SET.iter()
        .fold(
            Feed::of(social_posts)
                .post(as_post)
                .reactions(react_to_post),
            |feed, (key, name, symbol)| feed.reaction(key, *name, symbol),
        )
        .topic("posts")
}

/// A button that publishes a post and names the topic the feed listens to.
fn publish_button() -> ActionButton<publish_post> {
    ActionButton::new("Publish a sample post", publish_post, ())
        .on_success(|_, ui| ui.refresh("posts"))
}

pub fn overview(_ctx: &Ctx) -> Node {
    Stack::new()
        .add(publish_button())
        .add(social_feed().batch(5))
        .into()
}

/// The social demo: story rings on top, then the feed of 10,000 posts with reactions and the
/// lightbox on every photo.
#[screen(route = "/ui/feed/social", public)]
pub fn social_demo() -> Screen {
    let rings = people()
        .iter()
        .fold(StoryRow::new(), |row, p| {
            row.add(
                StoryRing::new(p.name.clone(), story_state(p.id.0))
                    .image(crate::data::picture(p.id.0)),
            )
        });
    Screen::new("Social feed, 10,000 posts")
        .body(rings)
        .body(publish_button())
        .body(social_feed().title("Posts").batch(20))
}

/// What the catalog shows of `Feed`.
pub const ENTRY: Entry = Entry {
    name: "Feed",
    docs: "components/social#feed",
    api: &["Feed", "Post"],
    overview: &[ex!(overview)],
    examples: &[],
};
