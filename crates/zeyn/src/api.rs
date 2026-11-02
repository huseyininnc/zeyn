//! Actions published as JSON API endpoints, with an OpenAPI description built from the models.

use crate::action::{Action, ActionMeta, DataQuery, Error, Page, Result};
use crate::ctx::{BoxFuture, Ctx, Dep};
use crate::model::{FieldMeta, Model, Valid, ValidationErrors};
use crate::value::FieldKind;
use serde::Serialize;
use serde_json::{Map, Value as Json, json};

/// JSON Schema of an object with the model's fields.
pub fn model_schema<M: Model>() -> Json {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for f in M::fields() {
        properties.insert(f.name.to_string(), field_schema(f));
        if !f.optional {
            required.push(f.name);
        }
    }
    json!({ "type": "object", "title": M::NAME, "properties": properties, "required": required })
}

fn field_schema(f: &FieldMeta) -> Json {
    let mut s = match f.kind {
        FieldKind::Id | FieldKind::Integer => json!({ "type": "integer" }),
        FieldKind::Decimal => {
            let (precision, scale) = f.decimal_limits();
            json!({
                "type": "string",
                "pattern": "^-?\\d+(\\.\\d+)?$",
                "description": format!("Exact decimal number with '.' as the separator, at most {precision} digits and {scale} decimals")
            })
        }
        FieldKind::Text | FieldKind::Custom { .. } => json!({ "type": "string" }),
        FieldKind::Bool => json!({ "type": "boolean" }),
        FieldKind::Date => json!({ "type": "string", "format": "date" }),
        FieldKind::Time => json!({ "type": "string", "pattern": "^\\d{2}:\\d{2}(:\\d{2})?$" }),
        FieldKind::DateTime => json!({ "type": "string", "format": "date-time" }),
        FieldKind::LocalDateTime => {
            json!({ "type": "string", "description": "Wall clock date and time with no time zone, YYYY-MM-DDTHH:MM:SS" })
        }
        FieldKind::Money => json!({
            "type": "object",
            "properties": { "minor": { "type": "integer", "description": "Amount in the smallest unit (cents, kuruş)" }, "currency": { "type": "string", "enum": ["Try", "Usd", "Eur"] } },
            "required": ["minor", "currency"]
        }),
        FieldKind::Choice(options) => {
            json!({ "type": "string", "enum": options.iter().map(|o| o.value).collect::<Vec<_>>() })
        }
    };
    let o = s.as_object_mut().expect("object schema");
    o.insert("title".into(), json!(f.label));
    for r in f.rules {
        match r {
            crate::model::Rule::MaxLen(n) => {
                o.insert("maxLength".into(), json!(n));
            }
            crate::model::Rule::MinLen(n) => {
                o.insert("minLength".into(), json!(n));
            }
            crate::model::Rule::Min(n) if f.kind == FieldKind::Integer => {
                o.insert("minimum".into(), json!(n));
            }
            crate::model::Rule::Max(n) if f.kind == FieldKind::Integer => {
                o.insert("maximum".into(), json!(n));
            }
            crate::model::Rule::Email => {
                o.insert("format".into(), json!("email"));
            }
            _ => {}
        }
    }
    if f.readonly || f.key {
        o.insert("readOnly".into(), json!(true));
    }
    if f.optional {
        s = json!({ "anyOf": [s, { "type": "null" }] });
    }
    s
}

/// Types an API action can take as input.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be the input of an API action",
    note = "API inputs are `Valid<M>`, `DataQuery`, `()`, numbers, strings, booleans, entity ids or `#[params]` types"
)]
pub trait ApiInput: Sized + Send + 'static {
    fn schema() -> Json;

    /// Reads the request body. Validation failures become `Error::Validation`.
    fn from_json(value: Json) -> Result<Self>;

    /// The record this input saves, for the audit log.
    fn record(&self) -> Option<crate::audit::RecordRef> {
        None
    }

    /// Whether an action taking this input only reads. AI tools ask the user before running any
    /// other action.
    const READS: bool = false;
}

fn unreadable(e: serde_json::Error) -> Error {
    let mut errors = ValidationErrors::default();
    errors.add_general(format!("The request body could not be read: {e}"));
    Error::Validation(errors)
}

impl<M: Model> ApiInput for Valid<M> {
    fn schema() -> Json {
        model_schema::<M>()
    }

    fn from_json(value: Json) -> Result<Self> {
        let model: M = serde_json::from_value(value).map_err(unreadable)?;
        Valid::new(model).map_err(Error::Validation)
    }

    fn record(&self) -> Option<crate::audit::RecordRef> {
        Some(crate::audit::RecordRef::of(&**self))
    }
}

macro_rules! serde_input {
    ($($ty:ty => $schema:expr),* $(,)?) => {
        $(impl ApiInput for $ty {
            fn schema() -> Json {
                $schema
            }

            fn from_json(value: Json) -> Result<Self> {
                serde_json::from_value(value).map_err(unreadable)
            }
        })*
    };
}

