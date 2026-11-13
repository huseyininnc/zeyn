//! List examples: rows with an image, a title and meta text, and a list people put in their own
//! order. The sample steps of the sortable list are shared by every visitor and only change
//! order, so the store never grows.

use crate::data::{Person, Product, people, search_people, search_products};
use crate::{Entry, ex};
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;
use zeyn::ui::ListItem;

/// A step of the sortable sample list.
#[entity(table = "steps")]
pub struct Step {
    #[key]
    pub id: StepId,
    pub title: String,
    pub rank: Rank,
}

/// Steps with the given titles, ranked in that order, with ids from 1.
fn seed(titles: impl Iterator<Item = String>) -> Mutex<Vec<Step>> {
    seed_from(1, titles)
}

/// Steps with the given titles, ranked in that order, with ids from `first`.
fn seed_from(first: i64, titles: impl Iterator<Item = String>) -> Mutex<Vec<Step>> {
    let titles: Vec<String> = titles.collect();
    let ranks = zeyn::rank::rebalance(titles.len());
    Mutex::new(
        titles
            .into_iter()
            .zip(ranks)
            .enumerate()
            .map(|(i, (title, rank))| Step {
                id: StepId(first + i as i64),
                title,
                rank,
            })
            .collect(),
    )
}

static STEPS: LazyLock<Mutex<Vec<Step>>> = LazyLock::new(|| {
    seed(
        [
            "Collect the requirements",
            "Sketch the screens",
            "Write the entities",
            "Add the actions",
            "Review with the team",
            "Ship it",
        ]
        .into_iter()
        .map(String::from),
    )
});

static BACKLOG: LazyLock<Mutex<Vec<Step>>> =
    LazyLock::new(|| seed((1..=500).map(|n| format!("Backlog item {n}"))));

/// The rows of `store` in the order the list asks for (by rank).
fn rows(store: &Mutex<Vec<Step>>, q: DataQuery) -> Page<Step> {
    let all = store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    q.apply(all)
}

/// Gives a moved step of `store` the rank between its new neighbors.
fn reorder(store: &Mutex<Vec<Step>>, r: ReorderRequest<StepId>) -> Result<()> {
    let mut all = store
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let rank = r.rank(|k| {
        all.iter()
            .find(|s| s.id == *k)
            .map(|s| s.rank.clone())
    })?;
    let step = all
        .iter_mut()
        .find(|s| s.id == r.key)
        .ok_or(Error::NotFound)?;
    step.rank = rank;
    if all
        .iter()
        .any(|s| s.rank.needs_rebalance())
    {
        all.sort_by(|a, b| a.rank.cmp(&b.rank));
        let fresh = zeyn::rank::rebalance(all.len());
        for (s, rank) in all.iter_mut().zip(fresh) {
            s.rank = rank;
        }
    }
    Ok(())
}

/// The sample steps, in the order the list asks for (by rank).
#[action(public)]
pub async fn sample_steps(q: DataQuery) -> Result<Page<Step>> {
    Ok(rows(&STEPS, q))
}

/// Gives a moved step the rank between its new neighbors.
#[action(public)]
pub async fn reorder_steps(r: ReorderRequest<StepId>) -> Result<()> {
    reorder(&STEPS, r)
}

/// The sample backlog of 500 items, by rank.
#[action(public)]
pub async fn sample_backlog(q: DataQuery) -> Result<Page<Step>> {
    Ok(rows(&BACKLOG, q))
}

/// Gives a moved backlog item the rank between its new neighbors.
#[action(public)]
pub async fn reorder_backlog(r: ReorderRequest<StepId>) -> Result<()> {
    reorder(&BACKLOG, r)
}

pub fn sortable(_ctx: &Ctx) -> Node {
    List::of(sample_steps)
        .title("Release plan")
        .item(|s: &Step| ListItem::new(s.title.clone()))
        .sortable(Step::rank, reorder_steps)
        .into()
}

/// The two lists of the connected example: "1" is the sprint, "2" the done list.
static SPRINT: LazyLock<Mutex<Vec<Step>>> = LazyLock::new(|| {
    seed_from(
        201,
        [
            "Fix the login form",
            "Translate the help",
            "Add the export",
            "Check the dark theme",
        ]
        .into_iter()
        .map(String::from),
    )
});

static DONE: LazyLock<Mutex<Vec<Step>>> = LazyLock::new(|| {
    seed_from(
        301,
        ["Set up the project", "Write the entities"]
            .into_iter()
            .map(String::from),
    )
});

/// The store of a connected list by its id.
fn board(id: Option<&str>) -> Result<&'static Mutex<Vec<Step>>> {
    match id {
        Some("1") => Ok(&SPRINT),
        Some("2") => Ok(&DONE),
        _ => Err(Error::NotFound),
    }
}

