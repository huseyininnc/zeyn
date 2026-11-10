//! Field specs and runtime form schemas.
//!
//! A [`FieldSpec`] describes one form field: its name, label, kind, rules, help and default.
//! Model forms build one for each field they show (from the field's `FieldMeta`), and a
//! [`FormSchema`] holds specs made at run time, for objects whose properties are only known
//! then, such as the shapes of a designer. Both are drawn by [`render_spec`], so a field looks
//! and behaves the same wherever it comes from. [`SchemaForm`] draws a schema with its values on
//! the server, and [`super::PropertyInspector::island`] draws schemas an island sends in the
//! browser from templates rendered here.
//!
//! ```
//! use zeyn::ui::{FieldSpec, FormSchema, LengthUnit};
//! let schema = FormSchema::new()
//!     .section("Position")
//!     .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
//!     .section("Look")
//!     .field(FieldSpec::color("fill", "Fill", &["#1E88E5"]).required());
//! assert_eq!(schema.len(), 2);
//! ```

use super::{Control, LengthUnit, Slot, is_hex_color};
use crate::i18n::Text;
use crate::model::{FieldMeta, Rule, Rules};
use crate::render::Render;
use crate::screen::{Component, Node};
use crate::value::{FieldKind, Value};
use serde::Serialize;
use serde_json::{Map, Value as Json, json};
use std::borrow::Cow;
use std::fmt::Write as _;

/// Most fields of a [`FormSchema`]; the inspector turns away a larger schema.
pub const MAX_SCHEMA_FIELDS: usize = 200;
/// Most characters of a field name.
pub const MAX_FIELD_NAME: usize = 64;
/// Most characters of a title, label, help text, option label or text value of a schema.
pub const MAX_SCHEMA_TEXT: usize = 1000;
/// Most options of a choice field.
pub const MAX_SCHEMA_OPTIONS: usize = 200;

/// The kind of a runtime field, as a schema names it (`"kind": "length"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    Text,
    Multiline,
    Integer,
    Decimal,
    /// A [`Control::Length`], stored in pixels.
    Length,
    /// A [`Control::Color`], `#RRGGBB` or `#RRGGBBAA`.
    Color,
    Boolean,
    Choice,
    /// An expression, edited in the code editor ([`Control::Code`] with
    /// [`super::CodeLang::Expression`]).
    Expression,
}

impl SpecKind {
    /// Every kind, in the order the inspector keeps its templates.
    pub const ALL: [SpecKind; 9] = [
        SpecKind::Text,
        SpecKind::Multiline,
        SpecKind::Integer,
        SpecKind::Decimal,
        SpecKind::Length,
        SpecKind::Color,
        SpecKind::Boolean,
        SpecKind::Choice,
        SpecKind::Expression,
    ];

    /// The kind's name in a schema: `text`, `length`, `choice`.
    pub fn code(self) -> &'static str {
        match self {
            SpecKind::Text => "text",
            SpecKind::Multiline => "multiline",
            SpecKind::Integer => "integer",
            SpecKind::Decimal => "decimal",
            SpecKind::Length => "length",
            SpecKind::Color => "color",
            SpecKind::Boolean => "boolean",
            SpecKind::Choice => "choice",
            SpecKind::Expression => "expression",
        }
    }
}

/// One form field: what a control needs to draw it and what a value must be to be taken.
///
/// ```
/// use zeyn::ui::FieldSpec;
/// let name = FieldSpec::text("name", "Name").required().max_len(40);
/// assert!(name.check(&serde_json::json!("Title")).is_ok());
/// assert!(name.check(&serde_json::json!("")).is_err());
/// ```
pub struct FieldSpec {
    pub(crate) name: Cow<'static, str>,
    pub(crate) label: Text,
    pub(crate) kind: FieldKind,
    pub(crate) control: Option<Control>,
    pub(crate) rules: Cow<'static, [Rule]>,
    pub(crate) readonly: bool,
    pub(crate) optional: bool,
    options: Vec<(String, Text)>,
    help: Option<Text>,
    default: Json,
    spec: Option<SpecKind>,
}

