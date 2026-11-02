//! Personal data protection: field masking with audited reveal, and sensitive record reads.
//!
//! Masking happens on the server. A masked value leaves the server only as its masked text, unless
//! the user has the field's unmask permission and asks to see it; that reveal is audited.

use crate::action::{Access, Filter, DataQuery, Page, PageRequest, Result};
use crate::audit::{RecordRef, plain};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::model::{FieldMeta, Model};
use crate::render::{Render, escape};
use crate::value::Value;
use serde::Serialize;
use serde_json::Value as Json;

const DOTS: &str = "••••";

/// How a masked value is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskStyle {
    /// Only the last four characters: `•••• 1234`.
    LastFour,
    /// First letter of the name and the domain: `a••••@example.com`.
    Email,
    /// Nothing at all.
    Full,
}

impl MaskStyle {
    /// Masks a value.
    ///
    /// ```
    /// use zeyn::MaskStyle;
    /// assert_eq!(MaskStyle::LastFour.apply("TR12 3456 7890"), "•••• 7890");
    /// assert_eq!(MaskStyle::Email.apply("ayse@example.com"), "a••••@example.com");
    /// assert_eq!(MaskStyle::Full.apply("secret"), "••••");
    /// assert_eq!(MaskStyle::LastFour.apply("123"), "••••");
    /// ```
    pub fn apply(self, value: &str) -> String {
        let value = value.trim();
        match self {
            MaskStyle::LastFour => {
                let chars: Vec<char> = value.chars().collect();
                if chars.len() <= 4 {
                    return DOTS.into();
                }
                let tail: String = chars[chars.len() - 4..].iter().collect();
                format!("{DOTS} {tail}")
            }
            MaskStyle::Email => match value.split_once('@') {
                Some((local, domain)) if !domain.is_empty() => match local.chars().next() {
                    Some(first) => format!("{first}{DOTS}@{domain}"),
                    None => DOTS.into(),
                },
                _ => DOTS.into(),
            },
            MaskStyle::Full => DOTS.into(),
        }
    }
}

/// Masking of one field, generated from `#[mask(...)]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Masking {
    pub style: MaskStyle,
    /// Permission that lets a user reveal the value, `<model>.<field>.unmask` unless set.
    pub permission: &'static str,
}

/// Whether the identity behind `ctx` may see the raw value of a masked field. Never for calls
/// made by AI agents (MCP, WebMCP, the assistant): masked values do not go to models, whatever
/// the user may see.
pub fn can_unmask(ctx: Option<&Ctx>, meta: &FieldMeta) -> bool {
    match meta.mask {
        None => true,
        Some(m) => ctx.is_some_and(|c| !c.via_agent() && c.identity().can(&Access::Permission(m.permission))),
    }
}

/// The text a masked field is shown with. Unmasked fields come back unchanged.
pub fn masked_text(meta: &FieldMeta, value: &str) -> String {
    match meta.mask {
        Some(m) => m.style.apply(value),
        None => value.to_string(),
    }
}

/// A revealed value and the record it belongs to.
pub struct Revealed {
    pub meta: &'static FieldMeta,
    pub value: String,
    pub record: RecordRef,
}

/// The raw value of a masked field of `model`, for an audited reveal.
pub fn reveal_in<M: Model>(model: &M, field: &str) -> Option<Revealed> {
    let (i, meta) = M::fields().iter().enumerate().find(|(_, m)| m.name == field && m.mask.is_some())?;
    let value = model.value_at(i).and_then(|v| plain(&v)).unwrap_or_default();
    Some(Revealed { meta, value, record: RecordRef::of(model) })
}

/// Finds a row by key through a grid's data action and reveals one of its masked fields. The
/// action is asked for rows whose key contains `key` and the exact match is taken.
pub(crate) fn grid_reveal<E: Model>(
    fetch: fn(Ctx, DataQuery) -> BoxFuture<Result<Page<E>>>,
    ctx: Ctx,
    key: String,
    field: String,
) -> BoxFuture<Result<Option<Revealed>>> {
    Box::pin(async move {
        let Some(key_meta) = E::fields().iter().find(|m| m.key) else { return Ok(None) };
        let query = DataQuery {
            page: PageRequest::Offset { offset: 0, limit: crate::action::MAX_PAGE_SIZE },
            filters: vec![Filter::any_of(key_meta.name, [key.clone()])],
            ..DataQuery::default()
        };
        let page = fetch(ctx, query).await?;
        Ok(page
            .items
            .iter()
            .find(|row| RecordRef::of(*row).id.as_deref() == Some(key.as_str()))
            .and_then(|row| reveal_in(row, &field)))
    })
}

