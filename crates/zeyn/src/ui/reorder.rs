//! The reorder data source of sortable components: reads the moved rows and their new neighbors
//! from the browser and runs the app's reorder action with a [`ReorderRequest`].

use super::DataSource;
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::rank::ReorderRequest;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};
use std::marker::PhantomData;

/// Longest key the browser may send for a row or a neighbor, in bytes.
const MAX_KEY: usize = 200;

/// What the browser sends: row keys as the text of their `data-zeyn-key`.
#[derive(serde::Deserialize)]
struct ReorderCall {
    key: String,
    #[serde(default)]
    after: Option<String>,
    #[serde(default)]
    before: Option<String>,
    #[serde(default)]
    rest: Vec<String>,
}

/// How a sortable list takes moves: its group and list id when it is connected to other lists,
/// and whether several selected rows move at once.
#[derive(Clone, Default)]
pub(crate) struct Setup {
    pub(crate) link: Option<(&'static str, String)>,
    pub(crate) multi: bool,
}

/// The sortable part of a component: the rank field, the reorder source and how it was set up.
pub(crate) struct Sortable {
    pub(crate) rank: &'static str,
    pub(crate) source: Box<dyn DataSource>,
    make: Make,
    setup: Setup,
}

impl Sortable {
    /// Sorting by the field `rank` with the source `make` builds.
    pub(crate) fn new(rank: &'static str, make: Make) -> Self {
        Sortable {
            rank,
            source: make(Setup::default()),
            make,
            setup: Setup::default(),
        }
    }

    /// Changes the setup and builds the source again.
    pub(crate) fn with(mut self, change: impl FnOnce(&mut Setup)) -> Self {
        change(&mut self.setup);
        self.source = (self.make)(self.setup.clone());
        self
    }

    /// The attributes of the list's section that tell `sortable.js` about groups and selection.
    pub(crate) fn attrs(&self) -> String {
        let mut out = String::new();
        if let Some((group, id)) = &self.setup.link {
            out.push_str(&format!(
                r#" data-zeyn-sort-group="{}" data-zeyn-sort-list="{}""#,
                crate::render::escape(group),
                crate::render::escape(id)
            ));
        }
        if self.setup.multi {
            out.push_str(" data-zeyn-sort-multi");
        }
        out
    }
}

/// Runs a reorder action `A` for moved rows. Keys that are empty, too long, of the wrong type,
/// repeated, more than [`ReorderRequest::MAX_ROWS`], or that name a moved row as a neighbor are
/// refused before the action runs; the action finds the neighbors in its own list
/// ([`ReorderRequest::rank`]).
pub(crate) struct ReorderSource<K, A> {
    setup: Setup,
    types: PhantomData<fn() -> (K, A)>,
}

/// Builds the reorder source of a component for its `source("reorder")`.
pub(crate) type Make = fn(Setup) -> Box<dyn DataSource>;

/// The reorder source of action `A`.
pub(crate) fn source<A, K>(setup: Setup) -> Box<dyn DataSource>
where
    A: Action<Input = ReorderRequest<K>>,
    A::Output: Outcome,
    K: DeserializeOwned + Send + Sync + 'static,
{
    Box::new(ReorderSource::<K, A> {
        setup,
        types: PhantomData,
    })
}

/// A row key from its text: as a JSON string, or as the JSON it holds (a number).
fn key<K: DeserializeOwned>(text: &str) -> Result<K> {
    if text.is_empty() || text.len() > MAX_KEY {
        return Err(Error::NotFound);
    }
    serde_json::from_value(Json::String(text.to_string()))
        .or_else(|_| serde_json::from_str(text))
        .map_err(|_| Error::NotFound)
}

impl<K, A> ReorderSource<K, A>
where
    A: Action<Input = ReorderRequest<K>>,
    A::Output: Outcome,
    K: DeserializeOwned + Send + Sync + 'static,
{
    /// Checks the call and runs the action.
    fn run(&self, ctx: Ctx, input: Json, from: Option<String>) -> BoxFuture<Result<Json>> {
        let list = self
            .setup
            .link
            .as_ref()
            .map(|(_, id)| id.clone());
        let multi = self.setup.multi;
        Box::pin(async move {
            let call: ReorderCall = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            if (!multi && !call.rest.is_empty()) || call.rest.len() >= ReorderRequest::<K>::MAX_ROWS
            {
                return Err(Error::NotFound);
            }
            let moved: Vec<&String> = std::iter::once(&call.key)
                .chain(&call.rest)
                .collect();
            let repeated = moved
                .iter()
                .enumerate()
                .any(|(i, k)| moved[..i].contains(k));
            let neighbor = |n: &Option<String>| {
                n.as_ref()
                    .is_some_and(|n| moved.contains(&n))
            };
            if repeated
                || neighbor(&call.after)
                || neighbor(&call.before)
                || (call.after.is_some() && call.after == call.before)
            {
                return Err(Error::NotFound);
            }
            let request = ReorderRequest {
                key: key::<K>(&call.key)?,
                after: call
                    .after
                    .as_deref()
                    .map(key::<K>)
                    .transpose()?,
                before: call
                    .before
                    .as_deref()
                    .map(key::<K>)
                    .transpose()?,
                rest: call
                    .rest
                    .iter()
                    .map(|k| key::<K>(k))
                    .collect::<Result<_>>()?,
                list,
                from,
            };
            let output = A::call(ctx, request).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            Ok(json!({ "moved": true }))
        })
    }
}

impl<K, A> DataSource for ReorderSource<K, A>
where
    A: Action<Input = ReorderRequest<K>>,
    A::Output: Outcome,
    K: DeserializeOwned + Send + Sync + 'static,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        self.run(ctx, input, None)
    }

    fn link(&self) -> Option<(&str, &str)> {
        self.setup
            .link
            .as_ref()
            .map(|(group, id)| (*group, id.as_str()))
    }

    fn fetch_from(&self, ctx: Ctx, input: Json, from: String) -> BoxFuture<Result<Json>> {
        self.run(ctx, input, Some(from))
    }
}
