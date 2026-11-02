use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::model::{Field, FieldError, FieldMeta, Model, Rule, Valid, ValidationErrors};
use crate::render::{Render, escape};
use crate::screen::{Component, Effect, FormSource, Node, UiCtx};
use crate::time::{Date, DateTime, LocalDateTime, LocalTimeError, Time, TimeZone};
use crate::value::{Currency, FieldKind, FieldType, Money, Value};
use serde_json::{Map, Value as Json, json};
use std::marker::PhantomData;
use std::sync::Arc;

/// Actions that can submit a form of `M`: their input must be `Valid<M>`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot submit a `{M}` form",
    label = "this action's input is not `Valid<{M}>`",
    note = "a form's submit action must take `Valid<{M}>` as its first argument"
)]
pub trait FormAction<M>: Action {}

impl<M: Model, A: Action<Input = Valid<M>>> FormAction<M> for A {}

type ValueFn<M> = Box<dyn Fn(&M) -> Value + Send + Sync>;

struct FormField<M> {
    meta: &'static FieldMeta,
    value: ValueFn<M>,
}

impl<M: Model> FormField<M> {
    fn of<T: FieldType + 'static>(field: Field<M, T>) -> Self {
        Self { meta: field.meta(), value: Box::new(move |m| field.get(m).to_value()) }
    }

    fn at(index: usize) -> Self {
        Self { meta: &M::fields()[index], value: Box::new(move |m| m.value_at(index).unwrap_or(Value::Null)) }
    }
}

enum FormItem<M> {
    Field(FormField<M>),
    Row(Vec<FormField<M>>),
}

/// Fields placed side by side.
pub struct FormRow<M> {
    fields: Vec<FormField<M>>,
}

impl<M: Model> FormRow<M> {
    pub fn field<T: FieldType + 'static>(mut self, field: Field<M, T>) -> Self {
        self.fields.push(FormField::of(field));
        self
    }
}

/// Form that edits a model. Add fields, then call `submit` last.
///
/// Example:
/// ```ignore
/// Form::of(product).field(Product::name).submit("Save", save_product)
/// ```
pub struct Form<M: Model> {
    model: M,
    items: Vec<FormItem<M>>,
}

impl<M: Model> Form<M> {
    /// Form over the fields at these indexes of `M::fields()`. Used for grid row edits.
    pub(crate) fn with_fields(model: M, indexes: &[usize]) -> Self {
        Self { model, items: indexes.iter().map(|i| FormItem::Field(FormField::at(*i))).collect() }
    }

    /// Empty form filled with the given value. Add fields with `field` and `row`.
    pub fn of(model: M) -> Self {
        Self { model, items: Vec::new() }
    }

    /// Form with every field of the model, in declaration order. Keys and versions become hidden
    /// inputs. Useful for simple models; hand written layouts use `of`.
    pub fn auto(model: M) -> Self {
        let items = (0..M::fields().len())
            .filter(|i| !M::fields()[*i].version)
            .map(|i| FormItem::Field(FormField::at(i)))
            .collect();
        Self { model, items }
    }