serde_input! {
    () => json!({ "type": "null" }),
    i64 => json!({ "type": "integer" }),
    i32 => json!({ "type": "integer" }),
    bool => json!({ "type": "boolean" }),
    String => json!({ "type": "string" }),
}

impl ApiInput for DataQuery {
    const READS: bool = true;

    fn schema() -> Json {
        json!({ "type": "object", "description": "Paging, sort and filters. See the runtime protocol." })
    }

    fn from_json(value: Json) -> Result<Self> {
        serde_json::from_value(value).map_err(unreadable)
    }
}

/// Types an API action can return inside `Result`.
pub trait ApiOutput: Serialize + Send + 'static {
    fn schema() -> Json;

    /// Masks masked fields of the serialized output for users who may not reveal them, and
    /// records reveals and sensitive reads for the audit log.
    fn protect(_value: &mut Json, _ctx: &Ctx) {}
}

impl<M: Model> ApiOutput for M {
    fn schema() -> Json {
        model_schema::<M>()
    }

    fn protect(value: &mut Json, ctx: &Ctx) {
        crate::privacy::protect_object::<M>(value, ctx);
    }
}

impl<M: Model> ApiOutput for Page<M> {
    fn schema() -> Json {
        json!({
            "type": "object",
            "properties": {
                "items": { "type": "array", "items": model_schema::<M>() },
                "total": { "type": ["integer", "null"] },
                "next": { "type": ["string", "null"] }
            },
            "required": ["items"]
        })
    }

    fn protect(value: &mut Json, ctx: &Ctx) {
        if let Some(items) = value.get_mut("items").and_then(Json::as_array_mut) {
            items.iter_mut().for_each(|item| crate::privacy::protect_object::<M>(item, ctx));
        }
    }
}

impl<M: Model> ApiOutput for Vec<M> {
    fn schema() -> Json {
        json!({ "type": "array", "items": model_schema::<M>() })
    }

    fn protect(value: &mut Json, ctx: &Ctx) {
        if let Some(items) = value.as_array_mut() {
            items.iter_mut().for_each(|item| crate::privacy::protect_object::<M>(item, ctx));
        }
    }
}

macro_rules! plain_output {
    ($($ty:ty => $schema:expr),* $(,)?) => {
        $(impl ApiOutput for $ty {
            fn schema() -> Json {
                $schema
            }
        })*
    };
}

plain_output! {
    () => json!({ "type": "null" }),
    i64 => json!({ "type": "integer" }),
    bool => json!({ "type": "boolean" }),
    String => json!({ "type": "string" }),
}

/// What an API action returns: `zeyn::Result<T>` with an API friendly `T`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be returned by an API action",
    note = "API actions return `zeyn::Result<T>` where T is a model, `Page<M>`, `Vec<M>`, `()`, a number, a string or a boolean"
)]
pub trait ApiResult: Send + 'static {
    fn schema() -> Json;
    fn into_json(self) -> Result<Json>;

    /// See [`ApiOutput::protect`].
    fn protect(_value: &mut Json, _ctx: &Ctx) {}
}

impl<T: ApiOutput> ApiResult for Result<T> {
    fn schema() -> Json {
        T::schema()
    }

    fn into_json(self) -> Result<Json> {
        let value = self?;
        serde_json::to_value(value).map_err(|e| Error::Internal(format!("response could not be serialized: {e}")))
    }

    fn protect(value: &mut Json, ctx: &Ctx) {
        T::protect(value, ctx);
    }
}

/// An action published as an API endpoint.
#[derive(Clone, Copy)]
pub struct ApiEntry {
    pub meta: ActionMeta,
    pub call: fn(Ctx, Json) -> BoxFuture<Result<Json>>,
    pub input: fn() -> Json,
    pub output: fn() -> Json,
    pub deps: fn() -> Vec<Dep>,
    /// The action only reads (its input is a [`DataQuery`]). AI tools treat every other action as a write.
    pub reads: bool,
}

fn call_json<A>(ctx: Ctx, body: Json) -> BoxFuture<Result<Json>>
where
    A: Action,
    A::Input: ApiInput,
    A::Output: ApiResult,
{
    Box::pin(async move {
        let input = A::Input::from_json(body)?;
        if let Some(record) = input.record() {
            ctx.update_trail(|t| {
                t.record.get_or_insert(record);
            });
        }
        let mut value = A::call(ctx.clone(), input).await?.into_json()?;
        <A::Output as ApiResult>::protect(&mut value, &ctx);
        Ok(value)
    })
}

impl ApiEntry {
    pub fn of<A>() -> Self
    where
        A: Action,
        A::Input: ApiInput,
        A::Output: ApiResult,
    {
        ApiEntry {
            meta: A::META,
            call: call_json::<A>,
            input: A::Input::schema,
            output: A::Output::schema,
            deps: A::deps,
            reads: A::Input::READS,
        }
    }
}