/// A field name as markup can hold it without escaping: letters, digits, `_`, `.` and `-`, at
/// most [`MAX_FIELD_NAME`] characters; anything else becomes `_`.
fn clean_name(name: &str) -> String {
    name.chars()
        .take(MAX_FIELD_NAME)
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '.' | '-' => c,
            _ => '_',
        })
        .collect()
}

/// A text cut to [`MAX_SCHEMA_TEXT`] characters when it is a literal.
fn capped(text: Text) -> Text {
    match text {
        Text::Literal(s) if s.chars().count() > MAX_SCHEMA_TEXT => Text::Literal(Cow::Owned(
            s.chars()
                .take(MAX_SCHEMA_TEXT)
                .collect(),
        )),
        other => other,
    }
}

impl FieldSpec {
    fn runtime(
        name: &str,
        label: Text,
        spec: SpecKind,
        kind: FieldKind,
        control: Option<Control>,
    ) -> Self {
        Self {
            name: Cow::Owned(clean_name(name)),
            label: capped(label),
            kind,
            control,
            rules: Cow::Owned(Vec::new()),
            readonly: false,
            optional: true,
            options: Vec::new(),
            help: None,
            default: Json::Null,
            spec: Some(spec),
        }
    }

    /// The spec of a model field, with the control the form chose for it.
    pub(crate) fn of(meta: &'static FieldMeta, control: Option<Control>) -> Self {
        Self {
            name: Cow::Borrowed(meta.name),
            label: meta.label_text(),
            kind: meta.kind,
            control,
            rules: Cow::Borrowed(meta.rules),
            readonly: meta.readonly,
            optional: meta.optional,
            options: Vec::new(),
            help: None,
            default: Json::Null,
            spec: None,
        }
    }

    /// A one line text.
    pub fn text(name: &str, label: impl Into<Text>) -> Self {
        Self::runtime(name, label.into(), SpecKind::Text, FieldKind::Text, None)
    }