    pub fn field<T: FieldType + 'static>(mut self, field: Field<M, T>) -> Self {
        self.items.push(FormItem::Field(FormField::of(field)));
        self
    }

    pub fn row(mut self, build: impl FnOnce(FormRow<M>) -> FormRow<M>) -> Self {
        let row = build(FormRow { fields: Vec::new() });
        self.items.push(FormItem::Row(row.fields));
        self
    }

    /// Submit button and action. Does not compile unless the action takes `Valid<M>`.
    pub fn submit<A: FormAction<M>>(self, label: impl Into<Text>, _action: A) -> SubmitForm<M, A> {
        SubmitForm { form: self, label: label.into(), on_success: None, _action: PhantomData }
    }

    fn fields(&self) -> impl Iterator<Item = &FormField<M>> {
        self.items.iter().flat_map(|item| match item {
            FormItem::Field(f) => std::slice::from_ref(f).iter(),
            FormItem::Row(fs) => fs.iter(),
        })
    }

    fn render_fields(&self, out: &mut String, r: &Render) {
        for item in &self.items {
            match item {
                FormItem::Field(f) => render_field(out, r, f, &self.model),
                FormItem::Row(fields) => {
                    out.push_str(r#"<div class="z-form-row">"#);
                    for f in fields {
                        render_field(out, r, f, &self.model);
                    }
                    out.push_str("</div>");
                }
            }
        }
        for (i, meta) in M::fields().iter().enumerate().filter(|(_, m)| m.version) {
            let v = match self.model.value_at(i) {
                Some(Value::Int(n)) => n.to_string(),
                _ => String::new(),
            };
            out.push_str(&format!(r#"<input type="hidden" name="{}" value="{v}" data-zeyn-version>"#, meta.name));
        }
    }

    fn fields_schema(&self) -> Vec<Json> {
        let mut list = Vec::new();
        for (row, item) in self.items.iter().enumerate() {
            let fields: Vec<&FormField<M>> = match item {
                FormItem::Field(f) => vec![f],
                FormItem::Row(fs) => fs.iter().collect(),
            };
            for f in fields {
                list.push(json!({ "row": row, "field": f.meta }));
            }
        }
        list
    }

    /// Applies submitted values to a copy of the loaded model and validates it. All problems are
    /// reported together: values that cannot be read and the model's own rules.
    ///
    /// Only fields on the form that are not keys or read only are taken from the client, plus the
    /// row version. Everything else keeps the loaded value, so a client cannot change fields it was
    /// not shown or switch the record it saves.
    pub fn read(&self, values: &Map<String, Json>, locale: &str, tz: TimeZone) -> std::result::Result<Valid<M>, ValidationErrors> {
        let (model, mut errors) = self.merge(values, locale, tz)?;
        if let Err(rules) = model.validate() {
            let unreadable: Vec<&str> = errors.errors.iter().map(|e| e.field).collect();
            errors.errors.extend(rules.errors.into_iter().filter(|e| !unreadable.contains(&e.field)));
        }
        errors.into_result()?;
        Valid::new(model)
    }

    fn merge(&self, values: &Map<String, Json>, locale: &str, tz: TimeZone) -> std::result::Result<(M, ValidationErrors), ValidationErrors> {
        let mut object = match serde_json::to_value(&self.model) {
            Ok(Json::Object(o)) => o,
            _ => return Err(general_error()),
        };
        let mut errors = ValidationErrors::default();
        let editable = self.fields().map(|f| f.meta).filter(|m| !m.key && !m.readonly && !m.version);
        let versions = M::fields().iter().filter(|m| m.version);
        for meta in editable.chain(versions) {
            let Some(raw) = values.get(meta.name) else { continue };
            let current = object.get(meta.name).cloned().unwrap_or(Json::Null);
            match parse_value(meta, raw, locale, tz, &current) {
                Ok(v) => {
                    object.insert(meta.name.to_string(), v);
                }
                Err(problem) => {
                    let key = match problem {
                        Unreadable::Format => "zeyn.validation.format",
                        Unreadable::SkippedTime => "zeyn.validation.skipped_time",
                    };
                    errors.errors.push(FieldError {
                        field: meta.name,
                        rule: Rule::Format,
                        message: Text::key(key).arg("label", meta.label_text()),
                    })
                }
            }
        }
        let model = serde_json::from_value(Json::Object(object)).map_err(|_| general_error())?;
        Ok((model, errors))
    }
}

fn general_error() -> ValidationErrors {
    let mut e = ValidationErrors::default();
    e.add_general(Text::key("zeyn.error.internal"));
    e
}

/// Why a submitted value could not be read.
enum Unreadable {
    Format,
    SkippedTime,
}

impl From<()> for Unreadable {
    fn from(_: ()) -> Self {
        Unreadable::Format
    }
}

/// Reads one submitted value as the field's type. Text arrives as typed by the user; wall clock
/// date times are read in the user's time zone.
fn parse_value(meta: &FieldMeta, raw: &Json, locale: &str, tz: TimeZone, current: &Json) -> std::result::Result<Json, Unreadable> {
    if let FieldKind::Bool = meta.kind {
        return raw.as_bool().map(Json::Bool).ok_or(Unreadable::Format);
    }
    let text = match raw {
        Json::String(s) => s.clone(),
        Json::Number(n) => n.to_string(),
        Json::Null => String::new(),
        _ => return Err(Unreadable::Format),
    };
    let trimmed = text.trim();
    if trimmed.is_empty() && meta.optional {
        return Ok(Json::Null);
    }
    match meta.kind {
        FieldKind::Text | FieldKind::Custom { .. } => Ok(Json::String(text)),
        FieldKind::Integer | FieldKind::Id => trimmed.parse::<i64>().map(Json::from).map_err(|_| Unreadable::Format),
        FieldKind::Date => Date::parse_local(trimmed, locale).map(|d| Json::String(d.to_string())).ok_or(Unreadable::Format),
        FieldKind::Time => Time::parse(trimmed).map(|t| Json::String(t.to_string())).ok_or(Unreadable::Format),
        FieldKind::LocalDateTime => {
            LocalDateTime::parse(trimmed).map(|l| Json::String(l.to_string())).ok_or(Unreadable::Format)
        }
        FieldKind::DateTime => {
            if let Some(exact) = DateTime::parse(trimmed) {
                return Ok(Json::String(exact.to_string()));
            }
            let local = LocalDateTime::parse(trimmed).ok_or(Unreadable::Format)?;
            match DateTime::from_local(local, tz) {
                Ok(moment) => Ok(Json::String(moment.to_string())),
                Err(LocalTimeError::Skipped) => Err(Unreadable::SkippedTime),
            }
        }
        FieldKind::Money => {
            let currency: Currency = current
                .get("currency")
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or_default();
            let money = Money::parse(trimmed, locale, currency)?;
            serde_json::to_value(money).map_err(|_| Unreadable::Format)
        }
        FieldKind::Choice(options) => {
            options.iter().find(|o| o.value == trimmed).map(|o| Json::String(o.value.to_string())).ok_or(Unreadable::Format)
        }
        FieldKind::Bool => Err(Unreadable::Format),
    }
}

/// A value as an input holds it, in the form `Form::read` reads back: money in the locale's
/// format without the symbol, dates as `YYYY-MM-DD`, moments as local date and time in `tz`,
/// choices by value, booleans as `true` or `false`.
pub(crate) fn edit_value(value: &Value, locale: &str, tz: TimeZone) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Text(s) => s.clone(),
        Value::Money(m) => m.format_plain(locale),
        Value::Date(d) => d.to_string(),
        Value::Time(t) => t.to_string(),
        Value::LocalDateTime(l) => l.to_string(),
        Value::DateTime(d) => d.local(tz).to_string(),
        Value::Choice(c) => c.to_string(),
        Value::Decimal(x) => x.to_string(),
    }
}

