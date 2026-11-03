//! Master detail lines: an editable table for a `Vec<LineModel>` field, such as order lines.
//!
//! Cell inputs are named `{field}.{row}.{column}`, so the form still submits plain values and
//! existing lines can be edited without JavaScript. With JavaScript, lines can be added, removed
//! and moved, totals update while typing and errors appear in their cells. The server rebuilds
//! the lines from the submitted cells, starting each one from the loaded line it came from, so a
//! client can neither change columns it was not shown nor invent keys. Every line is validated
//! with the line model's rules when the parent becomes `Valid<M>`.

use crate::form::{Reading, Unreadable, parse_value};
use crate::i18n::Text;
use crate::model::{Field, FieldError, FieldMeta, Model, Rule, ValidationErrors};
use crate::render::{Render, escape};
use crate::value::{FieldKind, FieldType, Money, Value};
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;
use std::marker::PhantomData;

/// Most lines one form may submit.
pub const MAX_LINES: usize = 500;

/// Lines of a parent model, edited as a table. A line model is any `#[model]` or `#[entity]`.
impl<L: Model> crate::value::VecItem for L {
    const VEC_KIND: FieldKind = FieldKind::Custom { editor: "lines" };

    fn __vec_value(items: &[Self]) -> Value {
        Value::Int(items.len() as i64)
    }

    fn __vec_validate(items: &[Self], meta: &FieldMeta, errors: &mut ValidationErrors) {
        for (row, line) in items.iter().enumerate() {
            let Err(problems) = line.validate() else {
                continue;
            };
            for e in problems.errors {
                errors.errors.push(FieldError {
                    field: meta.name,
                    rule: Rule::Line {
                        row: row as u32,
                        field: e.field,
                    },
                    message: Text::key("zeyn.validation.line")
                        .arg("n", (row + 1).to_string())
                        .arg("message", e.message),
                });
            }
        }
    }
}

/// Columns of a lines table. Built in `Form::lines`.
///
/// ```ignore
/// Form::of(order).lines(Order::lines, |l| {
///     l.column(OrderLine::product).column(OrderLine::quantity).column(OrderLine::unit_price)
///         .total(OrderLine::quantity, OrderLine::unit_price)
/// })
/// ```
pub struct Lines<L: Model> {
    columns: Vec<usize>,
    total: Option<(usize, usize)>,
    _line: PhantomData<fn() -> L>,
}

impl<L: Model> Lines<L> {
    pub(crate) fn new() -> Self {
        Lines {
            columns: Vec::new(),
            total: None,
            _line: PhantomData,
        }
    }

    fn index<T>(field: Field<L, T>) -> usize {
        L::fields()
            .iter()
            .position(|m| m.name == field.meta().name)
            .unwrap_or(0)
    }

    pub fn column<T: FieldType + 'static>(mut self, field: Field<L, T>) -> Self {
        self.columns.push(Self::index(field));
        self
    }

    /// A line total column worked out as `quantity` times `price`, with a grand total in the
    /// footer. `quantity` is a whole number field and `price` a money field.
    pub fn total<
        Q: FieldType + crate::value::SupportsSum,
        P: FieldType + crate::value::SupportsSum,
    >(
        mut self,
        quantity: Field<L, Q>,
        price: Field<L, P>,
    ) -> Self {
        self.total = Some((Self::index(quantity), Self::index(price)));
        self
    }

    /// Line total in minor units of `line`, when it has a total column.
    fn line_total(&self, line: &L) -> Option<Money> {
        let (q, p) = self.total?;
        let qty = match line.value_at(q)? {
            Value::Int(n) => n,
            _ => 0,
        };
        match line.value_at(p)? {
            Value::Money(m) => Some(Money::new(m.minor.saturating_mul(qty), m.currency)),
            Value::Int(n) => Some(Money::new(
                n.saturating_mul(qty)
                    .saturating_mul(100),
                Default::default(),
            )),
            _ => None,
        }
    }
}

/// A lines table as the form stores it.
pub(crate) trait LinesEditor: Send + Sync {
    fn render(&self, out: &mut String, r: &Render, meta: &FieldMeta, current: &Json);

    /// Rebuilds the lines from the submitted cells. `None` when the form did not include them.
    fn read(
        &self,
        values: &Map<String, Json>,
        meta: &FieldMeta,
        reading: &Reading,
        current: &Json,
    ) -> Option<(Json, Vec<FieldError>)>;

    fn schema(&self) -> Json;
}

