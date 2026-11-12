//! [`Feed`]: an endless stream of [`Post`]s, newest first, that loads older posts as the reader
//! scrolls and holds new ones behind a "new posts" button.

use super::catalog::check_masks;
use super::content::Avatar;
use super::embed::{Image, Zoom, lightbox_texts};
use super::reactions::{ReactionSet, reaction_row};
use super::relative::RelativeTime;
use super::widgets::screen_href;
use super::{DataSource, dom_id};
use crate::action::{Action, ActionMeta, DataQuery, Error, Filter, Page, PageRequest, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::grid::{FetchFn, fetch_via, key_filter, value_key};
use crate::i18n::Text;
use crate::model::Model;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, GridSource, LoadedPage, Node, ScreenDef};
use crate::time::DateTime;
use crate::ui::{ReactionPress, ReactionState};
use serde::Deserialize;
use serde_json::{Value as Json, json};

/// Most posts one batch of a [`Feed`] loads; [`Feed::batch`] is clamped to it.
pub const MAX_FEED_BATCH: u32 = 50;

/// Most images one [`Post`] shows; more are left out.
pub const MAX_POST_MEDIA: usize = 4;

/// Longest post key the reaction endpoint accepts, in bytes.
const MAX_KEY: usize = 128;

type PostFn<E> = Box<dyn Fn(&E) -> Post + Send + Sync>;
type ReactFn = fn(Ctx, ReactionPress) -> BoxFuture<Result<ReactionState>>;

/// Runs the reaction action `A`.
fn react_via<A>(ctx: Ctx, press: ReactionPress) -> BoxFuture<Result<ReactionState>>
where
    A: Action<Input = ReactionPress, Output = Result<ReactionState>>,
{
    Box::pin(async move { A::call(ctx, press).await? })
}

/// One post of a [`Feed`]: who wrote it and when, its text, up to [`MAX_POST_MEDIA`] images
/// (which open in the lightbox, see [`Image::lightbox`]), the reactions of the feed with this
/// post's counts, the number of comments and a share button. Built from a row by the closure of
/// [`Feed::post`]. Every text is escaped; the text keeps its line breaks.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{Post, ReactionState};
///
/// let post = Post::new("Ada Lovelace", DateTime::from_unix_ms(1_778_580_000_000))
///     .avatar("/img/people/ada.jpg")
///     .handle("@ada")
///     .text("The engine can weave algebraic patterns.")
///     .media(
///         Image::new("/img/loom-640.webp")
///             .alt("A Jacquard loom")
///             .size(640, 480)
///             .full("/img/loom-2400.webp"),
///     )
///     .reactions(ReactionState::default().count("like", 12))
///     .comments(3);
/// ```
pub struct Post {
    author: Text,
    at: DateTime,
    avatar: Option<String>,
    handle: Option<Text>,
    text: Option<Text>,
    media: Vec<Image>,
    state: ReactionState,
    comments: Option<u64>,
    link: Option<(&'static str, Json)>,
}

impl Post {
    /// A post by `author`, written at `at`.
    pub fn new(author: impl Into<Text>, at: DateTime) -> Self {
        Post {
            author: author.into(),
            at,
            avatar: None,
            handle: None,
            text: None,
            media: Vec::new(),
            state: ReactionState::default(),
            comments: None,
            link: None,
        }
    }

    /// The author's picture, following the [`Avatar`] rules; the initials without it.
    pub fn avatar(mut self, src: impl Into<String>) -> Self {
        self.avatar = Some(src.into());
        self
    }

    /// A second name next to the author, such as `@ada`, in a quieter color.
    pub fn handle(mut self, handle: impl Into<Text>) -> Self {
        self.handle = Some(handle.into());
        self
    }