    /// A text of several lines in a growing text area.
    pub fn multiline(name: &str, label: impl Into<Text>) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Multiline,
            FieldKind::Text,
            Some(Control::Multiline),
        )
    }

    /// A whole number.
    pub fn integer(name: &str, label: impl Into<Text>) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Integer,
            FieldKind::Integer,
            None,
        )
    }

    /// A number with up to four decimals.
    pub fn decimal(name: &str, label: impl Into<Text>) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Decimal,
            FieldKind::Decimal,
            None,
        )
        .rule(Rule::Scale(4))
    }

    /// A length in pixels, shown in one of `units` (all of them when empty).
    pub fn length(name: &str, label: impl Into<Text>, units: &'static [LengthUnit]) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Length,
            FieldKind::Decimal,
            Some(Control::Length { units }),
        )
        .rule(Rule::Scale(4))
    }

    /// A color, with the app's `palette` in the picker.
    pub fn color(name: &str, label: impl Into<Text>, palette: &'static [&'static str]) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Color,
            FieldKind::Text,
            Some(Control::Color { palette }),
        )
    }

    /// A yes or no.
    pub fn boolean(name: &str, label: impl Into<Text>) -> Self {
        let mut spec = Self::runtime(name, label.into(), SpecKind::Boolean, FieldKind::Bool, None);
        spec.optional = false;
        spec
    }

    /// One of `options`, given as value and label pairs. At most [`MAX_SCHEMA_OPTIONS`] are kept.
    pub fn choice<V: Into<String>, L: Into<Text>>(
        name: &str,
        label: impl Into<Text>,
        options: impl IntoIterator<Item = (V, L)>,
    ) -> Self {
        let mut spec = Self::runtime(
            name,
            label.into(),
            SpecKind::Choice,
            FieldKind::Choice(&[]),
            None,
        );
        spec.options = options
            .into_iter()
            .map(|(v, l)| (v.into(), capped(l.into())))
            .filter(|(v, _)| v.chars().count() <= MAX_SCHEMA_TEXT)
            .take(MAX_SCHEMA_OPTIONS)
            .collect();
        spec
    }

    /// An expression such as `[Orders.Total] * 1.18m`, edited in the code editor
    /// ([`Control::Code`] with [`super::CodeLang::Expression`]).
    pub fn expression(name: &str, label: impl Into<Text>) -> Self {
        Self::runtime(
            name,
            label.into(),
            SpecKind::Expression,
            FieldKind::Text,
            Some(Control::Code(super::CodeLang::Expression)),
        )
    }

    fn rule(mut self, rule: Rule) -> Self {
        self.rules.to_mut().push(rule);
        self
    }

    /// The field must have a value.
    pub fn required(mut self) -> Self {
        self.optional = false;
        self.rule(Rule::Required)
    }

    /// The smallest number (pixels for a length). Ignored when it is above the `max` given
    /// before.
    pub fn min(self, n: i64) -> Self {
        match self.max_rule() {
            Some(max) if n > max => self,
            _ => self.rule(Rule::Min(n)),
        }
    }

    /// The largest number (pixels for a length). Ignored when it is below the `min` given
    /// before.
    pub fn max(self, n: i64) -> Self {
        match self.min_rule() {
            Some(min) if n < min => self,
            _ => self.rule(Rule::Max(n)),
        }
    }

    /// The most characters of a text.
    pub fn max_len(self, n: usize) -> Self {
        self.rule(Rule::MaxLen(n.min(MAX_SCHEMA_TEXT)))
    }

    /// The value shown when the values have none for the field.
    pub fn default(mut self, value: impl Serialize) -> Self {
        self.default = serde_json::to_value(value).unwrap_or(Json::Null);
        self
    }

    /// A line of help under the control.
    pub fn help(mut self, text: impl Into<Text>) -> Self {
        self.help = Some(capped(text.into()));
        self
    }

    /// The field's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn is_required(&self) -> bool {
        Rules(&self.rules).required()
    }

    pub(crate) fn max_len_rule(&self) -> Option<usize> {
        Rules(&self.rules).max_len()
    }

    pub(crate) fn min_len(&self) -> Option<usize> {
        Rules(&self.rules).min_len()
    }

    pub(crate) fn min_rule(&self) -> Option<i64> {
        Rules(&self.rules).min()
    }

    pub(crate) fn max_rule(&self) -> Option<i64> {
        Rules(&self.rules).max()
    }

    pub(crate) fn decimal_limits(&self) -> (u8, u8) {
        Rules(&self.rules).decimal_limits()
    }

    pub(crate) fn mask_pattern(&self) -> Option<&'static str> {
        Rules(&self.rules).mask_pattern()
    }

    /// The options of a choice as value and escaped label: the `#[choice]` enum of a model
    /// field, or the options of a runtime field.
    pub(crate) fn choice_options(&self, r: &Render) -> Vec<(&str, String)> {
        match self.kind {
            FieldKind::Choice(options) if self.spec.is_none() => options
                .iter()
                .map(|o| (o.value, r.text(&Text::key_or(o.key, o.label))))
                .collect(),
            _ => self
                .options
                .iter()
                .map(|(v, t)| (v.as_str(), r.text(t)))
                .collect(),
        }
    }

    /// Checks a value the user gave and returns it typed: a string for texts, colors and
    /// choices, a number for numbers and lengths (pixels), a boolean, or `null` for an empty
    /// optional field. The error is the message to show, in the words of `zeyn.validation.*`.
    /// The property inspector runs the same checks in the browser before it sends a value.
    ///
    /// ```
    /// use zeyn::ui::FieldSpec;
    /// use serde_json::json;
    /// let width = FieldSpec::length("width", "Width", &[]).min(0).max(2000);
    /// assert_eq!(width.check(&json!(37.8)).unwrap(), json!(37.8));
    /// assert!(width.check(&json!(-1)).is_err());
    /// assert!(width.check(&json!("NaN")).is_err());
    /// ```
    pub fn check(&self, value: &Json) -> Result<Json, Text> {
        let error = |key: &str| {
            Text::key(format!("zeyn.validation.{key}")).arg("label", self.label.clone())
        };
        let kind = self.spec.unwrap_or(SpecKind::Text);
        if kind == SpecKind::Boolean && value.is_null() {
            return Ok(Json::Bool(false));
        }
        let empty = value.is_null() || value.as_str() == Some("");
        if empty && kind != SpecKind::Boolean {
            return match self.is_required() {
                true => Err(error("required")),
                false => Ok(Json::Null),
            };
        }
        let range = |n: f64| {
            if let Some(min) = self.min_rule()
                && n < min as f64
            {
                return Err(error("min").arg("n", min.to_string()));
            }
            if let Some(max) = self.max_rule()
                && n > max as f64
            {
                return Err(error("max").arg("n", max.to_string()));
            }
            Ok(())
        };
        match kind {
            SpecKind::Text | SpecKind::Multiline | SpecKind::Expression => {
                let text = value
                    .as_str()
                    .ok_or_else(|| error("format"))?;
                let max = self
                    .max_len_rule()
                    .unwrap_or(MAX_SCHEMA_TEXT);
                if text.chars().count() > max {
                    return Err(error("max_len").arg("n", max.to_string()));
                }
                Ok(value.clone())
            }
            SpecKind::Integer => {
                let n = value
                    .as_i64()
                    .ok_or_else(|| error("format"))?;
                range(n as f64)?;
                Ok(Json::from(n))
            }
            SpecKind::Decimal | SpecKind::Length => {
                let n = value
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| error("format"))?;
                let scale = usize::from(self.decimal_limits().1);
                if format!("{n:.scale$}").parse::<f64>() != Ok(n) {
                    return Err(error("scale").arg("n", scale.to_string()));
                }
                range(n)?;
                Ok(value.clone())
            }
            SpecKind::Color => match value.as_str() {
                Some(c) if is_hex_color(c) => Ok(value.clone()),
                _ => Err(error("color")),
            },
            SpecKind::Boolean => value
                .as_bool()
                .map(Json::Bool)
                .ok_or_else(|| error("format")),
            SpecKind::Choice => match value.as_str() {
                Some(v) if self.options.iter().any(|(o, _)| o == v) => Ok(value.clone()),
                _ => Err(error("format")),
            },
        }
    }

    /// The value of this field in `values`, or its default, as a control shows it.
    fn value_in(&self, values: &Json) -> Value {
        let raw = values
            .get(&*self.name)
            .unwrap_or(&self.default);
        match (self.kind, raw) {
            (_, Json::Null) => Value::Null,
            (FieldKind::Bool, Json::Bool(b)) => Value::Bool(*b),
            (FieldKind::Integer, Json::Number(n)) => n
                .as_i64()
                .map_or(Value::Null, Value::Int),
            (FieldKind::Decimal, Json::Number(n)) => {
                let scale = self.decimal_limits().1;
                crate::Decimal::parse(&n.to_string())
                    .map_or(Value::Null, |d| Value::Number(d.round(scale)))
            }
            (FieldKind::Text | FieldKind::Choice(_), Json::String(s)) => Value::Text(
                s.chars()
                    .take(MAX_SCHEMA_TEXT)
                    .collect(),
            ),
            _ => Value::Null,
        }
    }

    /// The spec as the inspector reads it from a `zeyn:inspect` event, with texts in the
    /// words `text` gives.
    fn to_json(&self, text: &dyn Fn(&Text) -> String) -> Json {
        let mut out = json!({
            "name": self.name,
            "label": text(&self.label),
            "kind": self.spec.unwrap_or(SpecKind::Text).code(),
            "required": self.is_required(),
        });
        let mut put = |k: &str, v: Json| {
            out[k] = v;
        };
        if let Some(n) = self.min_rule() {
            put("min", json!(n));
        }
        if let Some(n) = self.max_rule() {
            put("max", json!(n));
        }
        if let Some(n) = self.max_len_rule() {
            put("max_len", json!(n));
        }
        if !self.default.is_null() {
            put("default", self.default.clone());
        }
        if let Some(help) = &self.help {
            put("help", json!(text(help)));
        }
        if !self.options.is_empty() {
            put(
                "options",
                self.options
                    .iter()
                    .map(|(v, l)| json!({ "value": v, "label": text(l) }))
                    .collect(),
            );
        }
        if self.kind == FieldKind::Decimal {
            put("scale", json!(self.decimal_limits().1));
        }
        match self.control {
            Some(Control::Length { units }) => put("units", json!(units)),
            Some(Control::Color { palette }) => put("palette", json!(palette)),
            _ => {}
        }
        out
    }
}

