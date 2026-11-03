//! [`WeekSchedule`]: seven day columns of rows from an action, such as classes, shifts or events
//! of a week.

use super::catalog::check_masks;
use super::dom_id;
use super::list::ListItem;
use crate::action::{ActionMeta, DataQuery, PageRequest, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::grid::{FetchFn, fetch_via};
use crate::i18n::Text;
use crate::model::Model;
use crate::render::Render;
use crate::screen::{Component, GridSource, LoadedPage, Node};
use crate::time::{Date, DateTime};
use serde_json::{Value as Json, json};

/// What a row of a [`WeekSchedule`] gives as its day: a [`Date`], an optional date (rows without
/// one are left out) or a weekday number, 1 for Monday to 7 for Sunday.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a day of the week",
    label = "return a Date, an Option<Date> or a weekday number (u8, 1 = Monday to 7 = Sunday)",
    note = "for example `.day(|e: &Event| e.date)` or `.day(|c: &Class| c.weekday)`"
)]
pub trait WeekDay {
    /// 1 for Monday to 7 for Sunday, or `None` for a row that belongs to no day.
    fn weekday(&self) -> Option<u8>;
}

impl WeekDay for Date {
    fn weekday(&self) -> Option<u8> {
        Some(Date::weekday(self))
    }
}

impl WeekDay for Option<Date> {
    fn weekday(&self) -> Option<u8> {
        self.as_ref().map(Date::weekday)
    }
}

impl WeekDay for u8 {
    fn weekday(&self) -> Option<u8> {
        (1..=7).contains(self).then_some(*self)
    }
}

type DayFn<E> = Box<dyn Fn(&E) -> Option<u8> + Send + Sync>;
type ItemFn<E> = Box<dyn Fn(&E) -> ListItem + Send + Sync>;

/// Most rows one schedule loads.
pub const MAX_WEEK_ROWS: u32 = 500;

/// Seven day columns of rows from an action, in the order of the user's week (Monday first,
/// Sunday first in `en-US`), with today marked. Each row is placed by its day and shown as a
/// [`ListItem`], so it can link to a screen. On phones the days stack.
///
/// The action is the same kind a DataGrid uses (`DataQuery` in, `Page` out) and is checked the
/// same way; the server asks for up to `limit` rows (100 unless set, at most
/// [`MAX_WEEK_ROWS`]) in the action's order. A model with `#[mask]` fields is only shown to
/// users who may reveal them.
///
/// ```ignore
/// WeekSchedule::of(this_week_classes)
///     .title(t!("classes.this_week"))
///     .day(|c: &Class| c.date)
///     .item(|c: &Class| ListItem::new(c.name.clone()).meta(c.starts.to_string()).link(class, ClassParams { id: c.id }))
/// ```
///
/// A row must give its day as a date or a weekday number:
///
/// ```compile_fail
/// use zeyn::prelude::*;
/// use zeyn::ui::WeekSchedule;
///
/// #[model]
/// pub struct Shift {
///     pub name: String,
/// }
///
/// #[action(public)]
/// pub async fn shifts(q: DataQuery) -> Result<Page<Shift>> {
///     Ok(q.apply(Vec::new()))
/// }
///
/// let week = WeekSchedule::of(shifts).day(|s: &Shift| s.name.clone());
/// ```
pub struct WeekSchedule<E: Model> {
    action: ActionMeta,
    fetch: FetchFn<E>,
    day: Option<DayFn<E>>,
    item: Option<ItemFn<E>>,
    title: Option<Text>,
    limit: u32,
}

impl<E: Model> WeekSchedule<E> {
    pub fn of<A>(_source: A) -> Self
    where
        A: crate::action::Action<Input = DataQuery, Output = Result<crate::action::Page<E>>>,
    {
        WeekSchedule {
            action: A::META,
            fetch: fetch_via::<A, E>,
            day: None,
            item: None,
            title: None,
            limit: 100,
        }
    }

    /// The day of a row: a [`Date`], an `Option<Date>` or a weekday number (1 = Monday).
    pub fn day<D: WeekDay>(mut self, day: impl Fn(&E) -> D + Send + Sync + 'static) -> Self {
        self.day = Some(Box::new(move |row| day(row).weekday()));
        self
    }

