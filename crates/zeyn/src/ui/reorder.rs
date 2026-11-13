//! The reorder data source of sortable components: reads the moved row and its new neighbors
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
}

/// Runs a reorder action `A` for one moved row. Keys that are empty, too long, of the wrong
/// type or name the moved row as its own neighbor are refused before the action runs; the
/// action finds the neighbors in its own list ([`ReorderRequest::rank`]).
pub(crate) struct ReorderSource<K, A>(PhantomData<fn() -> (K, A)>);

/// The reorder source of action `A`, for a component's `source("reorder")`.
pub(crate) fn source<A, K>() -> Box<dyn DataSource>
where
    A: Action<Input = ReorderRequest<K>>,
    A::Output: Outcome,
    K: DeserializeOwned + Send + Sync + 'static,
{
    Box::new(ReorderSource::<K, A>(PhantomData))
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
        Box::pin(async move {
            let call: ReorderCall = serde_json::from_value(input).map_err(|_| Error::NotFound)?;
            let own = Some(&call.key);
            if call.after.as_ref() == own
                || call.before.as_ref() == own
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
            };
            let output = A::call(ctx, request).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            Ok(json!({ "moved": true }))
        })
    }
}