impl Serialize for FieldSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json(&Text::english)
            .serialize(serializer)
    }
}

/// The fields of an object whose properties are known only at run time, in titled sections.
/// It serializes to the `schema` of a `zeyn:inspect` event:
/// `{"sections": [{"title": "Look", "fields": [{"name": "fill", "label": "Fill", "kind": "color"}]}]}`.
/// Fields after the [`MAX_SCHEMA_FIELDS`]th are dropped.
#[derive(Default)]
pub struct FormSchema {
    sections: Vec<(Option<Text>, Vec<FieldSpec>)>,
}

impl FormSchema {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a titled, foldable group of fields.
    pub fn section(mut self, title: impl Into<Text>) -> Self {
        self.sections
            .push((Some(capped(title.into())), Vec::new()));
        self
    }

    /// Adds a field to the last section. A field whose name the schema already has is dropped.
    pub fn field(mut self, spec: FieldSpec) -> Self {
        if self.len() >= MAX_SCHEMA_FIELDS || self.get(&spec.name).is_some() {
            return self;
        }
        if self.sections.is_empty() {
            self.sections.push((None, Vec::new()));
        }
        if let Some((_, fields)) = self.sections.last_mut() {
            fields.push(spec);
        }
        self
    }

    /// The number of fields.
    pub fn len(&self) -> usize {
        self.sections
            .iter()
            .map(|(_, f)| f.len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The field with this name.
    pub fn get(&self, name: &str) -> Option<&FieldSpec> {
        self.fields().find(|f| f.name == name)
    }

    fn fields(&self) -> impl Iterator<Item = &FieldSpec> {
        self.sections
            .iter()
            .flat_map(|(_, f)| f.iter())
    }

    /// Checks every value with [`FieldSpec::check`]: the typed values, or the errors by field
    /// name. Values of names the schema does not have are dropped.
    pub fn check(
        &self,
        values: &Map<String, Json>,
    ) -> Result<Map<String, Json>, Vec<(String, Text)>> {
        let mut out = Map::new();
        let mut errors = Vec::new();
        for f in self.fields() {
            let raw = values
                .get(&*f.name)
                .unwrap_or(&f.default);
            match f.check(raw) {
                Ok(v) => {
                    out.insert(f.name.to_string(), v);
                }
                Err(e) => errors.push((f.name.to_string(), e)),
            }
        }
        if errors.is_empty() {
            Ok(out)
        } else {
            Err(errors)
        }
    }
}

impl FormSchema {
    /// The schema for a `zeyn:inspect` event with its titles, labels, help and option labels in
    /// `locale`. Serializing the schema gives the English words.
    ///
    /// ```
    /// use zeyn::ui::{FieldSpec, FormSchema};
    /// use zeyn::{I18n, Text};
    /// let schema = FormSchema::new().field(FieldSpec::text("t", Text::key("zeyn.ui.inspector.title")));
    /// let json = schema.to_json(I18n::builtin(), "tr");
    /// assert_eq!(json["sections"][0]["fields"][0]["label"], "Özellikler");
    /// ```
    pub fn to_json(&self, i18n: &crate::I18n, locale: &str) -> Json {
        self.json_with(&|t| i18n.text(locale, t))
    }

    fn json_with(&self, text: &dyn Fn(&Text) -> String) -> Json {
        let sections: Vec<Json> = self
            .sections
            .iter()
            .map(|(title, fields)| {
                let fields: Vec<Json> = fields
                    .iter()
                    .map(|f| f.to_json(text))
                    .collect();
                json!({ "title": title.as_ref().map(text), "fields": fields })
            })
            .collect();
        json!({ "sections": sections })
    }
}

impl Serialize for FormSchema {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json_with(&Text::english)
            .serialize(serializer)
    }
}

/// Opens a titled, foldable group of fields; close it with `</div></details>`.
pub(crate) fn open_section(out: &mut String, title: &str) {
    out.push_str(&format!(
        r#"<details class="z-form-section" open><summary class="z-form-section-title">{title}</summary><div class="z-form-section-body" role="group" aria-label="{title}">"#
    ));
}

/// Writes one field: its frame (label, tooltip, help and error line) and the control that fits
/// its kind. Model forms and runtime schemas both draw every field here; `special` draws the
/// controls only model fields have (lookups, dates, lines, files) and returns whether it did.
///
/// Model fields get the id `{scope}-{name}`; runtime fields pass their place in the schema as
/// `index` and get `{scope}-f{index}`, because their names may hold `-` and would collide with
/// the `-error` and `-help` ids of another field.
pub(crate) fn render_spec(
    out: &mut String,
    r: &Render,
    f: &FieldSpec,
    value: &Value,
    index: Option<usize>,
    tip: Option<&Text>,
    special: &dyn Fn(&mut String, &Slot) -> bool,
) {
    let scope = r.scope;
    let name = &f.name;
    let id = match index {
        Some(i) => format!("{scope}-f{i}"),
        None => format!("{scope}-{name}"),
    };
    let required = f.is_required();
    let tip = tip.map(|t| {
        let (attrs, html) = super::tip(r, &format!("{id}-tip"), t);
        let about = r.text(&Text::key("zeyn.ui.tip.about").arg("label", f.label.clone()));
        (format!(r#"<button type="button" class="z-tip-button" aria-label="{about}"{attrs}><span aria-hidden="true">i</span></button>"#), html)
    });
    let mut common =
        format!(r#"id="{id}" name="{name}" data-testid="{scope}.field.{name}" aria-describedby=""#);
    if tip.is_some() {
        let _ = write!(common, "{id}-tip ");
    }
    if f.help.is_some() {
        let _ = write!(common, "{id}-help ");
    }
    let _ = write!(common, r#"{id}-error""#);
    if required {
        common.push_str(r#" required aria-required="true""#);
    }
    if f.readonly {
        common.push_str(" readonly");
    }
    let star = if required {
        r#"<span class="z-required" aria-hidden="true">*</span>"#
    } else {
        ""
    };
    let text = r.text(&f.label);
    let label = match &tip {
        Some((button, html)) => format!(
            r#"<span class="z-label-row"><label for="{id}" id="{id}-label">{text}{star}</label>{button}</span>{html}"#
        ),
        None => format!(r#"<label for="{id}" id="{id}-label">{text}{star}</label>"#),
    };
    out.push_str(r#"<div class="z-field">"#);
    let slot = Slot {
        f,
        value,
        common: &common,
        id: &id,
        label: &label,
    };
    if !special(out, &slot) {
        match f.kind {
            FieldKind::Bool => super::render_bool(out, &slot),
            FieldKind::Choice(_) => super::render_choice(out, r, &slot),
            FieldKind::Choices(_) => super::render_choices(out, r, &slot),
            FieldKind::Money | FieldKind::Integer | FieldKind::Id | FieldKind::Decimal => {
                super::render_number(out, r, &slot)
            }
            _ => super::render_text(out, r, &slot),
        }
    }
    if let Some(help) = &f.help {
        out.push_str(&format!(
            r#"<p class="z-field-help" id="{id}-help">{}</p>"#,
            r.text(help)
        ));
    }
    out.push_str(&format!(
        r#"<p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#
    ));
}

/// A form drawn from a [`FormSchema`] and its values, with the same fields as a model form.
/// It has no submit: read the values back with [`FormSchema::check`].
///
/// ```
/// use zeyn::ui::{FieldSpec, FormSchema, SchemaForm};
/// let schema = FormSchema::new().field(FieldSpec::text("title", "Title"));
/// let form = SchemaForm::new(schema, serde_json::json!({ "title": "Sales" }));
/// # let _ = form;
/// ```
pub struct SchemaForm {
    schema: FormSchema,
    values: Json,
}

impl SchemaForm {
    pub fn new(schema: FormSchema, values: Json) -> Self {
        Self { schema, values }
    }
}

impl Component for SchemaForm {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<form class="z-form" data-testid="{}.form" novalidate>"#,
            r.scope
        ));
        let mut index = 0;
        for (title, fields) in &self.schema.sections {
            if let Some(title) = title {
                open_section(out, &r.text(title));
            }
            for f in fields {
                let value = f.value_in(&self.values);
                render_spec(out, r, f, &value, Some(index), None, &|_, _| false);
                index += 1;
            }
            if title.is_some() {
                out.push_str("</div></details>");
            }
        }
        out.push_str("</form>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "schema_form", "schema": self.schema, "values": self.values })
    }
}

impl From<SchemaForm> for Node {
    fn from(f: SchemaForm) -> Self {
        Node::new(f)
    }
}

/// The field templates of an island inspector: one `<template data-kind="...">` per
/// [`SpecKind`], drawn by [`render_spec`] for a field named [`TEMPLATE_NAME`] with an empty
/// label. The browser clones one per field and fills the name, label, rules and value in with
/// DOM calls, so a runtime field has the markup of a model field and no schema string is
/// parsed as HTML.
pub(crate) fn render_templates(out: &mut String, r: &Render) {
    for kind in SpecKind::ALL {
        let label = Text::Literal(Cow::Borrowed(""));
        let spec = match kind {
            SpecKind::Text => FieldSpec::text(TEMPLATE_NAME, label),
            SpecKind::Multiline => FieldSpec::multiline(TEMPLATE_NAME, label),
            SpecKind::Integer => FieldSpec::integer(TEMPLATE_NAME, label),
            SpecKind::Decimal => FieldSpec::decimal(TEMPLATE_NAME, label),
            SpecKind::Length => FieldSpec::length(TEMPLATE_NAME, label, &[]),
            SpecKind::Color => FieldSpec::color(TEMPLATE_NAME, label, &[]),
            SpecKind::Boolean => FieldSpec::boolean(TEMPLATE_NAME, label),
            SpecKind::Choice => {
                FieldSpec::choice(TEMPLATE_NAME, label, std::iter::empty::<(String, Text)>())
            }
            SpecKind::Expression => FieldSpec::expression(TEMPLATE_NAME, label),
        }
        .help(Text::Literal(Cow::Borrowed("")));
        out.push_str(&format!(r#"<template data-kind="{}">"#, kind.code()));
        render_spec(out, r, &spec, &Value::Null, None, None, &|_, _| false);
        out.push_str("</template>");
    }
}

/// The field name in the inspector templates; the browser puts the real name in its place.
pub(crate) const TEMPLATE_NAME: &str = "zeyn-field-name";

/// The limits the inspector checks a `zeyn:inspect` event against, as a `data-limits` value.
pub(crate) fn limits_attr() -> String {
    crate::render::attr(&json!({
        "fields": MAX_SCHEMA_FIELDS,
        "name": MAX_FIELD_NAME,
        "text": MAX_SCHEMA_TEXT,
        "options": MAX_SCHEMA_OPTIONS,
        "palette": super::MAX_PALETTE,
    }))
}