    /// How a row is shown in its day.
    pub fn item(mut self, build: impl Fn(&E) -> ListItem + Send + Sync + 'static) -> Self {
        self.item = Some(Box::new(build));
        self
    }

    /// A heading over the week, which also names it for screen readers.
    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// How many rows to load, 1 to [`MAX_WEEK_ROWS`]. 100 unless set.
    pub fn limit(mut self, rows: u32) -> Self {
        self.limit = rows.clamp(1, MAX_WEEK_ROWS);
        self
    }

    fn allow(&self, query: &mut DataQuery) {
        *query = DataQuery {
            page: PageRequest::Offset {
                offset: 0,
                limit: self.limit,
            },
            time_zone: query.time_zone,
            ..DataQuery::default()
        };
    }

    fn days(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let today = r
            .ctx
            .map(|c| c.today())
            .unwrap_or_else(|| r.tz.today(DateTime::now()))
            .weekday();
        let first = Date::first_weekday(r.locale);
        let rows = page.and_then(|p| p.items.downcast_ref::<Vec<E>>());
        let id = dom_id(r, "week");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.week.{key}")));
        for i in 0..7u8 {
            let day = (first - 1 + i) % 7 + 1;
            let is_today = day == today;
            out.push_str(&format!(
                r#"<section class="z-week-day{}" aria-labelledby="{id}-{day}"><h3 class="z-week-name" id="{id}-{day}">{}{}</h3>"#,
                if is_today { " z-week-today" } else { "" },
                t(&format!("days.{day}")),
                if is_today { format!(r#" <span class="z-week-badge">{}</span>"#, t("today")) } else { String::new() }
            ));
            let mut items = String::new();
            if let (Some(rows), Some(place), Some(item)) = (rows, &self.day, &self.item) {
                for row in rows
                    .iter()
                    .filter(|row| place(row) == Some(day))
                {
                    item(row).html(&mut items, r);
                }
            }
            if items.is_empty() {
                out.push_str(&format!(r#"<p class="z-week-empty">{}</p>"#, t("empty")));
            } else {
                out.push_str(&format!(r#"<ul class="z-list z-week-items">{items}</ul>"#));
            }
            out.push_str("</section>");
        }
    }

    fn render_with(&self, out: &mut String, r: &Render, page: Option<&LoadedPage>) {
        let id = dom_id(r, "week");
        let (title, labelled) = match &self.title {
            Some(t) => (
                format!(
                    r#"<h2 class="z-cards-title" id="{id}-title">{}</h2>"#,
                    r.text(t)
                ),
                format!(r#" aria-labelledby="{id}-title""#),
            ),
            None => (
                String::new(),
                format!(
                    r#" aria-label="{}""#,
                    r.text(&Text::key("zeyn.ui.week.label"))
                ),
            ),
        };
        out.push_str(&format!(
            r#"<section class="z-week"{labelled}>{title}<div class="z-week-days">"#
        ));
        self.days(out, r, page);
        out.push_str("</div></section>");
    }
}

impl<E: Model> GridSource for WeekSchedule<E> {
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
        self.days(&mut out, r, Some(page));
        out
    }

    fn foot_html(&self, _page: &LoadedPage, _r: &Render) -> Option<String> {
        None
    }

    fn render_loaded(&self, out: &mut String, r: &Render, page: &LoadedPage) {
        self.render_with(out, r, Some(page));
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "description": "Rows of the week. The server sets the query; nothing can be chosen." })
    }

    fn fields(&self) -> &'static [crate::model::FieldMeta] {
        E::fields()
    }
}

impl<E: Model> Component for WeekSchedule<E> {
    fn render(&self, out: &mut String, r: &Render) {
        self.render_with(out, r, None);
    }

    fn schema(&self) -> Json {
        json!({ "type": "week_schedule", "model": E::NAME, "action": self.action, "title": self.title, "limit": self.limit })
    }

    fn grid(&self) -> Option<&dyn GridSource> {
        Some(self)
    }
}

impl<E: Model> From<WeekSchedule<E>> for Node {
    fn from(w: WeekSchedule<E>) -> Self {
        Node::new(w)
    }
}