/// Removes filters and sorts on masked fields the user may not reveal, so a value cannot be
/// guessed by searching for it.
pub(crate) fn strip_masked(query: &mut DataQuery, fields: &[FieldMeta], ctx: &Ctx) {
    let hidden = |name: &str| fields.iter().any(|m| m.name == name && !can_unmask(Some(ctx), m));
    query.filters.retain(|f| !hidden(&f.field));
    query.sort.retain(|s| !hidden(&s.field));
}

fn reveal_button(r: &Render, meta: &FieldMeta, key: Option<&str>) -> String {
    if !can_unmask(r.ctx, meta) {
        return String::new();
    }
    let key = key.map(|k| format!(r#" data-zeyn-key="{}""#, escape(k))).unwrap_or_default();
    let label = r.text(&Text::key("zeyn.privacy.reveal").arg("label", meta.label_text()));
    format!(
        r#"<button type="button" class="z-reveal" data-zeyn-reveal="{}"{key} aria-label="{label}" title="{label}"><span class="z-icon z-icon-eye" aria-hidden="true"></span></button>"#,
        meta.name
    )
}

/// DataGrid cell of a masked field: the masked text and, for users who may, a reveal button.
pub(crate) fn cell<E: Model>(r: &Render, meta: &FieldMeta, value: &Value, row: &E) -> String {
    let text = plain(value).map(|v| masked_text(meta, &v)).unwrap_or_default();
    let key = RecordRef::of(row).id;
    format!(r#"<span data-zeyn-masked>{}</span>{}"#, escape(&text), reveal_button(r, meta, key.as_deref()))
}

/// Form field of a masked field: read only masked text and, for users who may, a reveal button.
/// The input is read only, so the runtime does not send it back and the stored value is kept.
pub(crate) fn form_field(out: &mut String, r: &Render, meta: &FieldMeta, value: &Value) {
    let id = format!("{}-{}", r.scope, meta.name);
    let text = plain(value).map(|v| masked_text(meta, &v)).unwrap_or_default();
    out.push_str(&format!(
        r#"<div class="z-field"><label for="{id}">{}</label><div class="z-masked-field"><input type="text" id="{id}" name="{}" data-testid="{}.field.{}" aria-describedby="{id}-error" readonly data-zeyn-masked value="{}">{}</div><p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#,
        r.label(meta),
        meta.name,
        r.scope,
        meta.name,
        escape(&text),
        reveal_button(r, meta, None),
    ));
}

fn key_of<M: Model>(object: &serde_json::Map<String, Json>) -> Option<String> {
    M::fields()
        .iter()
        .find(|m| m.key)
        .and_then(|m| object.get(m.name))
        .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
}

/// Masks the masked fields of one serialized model in place. Users with the unmask permission
/// get raw values; each field they receive is recorded as revealed.
pub(crate) fn protect_object<M: Model>(value: &mut Json, ctx: &Ctx) {
    let Some(object) = value.as_object_mut() else { return };
    if M::SENSITIVE {
        ctx.note_read(RecordRef { model: M::NAME, id: key_of::<M>(object) });
    }
    let mut revealed = Vec::new();
    for meta in M::fields().iter().filter(|m| m.mask.is_some()) {
        let Some(Json::String(text)) = object.get(meta.name) else { continue };
        if can_unmask(Some(ctx), meta) {
            revealed.push(meta.name);
        } else {
            let masked = masked_text(meta, text);
            object.insert(meta.name.to_string(), Json::String(masked));
        }
    }
    if !revealed.is_empty() {
        let id = key_of::<M>(object);
        ctx.update_trail(|t| {
            for field in revealed {
                t.revealed.push(crate::audit::FieldRef { model: M::NAME, id: id.clone(), field });
            }
        });
    }
}
