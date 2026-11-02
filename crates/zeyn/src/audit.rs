//! Field level data audit: which record a call saved, which fields changed and how.
//!
//! The server collects this in a [`Trail`] on the request context while a call runs, and
//! `AuditLog` writes it with the call's audit record. Old values always come from the record the
//! server loaded, never from the client.

use crate::model::Model;
use crate::value::Value;
use serde::Serialize;

/// One changed field. Values are plain strings; masked fields stay masked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FieldChange {
    pub field: &'static str,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// A record named by its model and key. `id` is `None` for models without a key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecordRef {
    pub model: &'static str,
    pub id: Option<String>,
}

impl RecordRef {
    /// The record `value` stands for.
    pub fn of<M: Model>(value: &M) -> Self {
        let id = M::fields().iter().position(|f| f.key).and_then(|i| value.value_at(i)).and_then(|v| plain(&v));
        RecordRef { model: M::NAME, id }
    }
}

/// What a call did to data. Collected on the request context; read by audit interceptors.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Trail {
    /// The record the call saved.
    pub record: Option<RecordRef>,
    /// Fields that changed, in declaration order.
    pub changes: Vec<FieldChange>,
}

/// A value as a plain, language independent string. `None` for an empty value.
///
/// ```
/// use zeyn::{Money, Value};
/// assert_eq!(zeyn::audit::plain(&Value::Money(Money::tl(123456))).as_deref(), Some("1234.56 TRY"));
/// assert_eq!(zeyn::audit::plain(&Value::Null), None);
/// ```
pub fn plain(value: &Value) -> Option<String> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Text(s) => s.clone(),
        Value::Money(m) => {
            let sign = if m.minor < 0 { "-" } else { "" };
            let abs = m.minor.unsigned_abs();
            format!("{sign}{}.{:02} {}", abs / 100, abs % 100, format!("{:?}", m.currency).to_uppercase())
        }
        Value::Date(d) => d.to_string(),
        Value::Time(t) => t.to_string(),
        Value::DateTime(d) => d.to_string(),
        Value::LocalDateTime(l) => l.to_string(),
        Value::Choice(c) => c.to_string(),
    })
}

/// Fields whose value differs between the loaded record and the saved one. Row versions are left
/// out.
pub fn diff<M: Model>(old: &M, new: &M) -> Vec<FieldChange> {
    M::fields()
        .iter()
        .enumerate()
        .filter(|(_, meta)| !meta.version)
        .filter_map(|(i, meta)| {
            let (a, b) = (old.value_at(i).unwrap_or(Value::Null), new.value_at(i).unwrap_or(Value::Null));
            if a == b {
                return None;
            }
            Some(FieldChange { field: meta.name, old: plain(&a), new: plain(&b) })
        })
        .collect()
}
