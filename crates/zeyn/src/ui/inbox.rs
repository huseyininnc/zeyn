//! [`Inbox`]: notifications of the user grouped by day, read and unread, with bulk mark read
//! and archive; [`NotificationCenter`]: the bell of the top bar with the unread count.

use super::catalog::check_masks;
use super::list::ListItem;
use super::widgets::screen_href;
use super::{DataSource, dom_id, topic_list, topics_attr};
use crate::action::{Action, ActionMeta, DataQuery, Error, PageRequest, Result, Sort};
use crate::ctx::{BoxFuture, Ctx};
use crate::grid::{FetchFn, fetch_via};
use crate::i18n::Text;
use crate::model::{Field, Model};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, GridSource, LoadedPage, Node, ScreenDef};
use crate::time::{Date, DateTime};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

/// Most notifications one [`Inbox`] shows, and most ids one bulk action takes.
pub const MAX_INBOX: u32 = 200;

/// Most unread notifications the count of a [`NotificationCenter`] asks for; more show as
/// `99+`.
pub const MAX_UNREAD: u32 = 100;

/// What a bulk action of an [`Inbox`] gets: the keys of the chosen notifications, at most
/// [`MAX_INBOX`], each at most 200 characters. The keys come from the browser: the action must
/// change only notifications of `ctx.identity()`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxSelection {
    pub ids: Vec<String>,
}

/// What the count action of a [`NotificationCenter`] gets: count unread notifications of the
/// user, but stop at `limit` (one bounded query, such as `SELECT count(*) FROM (SELECT 1 ...
/// LIMIT $limit)`). The server sets it to [`MAX_UNREAD`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnreadCount {
    pub limit: u32,
}

/// A bulk action behind an inbox button: checks the selection, then runs the action.
struct BulkSource<A>(PhantomData<fn() -> A>);

impl<A, T> DataSource for BulkSource<A>
where
    A: Action<Input = InboxSelection, Output = Result<T>>,
    T: Send + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let selection: InboxSelection =
                serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            let fits = !selection.ids.is_empty()
                && selection.ids.len() <= MAX_INBOX as usize
                && selection
                    .ids
                    .iter()
                    .all(|id| !id.is_empty() && id.len() <= 200);
            if !fits {
                return Err(Error::NotFound);
            }
            let count = selection.ids.len();
            A::call(ctx, selection).await??;
            Ok(json!({ "done": count }))
        })
    }
}

/// The count behind a notification center: always asks for at most [`MAX_UNREAD`].
struct CountSource<A>(PhantomData<fn() -> A>);

impl<A> DataSource for CountSource<A>
where
    A: Action<Input = UnreadCount, Output = Result<u32>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, _input: Json) -> BoxFuture<Result<Json>> {
        Box::pin(async move {
            let n = A::call(ctx, UnreadCount { limit: MAX_UNREAD }).await??;
            Ok(json!({ "unread": n.min(MAX_UNREAD) }))
        })
    }
}

type ItemFn<E> = Box<dyn Fn(&E) -> ListItem + Send + Sync>;

/// A bulk button: its key, label and action.
struct Bulk {
    key: &'static str,
    label: Text,
    source: Box<dyn DataSource>,
}

/// The notifications of the user, newest first and grouped by day in the user's time zone
/// (today, yesterday, then dates), unread ones marked. Rows come from the same kind of action a
/// DataGrid uses (`DataQuery` in, `Page` out), sorted by the time field, at most
/// [`Inbox::limit`] of them; each is shown as a [`ListItem`].
///
/// With [`Inbox::mark_read`] and [`Inbox::archive`] every row gets a check box and the inbox a
/// toolbar: the buttons run the action with an [`InboxSelection`] through `POST /_zeyn/data`
/// (CSRF, write limit, the screen's access and the action's permission), then the inbox and
/// every node that listens to its topic refresh, the [`NotificationCenter`] count among them.
/// Buttons the user may not run are left out.
///
/// ```ignore
/// Inbox::of(my_notifications)
///     .when(Note::created_at)
///     .read(Note::read)
///     .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
///     .mark_read(mark_notes_read)
///     .archive(archive_notes)
/// ```
pub struct Inbox<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    when: Option<Field<E, DateTime>>,
    read: Option<Field<E, bool>>,
    item: Option<ItemFn<E>>,
    title: Option<Text>,
    limit: u32,
    bulk: Vec<Bulk>,
    topics: Vec<&'static str>,
}