    /// The text of the post. Line breaks are kept; nothing in it is read as markup.
    pub fn text(mut self, text: impl Into<Text>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// An image of the post, shown in a grid of up to [`MAX_POST_MEDIA`]; more are left out.
    /// It opens in the lightbox with the other images of the post. Give it `.full(src)` for a
    /// larger file there, or `.video(source, file)` to make it the poster of a video.
    pub fn media(mut self, image: Image) -> Self {
        if self.media.len() < MAX_POST_MEDIA {
            self.media.push(image);
        }
        self
    }

    /// The counts of the feed's reactions for this post and the ones the viewer has given.
    pub fn reactions(mut self, state: ReactionState) -> Self {
        self.state = state;
        self
    }

    /// How many comments the post has. A link to the post's screen when [`Post::link`] is set.
    pub fn comments(mut self, count: u64) -> Self {
        self.comments = Some(count);
        self
    }

    /// The post's own screen: its time and comment count link there, and a share button shares
    /// it. Params of the wrong type do not compile.
    pub fn link<S: ScreenDef>(mut self, _screen: S, params: S::Params) -> Self {
        self.link = Some((
            S::META.name,
            serde_json::to_value(params).unwrap_or(Json::Null),
        ));
        self
    }

    /// Writes the post as an `<article>` of the feed.
    fn html(self, out: &mut String, r: &Render, at: &PostAt<'_>) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.feed.{key}")));
        let id = &at.id;
        let described = if self.text.is_some() {
            format!(r#" aria-describedby="{id}-t""#)
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<article class="z-post" tabindex="0" data-zeyn-key="{}" aria-posinset="{}" aria-setsize="-1" aria-labelledby="{id}-a"{described}><header class="z-post-head">"#,
            escape(at.key),
            at.position
        ));
        let mut avatar = Avatar::new(self.author.clone()).decorative();
        if let Some(src) = self.avatar {
            avatar = avatar.image(src);
        }
        avatar.render(out, r);
        out.push_str(&format!(
            r#"<span class="z-post-who"><span class="z-post-author" id="{id}-a">{}</span>"#,
            r.text(&self.author)
        ));
        if let Some(handle) = &self.handle {
            out.push_str(&format!(
                r#"<span class="z-post-handle">{}</span>"#,
                r.text(handle)
            ));
        }
        out.push_str("</span>");
        let href = self
            .link
            .as_ref()
            .map(|(screen, params)| screen_href(r, screen, params));
        match &href {
            Some(h) => {
                out.push_str(&format!(
                    r#"<a class="z-post-time" href="{h}" data-zeyn-base-href>"#
                ));
                RelativeTime::new(self.at).render(out, r);
                out.push_str("</a>");
            }
            None => {
                out.push_str(r#"<span class="z-post-time">"#);
                RelativeTime::new(self.at).render(out, r);
                out.push_str("</span>");
            }
        }
        out.push_str("</header>");
        if let Some(text) = &self.text {
            out.push_str(&format!(
                r#"<p class="z-post-text" id="{id}-t">{}</p>"#,
                r.text(text)
            ));
        }
        if !self.media.is_empty() {
            out.push_str(&format!(
                r#"<div class="z-post-media z-post-media-{}" data-zeyn-gallery>"#,
                self.media.len()
            ));
            for (i, mut image) in self.media.into_iter().enumerate() {
                image
                    .zoom
                    .get_or_insert_with(Zoom::default)
                    .texts_elsewhere = true;
                if at.priority && i == 0 {
                    image = image.priority();
                }
                image.render(out, r);
            }
            out.push_str("</div>");
        }
        out.push_str(r#"<footer class="z-post-actions">"#);
        if let Some(set) = at.reactions {
            reaction_row(out, r, None, set, &self.state, true);
        }
        if let Some(n) = self.comments {
            let label = match n {
                1 => t("comments_one"),
                _ => t("comments").replace("{count}", &n.to_string()),
            };
            let inner = format!(
                r#"<span class="z-post-count" aria-hidden="true">{n}</span><span class="z-sr-only">{label}</span>"#
            );
            match &href {
                Some(h) => out.push_str(&format!(
                    r#"<a class="z-post-comments" href="{h}" data-zeyn-base-href>{inner}</a>"#
                )),
                None => out.push_str(&format!(r#"<span class="z-post-comments">{inner}</span>"#)),
            }
        }
        if let Some((screen, params)) = &self.link {
            super::share::Share::to(screen, params.clone()).render(out, r);
        }
        out.push_str("</footer></article>");
    }
}

/// What a post needs from its feed when it is written.
struct PostAt<'a> {
    key: &'a str,
    id: String,
    position: u64,
    priority: bool,
    reactions: Option<&'a ReactionSet>,
}

/// The reactions of a feed: the action that stores a press and the set offered.
struct FeedReactions {
    action: ActionMeta,
    call: ReactFn,
    set: ReactionSet,
}

/// An endless stream of posts, newest first, following the W3C APG feed pattern: a
/// `role="feed"` of `<article>`s, each with `aria-posinset` and `aria-setsize` and named by its
/// author, that loads older posts through the action (a `DataQuery` with a cursor in, a `Page`
/// out, as for a [`super::CardGrid`]) when the reader nears the end. The server sets the batch
/// size; the action's order is the feed's order, so return newest first. Reading needs the
/// action's access.
///
/// New posts never move what the reader sees. When a topic of [`Feed::topic`] is refreshed
/// (an action's `ui.refresh`, or the realtime hub with [`Feed::live`]), the feed asks for its
/// first batch, keeps the posts it does not have and shows a "3 new posts" button over the
/// feed; pressing it inserts them above with the scroll position held, then moves focus to the
/// first new post.
///
/// Memory stays bounded: only the posts near the viewport are in the page, the others are kept
/// as HTML with their measured height (no jump when they come back), and above 1,000 posts the
/// oldest ones the reader left far behind are dropped, with a button back to the newest. A post
/// keeps its element while it is in the page, so focus stays on it.
///
/// Reactions use the feed's set ([`Feed::reactions`], [`Feed::reaction`]) with each post's
/// counts. A press sends the post's key, and the server runs the reaction action only after the
/// feed's own action returned that post to this user, so nobody reacts to a post they cannot
/// read. Cards are built by a closure that sees the raw row, so a model with `#[mask]` fields is
/// only shown to users who may reveal them.
///
/// | Key | What it does |
/// |---|---|
/// | Page Down, Page Up | Focus to the next or the previous post |
/// | Control+End, Control+Home | Focus to the first element after or before the feed |
///
/// ```ignore
/// Feed::of(search_posts)
///     .title(t!("social.feed"))
///     .post(|p: &Story| Post::new(p.author.clone(), p.at).text(p.text.clone())
///         .reactions(p.reactions())
///         .comments(p.comments)
///         .link(story_page, StoryParams { id: p.id }))
///     .reactions(react_to_post)
///     .reaction("like", t!("social.like"), "\u{1f44d}")
///     .topic("posts")
/// ```
pub struct Feed<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    post: Option<PostFn<E>>,
    title: Option<Text>,
    batch: u32,
    topics: Vec<&'static str>,
    live: bool,
    scope: Vec<Filter>,
    reactions: Option<FeedReactions>,
    source: Option<Box<dyn DataSource>>,
}

impl<E: Model> From<Feed<E>> for Node {
    fn from(f: Feed<E>) -> Self {
        Node::new(f)
    }
}

impl<E: Model> Feed<E> {
    /// Posts from the rows of the action, 10 a batch.
    pub fn of<A>(_source: A) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<Page<E>>>,
    {
        Feed {
            action: A::META,
            fetch: fetch_via::<A, E>,
            post: None,
            title: None,
            batch: 10,
            topics: Vec::new(),
            live: false,
            scope: Vec::new(),
            reactions: None,
            source: None,
        }
    }

    /// How a row becomes a post.
    pub fn post(mut self, build: impl Fn(&E) -> Post + Send + Sync + 'static) -> Self {
        self.post = Some(Box::new(build));
        self
    }

    /// A heading over the feed, which also names it for screen readers ("Posts" unless set).
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// How many posts one batch loads, 10 unless set, at most [`MAX_FEED_BATCH`].
    pub fn batch(mut self, posts: u32) -> Self {
        self.batch = posts.clamp(1, MAX_FEED_BATCH);
        self
    }

    /// The topics after which the feed looks for new posts and offers them behind its "new
    /// posts" button, such as `"posts"` refreshed by the action that publishes one.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        self.topics = super::topic_list(topics);
        self
    }

    /// Also subscribes to the topics on the realtime hub (feature `realtime`), so a
    /// `socket.notify` from another user's action brings the "new posts" button. The topics must be
    /// declared with `Realtime::topic`.
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }

    /// Keeps only the posts whose `field` equals `value`, on every request, such as the posts of
    /// one author. The client can neither change nor remove it.
    pub fn scope<T: crate::value::FieldType>(
        mut self,
        field: crate::model::Field<E, T>,
        value: T,
    ) -> Self {
        self.scope.push(key_filter(
            field.meta(),
            value_key(&value.to_value()).as_deref(),
        ));
        self.rebuild();
        self
    }

    /// The action that stores a reaction to a post. It gets the post's key as
    /// [`ReactionPress::target`] and returns the post's new [`ReactionState`].
    pub fn reactions<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = ReactionPress, Output = Result<ReactionState>>,
    {
        let set = self
            .reactions
            .take()
            .map(|r| r.set)
            .unwrap_or_default();
        self.reactions = Some(FeedReactions {
            action: A::META,
            call: react_via::<A>,
            set,
        });
        self.rebuild();
        self
    }

    /// A reaction every post offers: its key (what the action gets), its name and its symbol.
    /// Needs [`Feed::reactions`] first; at most [`super::MAX_REACTIONS`].
    pub fn reaction(
        mut self,
        key: &'static str,
        name: impl Into<Text>,
        symbol: &'static str,
    ) -> Self {
        if let Some(r) = &mut self.reactions {
            r.set.add(key, name.into(), symbol);
        }
        self.rebuild();
        self
    }

    /// Builds the data source of reaction presses again after the set or the scope changed.
    fn rebuild(&mut self) {
        self.source = self.reactions.as_ref().map(|r| {
            Box::new(PressSource::<E> {
                fetch: self.fetch,
                read: self.action,
                react: r.action,
                call: r.call,
                scope: self.scope.clone(),
                allowed: r.set.keys(),
            }) as Box<dyn DataSource>
        });
    }

    /// The query the server runs: the client's cursor only.
    fn allow(&self, query: &mut DataQuery) {
        let cursor = match &query.page {
            PageRequest::After { cursor, .. } => cursor.clone(),
            PageRequest::Offset { .. } => None,
        };
        query.page = PageRequest::After {
            cursor,
            limit: self.batch,
        };
        query.sort.clear();
        query.filters.clone_from(&self.scope);
        query.q.clear();
        query.with_total = false;
        query.aggregates.clear();
        query.group = None;
    }

    /// The posts of a loaded page as `<article>`s. The first post of the page rendered with the
    /// screen loads its first image at once.
    fn posts(&self, page: &LoadedPage, r: &Render, first: bool) -> String {
        let mut out = String::new();
        let (Some(rows), Some(build)) = (page.items.downcast_ref::<Vec<E>>(), &self.post) else {
            return out;
        };
        let key = E::fields().iter().position(|f| f.key);
        let base = dom_id(r, "feed");
        for (i, row) in rows.iter().enumerate() {
            let k = key
                .and_then(|k| row.value_at(k))
                .and_then(|v| value_key(&v))
                .unwrap_or_else(|| i.to_string());
            let at = PostAt {
                id: format!("{base}-{}", id_part(&k)),
                key: &k,
                position: i as u64 + 1,
                priority: first && i == 0,
                reactions: self.reactions.as_ref().map(|r| &r.set),
            };
            build(row).html(&mut out, r, &at);
        }
        out
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let id = dom_id(r, "feed");
        let t = |key: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.feed.{key}")))
        };
        let texts = json!({
            "new_one": t("new_one"),
            "new_many": t("new_many"),
            "new_more": t("new_more"),
            "newest": t("newest"),
            "loaded": t("loaded"),
            "error": t("error"),
        });
        let topics = self.topics.join(",");
        let live = if self.live && !topics.is_empty() {
            format!(
                r#" data-zeyn-live="{}" data-zeyn-live-self"#,
                escape(&topics)
            )
        } else {
            String::new()
        };
        let next = page
            .and_then(|p| p.next.as_ref())
            .map(|c| format!(r#" data-zeyn-next="{}""#, escape(&c.0)))
            .unwrap_or_default();
        out.push_str(&format!(
            r#"<section class="z-feed" data-zeyn-feed="{}"{live}{next} data-texts="{}" data-zeyn-lightbox-texts="{}" data-testid="{}.feed">"#,
            escape(&topics),
            attr(&texts),
            attr(&lightbox_texts(r)),
            r.scope
        ));
        let name = match &self.title {
            Some(title) => {
                out.push_str(&format!(
                    r#"<h2 class="z-feed-title" id="{id}-title">{}</h2>"#,
                    r.text(title)
                ));
                format!(r#"aria-labelledby="{id}-title""#)
            }
            None => format!(r#"aria-label="{}""#, escape(&t("label"))),
        };
        out.push_str(&format!(
            r#"<div class="z-feed-pill-slot"><button type="button" class="z-button z-feed-pill" data-zeyn-feed-pill aria-controls="{id}-list" hidden></button></div><div class="z-feed-list" id="{id}-list" role="feed" {name} aria-busy="false" data-zeyn-rows>"#
        ));
        if let Some(p) = page {
            out.push_str(&self.posts(p, r, true));
        }
        let empty = page.is_none_or(|p| p.count == 0);
        out.push_str(&format!(
            r#"</div><p class="z-feed-empty" data-zeyn-feed-empty{}>{}</p><div class="z-feed-more"><button type="button" class="z-button" data-zeyn-feed-more{}>{}</button></div><p class="z-sr-only" role="status" aria-live="polite" data-zeyn-feed-status></p></section>"#,
            if empty { "" } else { " hidden" },
            escape(&t("empty")),
            if next.is_empty() { " hidden" } else { "" },
            r.text(&Text::key("zeyn.ui.cards.load_more"))
        ));
    }
}

/// A key as a part of an element id: itself when it is letters, digits, `-` and `_`, else its
/// bytes in hex.
fn id_part(key: &str) -> String {
    if !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        key.to_string()
    } else {
        key.bytes()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

impl<E: Model> GridSource for Feed<E> {
    fn action(&self) -> ActionMeta {
        self.action
    }

    fn first_query(&self, _view: Option<&crate::grid::DataGridView>) -> DataQuery {
        let mut query = DataQuery::default();
        self.allow(&mut query);
        query
    }

    fn clean_view(&self, _view: crate::grid::DataGridView) -> crate::grid::DataGridView {
        crate::grid::DataGridView::default()
    }

    fn head_html(&self, _r: &Render, _view: Option<&crate::grid::DataGridView>) -> String {
        String::new()
    }

    fn fetch(&self, ctx: Ctx, mut query: DataQuery) -> BoxFuture<Result<LoadedPage>> {
        self.allow(&mut query);
        query.time_zone = ctx.time_zone();
        let fetch = self.fetch;
        Box::pin(async move {
            check_masks::<E>(&ctx)?;
            let page = fetch(ctx, query).await?;
            Ok(LoadedPage {
                count: page.items.len(),
                total: None,
                next: page.next,
                offset: 0,
                totals: Vec::new(),
                items: Box::new(page.items),
                view: None,
                saved_views: false,
                depth: 0,
                path: Vec::new(),
                groups: None,
                parents: Vec::new(),
            })
        })
    }

    fn rows_html(&self, page: &LoadedPage, r: &Render) -> String {
        self.posts(page, r, false)
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        json!({
            "type": "object",
            "description": "Posts, newest first. The server sets the batch size; pass the cursor of the last batch to get older posts.",
            "properties": {
                "page": {
                    "type": "object",
                    "properties": {
                        "type": { "const": "after" },
                        "cursor": { "type": ["string", "null"] }
                    }
                }
            }
        })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for Feed<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        json!({
            "type": "feed",
            "model": E::NAME,
            "action": self.action,
            "title": self.title,
            "batch": self.batch,
            "topics": self.topics,
            "reactions": self.reactions.as_ref().map(|r| json!({ "action": r.action, "set": r.set.schema() })),
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "react")
            .then_some(self.source.as_deref())
            .flatten()
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
        if self.post.is_none() {
            check.problem("a `Feed` needs `.post(|row| Post::new(..))` to show its rows");
        }
        if self
            .reactions
            .as_ref()
            .is_some_and(|r| r.set.keys().is_empty())
        {
            check.problem("`Feed::reactions` without `.reaction(key, name, symbol)` offers nothing to press; add the reactions or leave it out");
        }
    }
}

/// What the browser sends with a reaction press in a feed: the post's key, the reaction and
/// whether it was turned on.
#[derive(Deserialize)]
struct FeedPress {
    key: String,
    reaction: String,
    #[serde(default)]
    pressed: bool,
}

/// Runs the reaction action for a post of the feed, after the feed's own action returned that
/// post to the user: the key comes from the browser, so it is checked against what the user may
/// read, with the feed's scope.
struct PressSource<E: Model> {
    fetch: FetchFn<E>,
    read: ActionMeta,
    react: ActionMeta,
    call: ReactFn,
    scope: Vec<Filter>,
    allowed: Vec<&'static str>,
}

impl<E: Model> DataSource for PressSource<E> {
    fn action(&self) -> ActionMeta {
        self.react
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let press = serde_json::from_value::<FeedPress>(input)
            .ok()
            .filter(|p| {
                (1..=MAX_KEY).contains(&p.key.len())
                    && self
                        .allowed
                        .contains(&p.reaction.as_str())
            });
        let readable = ctx.identity().can(&self.read.access);
        let key_field = E::fields().iter().find(|f| f.key);
        let fetch = self.fetch;
        let call = self.call;
        let mut filters = self.scope.clone();
        Box::pin(async move {
            let press = press.ok_or(Error::NotFound)?;
            if !readable {
                return Err(Error::Forbidden);
            }
            check_masks::<E>(&ctx)?;
            let field = key_field.ok_or(Error::NotFound)?;
            filters.push(key_filter(field, Some(&press.key)));
            let index = E::fields()
                .iter()
                .position(|f| f.key)
                .ok_or(Error::NotFound)?;
            let query = DataQuery {
                page: PageRequest::Offset {
                    offset: 0,
                    limit: 2,
                },
                filters,
                time_zone: ctx.time_zone(),
                ..DataQuery::default()
            };
            let page = fetch(ctx.clone(), query).await?;
            let found = page.items.iter().any(|row| {
                row.value_at(index)
                    .and_then(|v| value_key(&v))
                    .is_some_and(|k| k == press.key)
            });
            if !found {
                return Err(Error::NotFound);
            }
            let state = call(
                ctx.clone(),
                ReactionPress {
                    target: press.key,
                    reaction: press.reaction,
                    pressed: press.pressed,
                },
            )
            .await?;
            crate::privacy::client_json(&state, &ctx).map_err(|e| Error::Internal(e.to_string()))
        })
    }
}