/// The steps of the sprint, by rank.
#[action(public)]
pub async fn sprint_steps(q: DataQuery) -> Result<Page<Step>> {
    Ok(rows(&SPRINT, q))
}

/// The finished steps, by rank.
#[action(public)]
pub async fn done_steps(q: DataQuery) -> Result<Page<Step>> {
    Ok(rows(&DONE, q))
}

/// Moves one or more steps within a list or from the other list: the rows are looked up in the
/// list they left, the neighbors in the list they land in.
#[action(public)]
pub async fn move_steps(r: ReorderRequest<StepId>) -> Result<()> {
    let to = board(r.list.as_deref())?;
    let from = match r.from.as_deref() {
        Some(id) => board(Some(id))?,
        None => to,
    };
    let ranks = {
        let target = to
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        r.ranks(|k| {
            target
                .iter()
                .find(|s| s.id == *k)
                .map(|s| s.rank.clone())
        })?
    };
    let mut moved = Vec::new();
    {
        let mut source = from
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !r
            .keys()
            .all(|k| source.iter().any(|s| s.id == *k))
        {
            return Err(Error::NotFound);
        }
        for (k, rank) in r.keys().zip(ranks) {
            let i = source
                .iter()
                .position(|s| s.id == *k)
                .ok_or(Error::NotFound)?;
            let mut step = source.remove(i);
            step.rank = rank;
            moved.push(step);
        }
    }
    to.lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(moved);
    Ok(())
}

pub fn connected(_ctx: &Ctx) -> Node {
    let item = |s: &Step| ListItem::new(s.title.clone());
    AutoGrid::new()
        .min(260)
        .add(
            List::of(sprint_steps)
                .title("Sprint")
                .item(item)
                .sortable(Step::rank, move_steps)
                .connect("board", "1")
                .multi_select(),
        )
        .add(
            List::of(done_steps)
                .title("Done")
                .item(item)
                .sortable(Step::rank, move_steps)
                .connect("board", "2")
                .multi_select(),
        )
        .into()
}

pub fn sortable_virtual(_ctx: &Ctx) -> Node {
    List::of(sample_backlog)
        .title("Backlog")
        .item(|s: &Step| ListItem::new(s.title.clone()))
        .paging(Paging::virtual_scroll())
        .sortable(Step::rank, reorder_backlog)
        .into()
}

pub fn overview(_ctx: &Ctx) -> Node {
    List::of(search_people)
        .title("Team")
        .item(|p: &Person| {
            ListItem::new(p.name.clone())
                .image(crate::data::picture(p.id.0))
                .subtitle(p.role.clone())
                .meta(p.city.clone())
                .link(crate::avatar, ())
        })
        .paging(Paging::infinite(4))
        .into()
}

pub fn numbered_pages(ctx: &Ctx) -> Node {
    let now = ctx.now();
    List::of(crate::data::people_uncounted)
        .item(move |p: &Person| {
            ListItem::new(p.name.clone())
                .subtitle(p.role.clone())
                .meta(RelativeTime::new(now.add_ms(-p.id.0 * 3_600_000)))
        })
        .paging(Paging::pages(5))
        .into()
}

pub fn fixed(_ctx: &Ctx) -> Node {
    people()
        .iter()
        .take(3)
        .fold(List::new(), |list, p| {
            list.item(ListItem::new(p.name.clone()).subtitle(p.role.clone()))
        })
        .into()
}

pub fn builder(_ctx: &Ctx) -> Node {
    List::of(search_people)
        .title("Team, built row by row")
        .builder(|p: &Person| {
            Row::new()
                .compact()
                .col(Avatar::new(p.name.clone()))
                .col(Paragraph::new(format!("{}, {}", p.role, p.city)))
        })
        .separator(Paragraph::new("·"))
        .paging(Paging::infinite(4))
        .into()
}

pub fn virtual_rows(_ctx: &Ctx) -> Node {
    List::of(search_products)
        .title("Every product, scrolled virtually")
        .builder(|p: &Product| {
            Paragraph::new(format!("{} · {} · {} in stock", p.code, p.name, p.stock))
        })
        .row_height(40)
        .paging(Paging::virtual_scroll())
        .into()
}

pub fn repeat(_ctx: &Ctx) -> Node {
    Repeat::of(people().iter().take(4), |p| Badge::new(p.name.clone()))
        .separator(Paragraph::new("then"))
        .into()
}

/// What the catalog shows of `List`.
pub const ENTRY: Entry = Entry {
    name: "List",
    docs: "components/content#list",
    api: &["List", "ListItem", "CardGrid", "ReorderRequest", "Repeat"],
    overview: &[ex!(overview)],
    examples: &[
        ex!(numbered_pages),
        ex!(sortable),
        ex!(sortable_virtual),
        ex!(connected),
        ex!(fixed),
        ex!(builder),
        ex!(virtual_rows),
        ex!(repeat),
    ],
};