impl<E: Model> Inbox<E> {
    pub fn of<A>(_source: A) -> Self
    where
        A: Action<Input = DataQuery, Output = Result<crate::action::Page<E>>>,
    {
        Inbox {
            action: A::META,
            fetch: fetch_via::<A, E>,
            when: None,
            read: None,
            item: None,
            title: None,
            limit: 50,
            bulk: Vec::new(),
            topics: vec!["notifications"],
        }
    }

    /// When a notification arrived: the rows are sorted by it, newest first, and grouped by its
    /// day in the user's time zone.
    pub fn when(mut self, field: Field<E, DateTime>) -> Self {
        self.when = Some(field);
        self
    }

    /// Whether a notification was read. Unread ones are marked and counted.
    pub fn read(mut self, field: Field<E, bool>) -> Self {
        self.read = Some(field);
        self
    }

    /// How a notification is shown.
    pub fn item(mut self, build: impl Fn(&E) -> ListItem + Send + Sync + 'static) -> Self {
        self.item = Some(Box::new(build));
        self
    }

    /// A heading over the inbox, which also names it for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// How many notifications to show, 1 to [`MAX_INBOX`]. 50 unless set.
    pub fn limit(mut self, rows: u32) -> Self {
        self.limit = rows.clamp(1, MAX_INBOX);
        self
    }