impl<L: Model> LinesEditor for Lines<L> {
    fn render(&self, out: &mut String, r: &Render, meta: &FieldMeta, current: &Json) {
        let lines: Vec<L> = serde_json::from_value(current.clone()).unwrap_or_default();
        let name = meta.name;
        let id = format!("{}-{name}", r.scope);
        let t = |k: &str| r.text(&Text::key(format!("zeyn.ui.lines.{k}")));
        let f = crate::i18n::NumberFormat::for_locale(r.locale);
        let star = if meta.is_required() {
            r#"<span class="z-required" aria-hidden="true">*</span>"#
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<div class="z-lines" data-zeyn-lines="{name}" data-decimal="{}" data-group="{}" data-text-row="{}" data-text-remove="{}" data-text-up="{}" data-text-down="{}"><p class="z-lines-label" id="{id}-label">{}{star}</p><input type="text" hidden name="{name}" id="{id}" value="rows"><div class="z-lines-scroll"><table class="z-lines-table" aria-labelledby="{id}-label"><thead><tr><th scope="col" class="z-lines-n"><span class="z-sr-only">{}</span>#</th>"#,
            escape(&f.decimal.to_string()),
            escape(&f.group.to_string()),
            t("row"),
            t("remove"),
            t("up"),
            t("down"),
            r.label(meta),
            t("row").replace("{n}", ""),
        ));
        for &c in &self.columns {
            let m = &L::fields()[c];
            let numeric = matches!(m.kind, FieldKind::Integer | FieldKind::Money);
            out.push_str(&format!(
                r#"<th scope="col"{}>{}</th>"#,
                if numeric { r#" class="z-num""# } else { "" },
                r.label(m)
            ));
        }
        if self.total.is_some() {
            out.push_str(&format!(
                r#"<th scope="col" class="z-num">{}</th>"#,
                t("total")
            ));
        }
        out.push_str(&format!(
            r#"<th scope="col" class="z-lines-actions"><span class="z-sr-only">{}</span></th></tr></thead><tbody>"#,
            t("actions")
        ));
        let mut sum: Option<Money> = None;
        for (i, line) in lines.iter().enumerate() {
            self.row(out, r, name, &id, i, Some(i), Some(line));
            if let Some(total) = self.line_total(line) {
                let s = sum.get_or_insert(Money::new(0, total.currency));
                s.minor = s.minor.saturating_add(total.minor);
            }
        }
        out.push_str("</tbody>");
        let span = self.columns.len() + 1;
        if self.total.is_some() {
            out.push_str(&format!(
                r#"<tfoot><tr><th scope="row" colspan="{span}">{}</th><td class="z-num" data-zeyn-lines-sum aria-live="polite">{}</td><td></td></tr></tfoot>"#,
                t("total"),
                escape(&sum.unwrap_or_default().format_plain(r.locale))
            ));
        }
        out.push_str("</table></div><template data-zeyn-line-template>");
        self.row(out, r, name, &id, 0, None, None);
        out.push_str(&format!(
            r#"</template><p class="z-lines-empty" data-zeyn-lines-empty{}>{}</p><button type="button" class="z-button" data-zeyn-line-add data-testid="{}.lines.{name}.add"><span class="z-icon z-icon-plus" aria-hidden="true"></span>{}</button></div>"#,
            if lines.is_empty() { "" } else { " hidden" },
            t("empty"),
            r.scope,
            t("add")
        ));
    }

    fn read(
        &self,
        values: &Map<String, Json>,
        meta: &FieldMeta,
        reading: &Reading,
        current: &Json,
    ) -> Option<(Json, Vec<FieldError>)> {
        values.get(meta.name)?;
        let prefix = format!("{}.", meta.name);
        let mut rows: BTreeMap<usize, Map<String, Json>> = BTreeMap::new();
        for (key, value) in values {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            let Some((row, column)) = rest.split_once('.') else {
                continue;
            };
            let Ok(row) = row.parse::<usize>() else {
                continue;
            };
            if row >= MAX_LINES * 4 {
                continue;
            }
            rows.entry(row)
                .or_default()
                .insert(column.to_string(), value.clone());
            if rows.len() > MAX_LINES {
                return Some((Json::Array(Vec::new()), vec![too_many(meta)]));
            }
        }
        let loaded = current
            .as_array()
            .cloned()
            .unwrap_or_default();
        let blank = serde_json::to_value(L::default()).unwrap_or(Json::Null);
        let mut used = vec![false; loaded.len()];
        let mut out = Vec::new();
        let mut errors = Vec::new();
        for (n, cells) in rows.values().enumerate() {
            let origin = cells
                .get("_i")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|i| *i < loaded.len() && !used[*i]);
            if let Some(i) = origin {
                used[i] = true;
            }
            let mut line = match origin
                .map(|i| loaded[i].clone())
                .unwrap_or_else(|| blank.clone())
            {
                Json::Object(o) => o,
                _ => Map::new(),
            };
            for &c in &self.columns {
                let m = &L::fields()[c];
                if m.key || m.readonly || m.version {
                    continue;
                }
                let Some(raw) = cells.get(m.name) else {
                    continue;
                };
                let base = line
                    .get(m.name)
                    .cloned()
                    .unwrap_or(Json::Null);
                match parse_value(m, raw, reading, &base) {
                    Ok(v) => {
                        line.insert(m.name.to_string(), v);
                    }
                    Err(problem) => {
                        let key = match problem {
                            Unreadable::Format => "zeyn.validation.format",
                            Unreadable::SkippedTime => "zeyn.validation.skipped_time",
                        };
                        errors.push(FieldError {
                            field: meta.name,
                            rule: Rule::Line {
                                row: n as u32,
                                field: m.name,
                            },
                            message: Text::key("zeyn.validation.line")
                                .arg("n", (n + 1).to_string())
                                .arg("message", Text::key(key).arg("label", m.label_text())),
                        });
                    }
                }
            }
            out.push(Json::Object(line));
        }
        Some((Json::Array(out), errors))
    }

    fn schema(&self) -> Json {
        let columns: Vec<&FieldMeta> = self
            .columns
            .iter()
            .map(|c| &L::fields()[*c])
            .collect();
        serde_json::json!({ "type": "lines", "model": L::NAME, "columns": columns, "total": self.total.map(|(q, p)| [L::fields()[q].name, L::fields()[p].name]) })
    }
}

