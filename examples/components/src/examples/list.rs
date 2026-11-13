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

/// Steps with the given titles, ranked in that order.
fn seed(titles: impl Iterator<Item = String>) -> Mutex<Vec<Step>> {
    let titles: Vec<String> = titles.collect();
    let ranks = zeyn::rank::rebalance(titles.len());
    Mutex::new(
        titles
            .into_iter()
            .zip(ranks)
            .enumerate()
            .map(|(i, (title, rank))| Step {
                id: StepId(i as i64 + 1),
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
        ex!(fixed),
        ex!(builder),
        ex!(virtual_rows),
        ex!(repeat),
    ],
};