    /// The topics the inbox listens to and names after a bulk action, `"notifications"` unless
    /// set. Name the same topic in `hub.notify` when a notification arrives.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        self.topics = topic_list(topics);
        self
    }

    /// A "Mark as read" button for the chosen notifications.
    pub fn mark_read<A, T>(self, _action: A) -> Self
    where
        A: Action<Input = InboxSelection, Output = Result<T>>,
        T: Send + 'static,
    {
        self.bulk_action::<A, T>("read", Text::key("zeyn.ui.inbox.mark_read"))
    }

    /// An "Archive" button for the chosen notifications.
    pub fn archive<A, T>(self, _action: A) -> Self
    where
        A: Action<Input = InboxSelection, Output = Result<T>>,
        T: Send + 'static,
    {
        self.bulk_action::<A, T>("archive", Text::key("zeyn.ui.inbox.archive"))
    }

    fn bulk_action<A, T>(mut self, key: &'static str, label: Text) -> Self
    where
        A: Action<Input = InboxSelection, Output = Result<T>>,
        T: Send + 'static,
    {
        self.bulk.retain(|b| b.key != key);
        self.bulk.push(Bulk {
            key,
            label,
            source: Box::new(BulkSource::<A>(PhantomData)),
        });
        self
    }

    fn allow(&self, query: &mut DataQuery) {
        *query = DataQuery {
            page: PageRequest::Offset {
                offset: 0,
                limit: self.limit,
            },
            sort: self
                .when
                .iter()
                .map(|f| Sort {
                    field: f.meta().name.to_string(),
                    desc: true,
                })
                .collect(),
            time_zone: query.time_zone,
            ..DataQuery::default()
        };
    }

    /// The heading of a day: today, yesterday, or the weekday and date.
    fn day_label(&self, r: &Render, day: Date, today: Date) -> String {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.inbox.{key}")));
        if day == today {
            return t("today");
        }
        if today.add_days(-1) == Some(day) {
            return t("yesterday");
        }
        format!(
            "{} {}",
            r.text(&Text::key(format!("zeyn.ui.week.days.{}", day.weekday()))),
            escape(&day.format(r.locale))
        )
    }

    fn rows(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.inbox.{key}")));
        let rows = page
            .and_then(|p| p.items.downcast_ref::<Vec<E>>())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if rows.is_empty() {
            out.push_str(&format!(r#"<p class="z-inbox-empty">{}</p>"#, t("empty")));
            return;
        }
        let today = r
            .ctx
            .map(|c| c.today())
            .unwrap_or_else(|| r.tz.today(DateTime::now()));
        let key = E::fields().iter().position(|f| f.key);
        let id = dom_id(r, "inbox");
        let selectable = self
            .bulk
            .iter()
            .any(|b| r.can(&b.source.action().access));
        let mut day: Option<Date> = None;
        for (i, row) in rows.iter().enumerate() {
            let at = self.when.map(|f| *f.get(row));
            let local = at.map(|a| a.local(r.tz));
            let this = local.map(|l| l.date);
            if i == 0 || this != day {
                if i > 0 {
                    out.push_str("</ul></section>");
                }
                let label = this
                    .map(|d| self.day_label(r, d, today))
                    .unwrap_or_default();
                out.push_str(&format!(
                    r#"<section class="z-inbox-day" aria-labelledby="{id}-d{i}"><h3 class="z-inbox-day-name" id="{id}-d{i}">{label}</h3><ul class="z-list z-inbox-items">"#
                ));
                day = this;
            }
            let unread = self.read.is_some_and(|f| !*f.get(row));
            let k = key
                .and_then(|k| row.value_at(k))
                .and_then(|v| crate::grid::value_key(&v))
                .unwrap_or_default();
            out.push_str(&format!(
                r#"<li class="z-inbox-item{}" data-zeyn-key="{}">"#,
                if unread { " z-inbox-unread" } else { "" },
                escape(&k)
            ));
            if selectable {
                out.push_str(&format!(
                    r#"<input type="checkbox" class="z-inbox-check" id="{id}-c{i}" value="{}" aria-describedby="{id}-t{i}"><label class="z-sr-only" for="{id}-c{i}">{}</label>"#,
                    escape(&k),
                    t("select")
                ));
            }
            out.push_str(&format!(r#"<div class="z-inbox-body" id="{id}-t{i}">"#));
            if unread {
                out.push_str(&format!(
                    r#"<span class="z-inbox-dot" aria-hidden="true"></span><span class="z-sr-only">{}: </span>"#,
                    t("unread")
                ));
            }
            if let Some(item) = &self.item {
                let mut li = String::new();
                item(row).html(&mut li, r);
                let inner = li
                    .strip_prefix(r#"<li class="z-list-item">"#)
                    .and_then(|s| s.strip_suffix("</li>"))
                    .unwrap_or(&li);
                out.push_str(inner);
            }
            out.push_str("</div>");
            if let (Some(a), Some(l)) = (at, local) {
                out.push_str(&format!(
                    r#"<time class="z-inbox-time" datetime="{a}">{}</time>"#,
                    escape(
                        &crate::time::Time::new(l.time.hour(), l.time.minute(), 0)
                            .unwrap_or_default()
                            .format(r.locale)
                    )
                ));
            }
            out.push_str("</li>");
        }
        out.push_str("</ul></section>");
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.inbox.{key}")));
        let id = dom_id(r, "inbox");
        let (title, labelled) = match &self.title {
            Some(title) => (
                format!(
                    r#"<h2 class="z-inbox-title" id="{id}-title">{}</h2>"#,
                    r.text(title)
                ),
                format!(r#" aria-labelledby="{id}-title""#),
            ),
            None => (String::new(), format!(r#" aria-label="{}""#, t("label"))),
        };
        let unread = match (page, &self.read) {
            (Some(p), Some(f)) => p
                .items
                .downcast_ref::<Vec<E>>()
                .map_or(0, |rows| {
                    rows.iter()
                        .filter(|row| !*f.get(row))
                        .count()
                }),
            _ => 0,
        };
        let allowed: Vec<&Bulk> = self
            .bulk
            .iter()
            .filter(|b| r.can(&b.source.action().access))
            .collect();
        let texts = json!({
            "selected": r.i18n.text(r.locale, &Text::key("zeyn.ui.inbox.selected")),
            "done": r.i18n.text(r.locale, &Text::key("zeyn.ui.inbox.done")),
            "failed": r.i18n.text(r.locale, &Text::key("zeyn.ui.load_failed")),
        });
        out.push_str(&format!(
            r#"<section class="z-inbox"{labelled}{} data-testid="{}.inbox"{}>"#,
            topics_attr("data-zeyn-listens", &self.topics),
            r.scope,
            if allowed.is_empty() {
                String::new()
            } else {
                format!(
                    r#" data-zeyn-inbox data-topics="{}" data-texts="{}""#,
                    escape(&self.topics.join(",")),
                    attr(&texts)
                )
            }
        ));
        out.push_str(&format!(
            r#"<div class="z-inbox-head">{title}<p class="z-inbox-count">{}</p>"#,
            r.text(&Text::key("zeyn.ui.inbox.unread_count").arg("n", unread.to_string()))
        ));
        if !allowed.is_empty() {
            out.push_str(&format!(
                r#"<div class="z-inbox-bar" role="toolbar" aria-label="{}"><label class="z-inbox-all"><input type="checkbox" class="z-inbox-check-all"> {}</label>"#,
                t("actions"),
                t("select_all")
            ));
            for b in allowed {
                out.push_str(&format!(
                    r#"<button type="button" class="z-button" data-zeyn-inbox-run="{}" aria-disabled="true">{}</button>"#,
                    b.key,
                    r.text(&b.label)
                ));
            }
            out.push_str("</div>");
        }
        out.push_str("</div>");
        self.rows(out, r, page);
        out.push_str(r#"<p class="z-sr-only" role="status" aria-live="polite" data-zeyn-inbox-status></p></section>"#);
    }
}

impl<E: Model> GridSource for Inbox<E> {
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
                total: page.total,
                next: None,
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
        let mut out = String::new();
        self.rows(&mut out, r, Some(page));
        out
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "description": "Notifications of the user, newest first. The server sets the query." })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for Inbox<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        json!({
            "type": "inbox",
            "model": E::NAME,
            "action": self.action,
            "title": self.title,
            "limit": self.limit,
            "bulk": self.bulk.iter().map(|b| json!({ "key": b.key, "action": b.source.action() })).collect::<Vec<_>>(),
            "topics": self.topics,
        })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        self.bulk
            .iter()
            .find(|b| b.key == key)
            .map(|b| b.source.as_ref())
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
        if self.when.is_none() {
            check.problem("an Inbox needs `.when(field)`, the time a notification arrived, to sort and group by day");
        }
    }
}

impl<E: Model> From<Inbox<E>> for Node {
    fn from(i: Inbox<E>) -> Self {
        Node::new(i)
    }
}

/// The bell of the top bar: a link to the inbox screen with the number of unread
/// notifications, from an action that counts them in one bounded query ([`UnreadCount`]). Put
/// it in the end slot of `App::header`. The count loads with one `POST /_zeyn/data` after the
/// page shows (the action's permission checked) and again whenever one of its topics is named:
/// after a bulk action of an [`Inbox`], `ui.refresh("notifications")` or, with
/// [`NotificationCenter::live`] and the `realtime` feature, `hub.notify(&ctx, "notifications")`.
/// Above 99 it shows `99+`. The link's name says the count, for screen readers.
///
/// ```ignore
/// App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
/// ```
pub struct NotificationCenter {
    source: Box<dyn DataSource>,
    href: (&'static str, Json),
    topics: Vec<&'static str>,
    live: bool,
}

impl NotificationCenter {
    /// A bell that counts with `count` and links to `screen`. Params of the wrong type do not
    /// compile.
    pub fn of<A, S: ScreenDef>(_count: A, _screen: S, params: S::Params) -> Self
    where
        A: Action<Input = UnreadCount, Output = Result<u32>>,
    {
        NotificationCenter {
            source: Box::new(CountSource::<A>(PhantomData)),
            href: (
                S::META.name,
                serde_json::to_value(params).unwrap_or(Json::Null),
            ),
            topics: vec!["notifications"],
            live: false,
        }
    }

    /// The topics after which the count loads again, `"notifications"` unless set.
    pub fn topic(mut self, topics: impl super::Topics) -> Self {
        self.topics = topic_list(topics);
        self
    }

    /// Also subscribes to the topics on the realtime hub (feature `realtime`), so a
    /// `hub.notify` from another user's action updates the count. The topics must be declared
    /// with `Realtime::topic`.
    pub fn live(mut self) -> Self {
        self.live = true;
        self
    }
}

impl Component for NotificationCenter {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| {
            r.i18n
                .text(r.locale, &Text::key(format!("zeyn.ui.inbox.{key}")))
        };
        let texts = json!({ "none": t("bell"), "some": t("bell_count"), "many": t("bell_many") });
        let topics = self.topics.join(",");
        let live = if self.live {
            format!(
                r#" data-zeyn-live="{}" data-zeyn-live-self"#,
                escape(&topics)
            )
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<a class="z-notify" href="{}" data-zeyn-base-href data-zeyn-notify="{}"{live} data-texts="{}" aria-label="{}" data-testid="{}.notify">{}<span class="z-notify-count" aria-hidden="true" hidden></span></a>"#,
            screen_href(r, self.href.0, &self.href.1),
            escape(&topics),
            attr(&texts),
            escape(&t("bell")),
            r.scope,
            crate::screen::Icon::Bell.html()
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "notification_center", "action": self.source.action(), "screen": self.href.0, "topics": self.topics })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "count").then_some(self.source.as_ref())
    }

    fn page_check(&self, check: &mut super::PageCheck) {
        for t in &self.topics {
            check.listens(t);
        }
    }
}

impl From<NotificationCenter> for Node {
    fn from(n: NotificationCenter) -> Self {
        Node::new(n)
    }
}