fn too_many(meta: &FieldMeta) -> FieldError {
    FieldError {
        field: meta.name,
        rule: Rule::MaxLen(MAX_LINES),
        message: Text::key("zeyn.validation.max_len")
            .arg("label", meta.label_text())
            .arg("n", MAX_LINES.to_string()),
    }
}

impl<L: Model> Lines<L> {
    /// One table row. `origin` is the index of the loaded line it shows; `None` for new lines
    /// and for the template row.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        out: &mut String,
        r: &Render,
        name: &str,
        id: &str,
        i: usize,
        origin: Option<usize>,
        line: Option<&L>,
    ) {
        let n = i + 1;
        let row_label = r.i18n.text(
            r.locale,
            &Text::key("zeyn.ui.lines.row").arg("n", n.to_string()),
        );
        out.push_str(&format!(
            r#"<tr data-zeyn-line><th scope="row" class="z-lines-n">{n}<input type="text" hidden name="{name}.{i}._i" value="{}"></th>"#,
            origin.map(|o| o.to_string()).unwrap_or_default()
        ));
        for &c in &self.columns {
            let m = &L::fields()[c];
            let value = line
                .and_then(|l| l.value_at(c))
                .unwrap_or(Value::Null);
            let cell_id = format!("{id}-{i}-{}", m.name);
            let label = format!("{}, {}", r.label(m), escape(&row_label));
            let readonly = if m.key || m.readonly { " readonly" } else { "" };
            let (class, mode, text) = match (&value, m.kind) {
                (Value::Money(money), _) => (
                    " z-num",
                    r#" inputmode="decimal""#,
                    money.format_plain(r.locale),
                ),
                (_, FieldKind::Money) => (" z-num", r#" inputmode="decimal""#, String::new()),
                (Value::Int(v), _) => (" z-num", r#" inputmode="numeric""#, v.to_string()),
                (_, FieldKind::Integer | FieldKind::Id) => {
                    (" z-num", r#" inputmode="numeric""#, String::new())
                }
                (Value::Text(s), _) => ("", "", s.clone()),
                (Value::Choice(v), _) => ("", "", (*v).to_string()),
                (Value::Null, _) => ("", "", String::new()),
                (other, _) => ("", "", super::input_text(other, r)),
            };
            let role = match m.kind {
                FieldKind::Integer => " data-zeyn-qty",
                FieldKind::Money => " data-zeyn-price",
                _ => "",
            };
            let required = if m.is_required() {
                r#" aria-required="true""#
            } else {
                ""
            };
            out.push_str(&format!(
                r#"<td><input type="text" class="z-cell{class}" id="{cell_id}" name="{name}.{i}.{}" data-col="{}" aria-label="{label}" aria-describedby="{cell_id}-error"{mode}{role}{required}{readonly} autocomplete="off" value="{}"><span class="z-cell-error" id="{cell_id}-error"></span></td>"#,
                m.name,
                m.name,
                escape(&text)
            ));
        }
        if self.total.is_some() {
            let total = line
                .and_then(|l| self.line_total(l))
                .map(|m| m.format_plain(r.locale))
                .unwrap_or_default();
            out.push_str(&format!(
                r#"<td class="z-num z-lines-total" data-zeyn-line-total>{}</td>"#,
                escape(&total)
            ));
        }
        let t = |k: &str| {
            escape(&r.i18n.text(
                r.locale,
                &Text::key(format!("zeyn.ui.lines.{k}")).arg("n", n.to_string()),
            ))
        };
        out.push_str(&format!(
            r#"<td class="z-lines-actions"><button type="button" class="z-icon-button" data-zeyn-line-move="-1" aria-label="{}"><span aria-hidden="true">&#8593;</span></button><button type="button" class="z-icon-button" data-zeyn-line-move="1" aria-label="{}"><span aria-hidden="true">&#8595;</span></button><button type="button" class="z-icon-button z-danger" data-zeyn-line-remove aria-label="{}"><span class="z-icon z-icon-delete" aria-hidden="true"></span></button></td></tr>"#,
            t("up"),
            t("down"),
            t("remove")
        ));
    }
}