/// Writes the input that fits the field kind, with the validation attributes the client mirrors.
fn render_field<M>(out: &mut String, r: &Render, f: &FormField<M>, model: &M) {
    let m = f.meta;
    let scope = r.scope;
    let value = (f.value)(model);
    let id = format!("{scope}-{}", m.name);
    if m.key {
        let v = match &value {
            Value::Int(n) => n.to_string(),
            _ => String::new(),
        };
        out.push_str(&format!(r#"<input type="hidden" name="{}" value="{v}">"#, m.name));
        return;
    }

    let required = m.is_required();
    let mut common = format!(
        r#"id="{id}" name="{}" data-testid="{scope}.field.{}" aria-describedby="{id}-error""#,
        m.name, m.name
    );
    if required {
        common.push_str(r#" required aria-required="true""#);
    }
    if m.readonly {
        common.push_str(" readonly");
    }

    out.push_str(r#"<div class="z-field">"#);
    let star = if required { r#"<span class="z-required" aria-hidden="true">*</span>"# } else { "" };
    let label = format!(r#"<label for="{id}">{}{star}</label>"#, r.label(m));
    let text = |v: &Value| match v {
        Value::Text(s) => escape(s),
        _ => String::new(),
    };

    match m.kind {
        FieldKind::Bool => {
            let checked = if value == Value::Bool(true) { " checked" } else { "" };
            out.push_str(&format!(r#"<input type="checkbox" {common}{checked}>"#));
            out.push_str(&label);
        }
        FieldKind::Choice(options) => {
            out.push_str(&label);
            out.push_str(&format!("<select {common}>"));
            if m.optional {
                out.push_str(r#"<option value=""></option>"#);
            }
            for o in options {
                let selected = if value == Value::Choice(o.value) { " selected" } else { "" };
                out.push_str(&format!(
                    r#"<option value="{}"{selected}>{}</option>"#,
                    o.value,
                    r.text(&Text::key_or(o.key, o.label))
                ));
            }
            out.push_str("</select>");
        }
        FieldKind::Money => {
            let v = match &value {
                Value::Money(money) => money.format_plain(r.locale),
                _ => String::new(),
            };
            out.push_str(&label);
            out.push_str(&format!(r#"<input type="text" inputmode="decimal" class="z-num" {common} value="{v}">"#));
        }
        FieldKind::Integer | FieldKind::Id => {
            let v = match &value {
                Value::Int(n) => n.to_string(),
                _ => String::new(),
            };
            let min = m.min().map(|n| format!(r#" min="{n}""#)).unwrap_or_default();
            let max = m.max().map(|n| format!(r#" max="{n}""#)).unwrap_or_default();
            out.push_str(&label);
            out.push_str(&format!(r#"<input type="number" step="1" class="z-num" {common}{min}{max} value="{v}">"#));
        }
        FieldKind::Text => {
            let max = m.max_len().map(|n| format!(r#" maxlength="{n}""#)).unwrap_or_default();
            let min = m.min_len().map(|n| format!(r#" minlength="{n}""#)).unwrap_or_default();
            let input_type = if m.rules.contains(&Rule::Email) { "email" } else { "text" };
            out.push_str(&label);
            out.push_str(&format!(r#"<input type="{input_type}" {common}{min}{max} value="{}">"#, text(&value)));
        }
        FieldKind::Date | FieldKind::Time | FieldKind::DateTime | FieldKind::LocalDateTime => {
            let (input_type, v) = match &value {
                Value::Date(d) => ("date", d.to_string()),
                Value::Time(t) => ("time", t.to_string()),
                Value::LocalDateTime(l) => ("datetime-local", l.to_string()),
                Value::DateTime(d) => ("datetime-local", d.local(r.tz).to_string()),
                _ => match m.kind {
                    FieldKind::Date => ("date", String::new()),
                    FieldKind::Time => ("time", String::new()),
                    _ => ("datetime-local", String::new()),
                },
            };
            let step = if input_type == "date" { "" } else { r#" step="1""# };
            out.push_str(&label);
            out.push_str(&format!(r#"<input type="{input_type}"{step} {common} value="{v}">"#));
        }
        FieldKind::Custom { editor } => {
            out.push_str(&label);
            out.push_str(&format!(
                r#"<input type="text" {common} data-zeyn-editor="{}" value="{}">"#,
                escape(editor),
                text(&value)
            ));
        }
    }
    out.push_str(&format!(r#"<p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#));
}

type SuccessFn<O> = Arc<dyn Fn(&O, &mut UiCtx) + Send + Sync>;

/// Form with a submit action attached.
pub struct SubmitForm<M: Model, A: Action> {
    form: Form<M>,
    label: Text,
    on_success: Option<SuccessFn<A::Output>>,
    _action: PhantomData<fn() -> A>,
}

impl<M: Model, A: Action> SubmitForm<M, A> {
    /// What happens after a successful save.
    pub fn on_success(mut self, handler: impl Fn(&A::Output, &mut UiCtx) + Send + Sync + 'static) -> Self {
        self.on_success = Some(Arc::new(handler));
        self
    }

    /// Effects produced after a successful save.
    pub fn success_effects(&self, output: &A::Output) -> Vec<Effect> {
        let mut ctx = UiCtx::default();
        if let Some(handler) = &self.on_success {
            handler(output, &mut ctx);
        }
        ctx.into_effects()
    }

    pub fn form(&self) -> &Form<M> {
        &self.form
    }
}

impl<M: Model, A> FormSource for SubmitForm<M, A>
where
    A: Action<Input = Valid<M>>,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn submit(&self, ctx: Ctx, values: Map<String, Json>) -> BoxFuture<Result<Vec<Effect>>> {
        let valid = self.form.read(&values, ctx.locale(), ctx.time_zone());
        let on_success = self.on_success.clone();
        Box::pin(async move {
            let valid = valid.map_err(Error::Validation)?;
            let output = A::call(ctx, valid).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            let mut ui = UiCtx::default();
            if let Some(handler) = on_success {
                handler(&output, &mut ui);
            }
            Ok(ui.into_effects())
        })
    }

    fn tool_schema(&self) -> Json {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for m in self.form.fields().map(|f| f.meta).filter(|m| !m.key && !m.readonly && !m.version) {
            let (kind, hint) = match m.kind {
                FieldKind::Bool => ("boolean", String::new()),
                FieldKind::Integer | FieldKind::Id => ("string", "Whole number.".to_string()),
                FieldKind::Money => ("string", "Amount as typed by a user of the current language, for example 1.234,56 in Turkish.".to_string()),
                FieldKind::Choice(options) => {
                    let values: Vec<&str> = options.iter().map(|o| o.value).collect();
                    ("string", format!("One of: {}.", values.join(", ")))
                }
                FieldKind::Text | FieldKind::Custom { .. } => ("string", String::new()),
                FieldKind::Date => ("string", "Date as YYYY-MM-DD.".to_string()),
                FieldKind::Time => ("string", "Time as HH:MM.".to_string()),
                FieldKind::LocalDateTime => ("string", "Date and time as YYYY-MM-DDTHH:MM, no time zone.".to_string()),
                FieldKind::DateTime => ("string", "Moment in RFC 3339 with offset, or YYYY-MM-DDTHH:MM in the user's time zone.".to_string()),
            };
            properties.insert(m.name.to_string(), json!({ "type": kind, "description": format!("{}. {hint}", m.label).trim().to_string() }));
            if m.is_required() {
                required.push(m.name);
            }
        }
        for m in M::fields().iter().filter(|m| m.version) {
            properties.insert(m.name.to_string(), json!({ "type": "string", "description": "Row version as loaded. Send it back unchanged." }));
        }
        json!({ "type": "object", "properties": properties, "required": required })
    }
}

impl<M: Model> Component for Form<M> {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<form class="z-form" data-testid="{}.form" novalidate>"#, r.scope));
        self.render_fields(out, r);
        out.push_str("</form>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "form", "model": M::NAME, "fields": self.fields_schema(), "submit": null })
    }
}

impl<M: Model, A> Component for SubmitForm<M, A>
where
    A: Action<Input = Valid<M>>,
    A::Output: Outcome,
{
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<form class="z-form" data-testid="{}.form" data-zeyn-submit="{}" novalidate>"#,
            r.scope,
            A::META.name
        ));
        self.form.render_fields(out, r);
        out.push_str(&format!(
            r#"<div class="z-form-actions"><button type="submit" class="z-button z-primary" data-testid="{}.form.submit">{}</button></div></form>"#,
            r.scope,
            r.text(&self.label)
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "form",
            "model": M::NAME,
            "fields": self.form.fields_schema(),
            "submit": { "label": self.label, "action": A::META },
        })
    }

    fn form(&self) -> Option<&dyn FormSource> {
        Some(self)
    }
}

impl<M: Model> From<Form<M>> for Node {
    fn from(f: Form<M>) -> Self {
        Node::new(f)
    }
}

impl<M: Model, A> From<SubmitForm<M, A>> for Node
where
    A: Action<Input = Valid<M>>,
    A::Output: Outcome,
{
    fn from(f: SubmitForm<M, A>) -> Self {
        Node::new(f)
    }
}
