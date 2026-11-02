use crate::action::Page;
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::model::{Field, FieldError, FieldMeta, Model, Rule, Valid, ValidationErrors};
use crate::render::{Render, escape};
use crate::screen::{Component, Effect, FormSource, Node, UiCtx};
use crate::time::{Date, DateTime, DateWords, LocalDateTime, LocalTimeError, Time, TimeZone};
use crate::ui::{
    ActionSource, Control, DataSource, DateOptions, Lines, LinesEditor, LookupItem, LookupQuery, LookupValue, Slot,
};
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
    /// A from and to date pair edited with one range picker.
    Range(FormField<M>, FormField<M>),
    /// Starts a step of a stepped form.
    Step(Text),
    /// Starts a titled, foldable group of fields.
    Section(Text),
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
    options: Vec<(&'static str, DateOptions)>,
    controls: Vec<(&'static str, Control)>,
    lookups: Vec<(&'static str, Box<dyn DataSource>, bool)>,
    /// Column headers of lookups that show several columns, by field.
    lookup_columns: Vec<(&'static str, Vec<Text>)>,
    lines: Vec<(&'static str, Box<dyn LinesEditor>)>,
    files: Vec<(&'static str, crate::files::FileRules, bool)>,
    /// Tooltips of fields, by field.
    tips: Vec<(&'static str, Text)>,
}

impl<M: Model> Form<M> {
    /// Form over the fields at these indexes of `M::fields()`. Used for grid row edits.
    pub(crate) fn with_fields(model: M, indexes: &[usize]) -> Self {
        let mut form = Self::of(model);
        form.items = indexes.iter().map(|i| FormItem::Field(FormField::at(*i))).collect();
        form
    }

    /// Empty form filled with the given value. Add fields with `field` and `row`.
    pub fn of(model: M) -> Self {
        Self {
            model,
            items: Vec::new(),
            options: Vec::new(),
            controls: Vec::new(),
            lookups: Vec::new(),
            lookup_columns: Vec::new(),
            lines: Vec::new(),
            files: Vec::new(),
            tips: Vec::new(),
        }
    }

    /// Form with every field of the model, in declaration order. Keys and versions become hidden
    /// inputs. Useful for simple models; hand written layouts use `of`.
    pub fn auto(model: M) -> Self {
        let items = (0..M::fields().len())
            .filter(|i| !M::fields()[*i].version)
            .map(|i| FormItem::Field(FormField::at(i)))
            .collect();
        Self {
            model,
            items,
            options: Vec::new(),
            controls: Vec::new(),
            lookups: Vec::new(),
            lookup_columns: Vec::new(),
            lines: Vec::new(),
            files: Vec::new(),
            tips: Vec::new(),
        }
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

    /// A from and to pair of date fields, edited together with one range picker that offers
    /// quick ranges (today, last 7 days, this month...). The server checks that `to` is not
    /// before `from`.
    pub fn range<T: FieldType + 'static>(mut self, from: Field<M, T>, to: Field<M, T>) -> Self {
        self.items.push(FormItem::Range(FormField::of(from), FormField::of(to)));
        self
    }

    /// Starts a step: the fields added after it, up to the next step, are shown together. A form
    /// with steps shows one step at a time with Back and Next buttons; Next checks the step's
    /// fields on the server before moving on, and the last step submits the whole form.
    ///
    /// ```ignore
    /// Form::of(application)
    ///     .step("Applicant").field(Application::name).field(Application::email)
    ///     .step("Address").field(Application::city)
    ///     .submit("Apply", apply)
    /// ```
    pub fn step(mut self, title: impl Into<Text>) -> Self {
        self.items.push(FormItem::Step(title.into()));
        self
    }

    /// Starts a section: the fields added after it, up to the next section or step, sit under a
    /// title the user can fold away. Used by property inspectors to group many fields.
    ///
    /// ```ignore
    /// Form::of(shape).section("Position").row(|r| r.field(Shape::x).field(Shape::y))
    ///     .section("Look").field(Shape::fill)
    /// ```
    pub fn section(mut self, title: impl Into<Text>) -> Self {
        self.items.push(FormItem::Section(title.into()));
        self
    }

    /// Field names of each step, in order. Empty when the form has no steps.
    pub fn steps(&self) -> Vec<Vec<&'static str>> {
        let mut steps: Vec<Vec<&'static str>> = Vec::new();
        for item in &self.items {
            match item {
                FormItem::Step(_) => steps.push(Vec::new()),
                FormItem::Section(_) => {}
                FormItem::Field(f) => steps.last_mut().into_iter().for_each(|s| s.push(f.meta.name)),
                FormItem::Row(fs) => {
                    steps.last_mut().into_iter().for_each(|s| s.extend(fs.iter().map(|f| f.meta.name)))
                }
                FormItem::Range(a, b) => {
                    steps.last_mut().into_iter().for_each(|s| s.extend([a.meta.name, b.meta.name]))
                }
            }
        }
        steps
    }

    /// Calendar and clock options of a date or time field: days that cannot be chosen, minute
    /// steps, seconds, week numbers. They shape the picker and are checked again on the server.
    ///
    /// ```ignore
    /// Form::of(visit).field(Visit::day).date_options(Visit::day, DateOptions::new().no_weekends())
    /// ```
    pub fn date_options<T: FieldType + 'static>(mut self, field: Field<M, T>, options: DateOptions) -> Self {
        let name = field.meta().name;
        self.options.retain(|(n, _)| *n != name);
        self.options.push((name, options));
        self
    }

    /// Another control than the default for a field: a switch for a `bool`, radio buttons, a
    /// segmented control or an open list for a `#[choice]`. See [`Control`].
    pub fn control<T: FieldType + 'static>(mut self, field: Field<M, T>, control: Control) -> Self {
        let name = field.meta().name;
        self.controls.retain(|(n, _)| *n != name);
        self.controls.push((name, control));
        self
    }

    /// Adds an id field edited with a searchable combobox. The options come from `action`, which
    /// gets the text typed so far and returns a page of [`LookupItem`]s; the server checks the
    /// action's permission on every search.
    ///
    /// ```ignore
    /// Form::of(order).lookup(Order::customer, find_customers)
    /// ```
    pub fn lookup<T, A>(mut self, field: Field<M, T>, _action: A) -> Self
    where
        T: FieldType + LookupValue + 'static,
        A: Action<Input = LookupQuery, Output = Result<Page<LookupItem>>>,
    {
        self.items.push(FormItem::Field(FormField::of(field)));
        self.lookups.push((field.meta().name, Box::new(ActionSource::<A>::new()), false));
        self
    }

    /// A tooltip for a field: an info button after its label shows `text` on hover, focus, tap
    /// and long press, and the field's input is described by it for screen readers. Use it for
    /// help such as a format or where a value comes from; the label and the rules must still say
    /// what is required.
    ///
    /// ```ignore
    /// Form::of(order).field(Order::reference).tip(Order::reference, t!("orders.reference_help"))
    /// ```
    pub fn tip<T: FieldType + 'static>(mut self, field: Field<M, T>, text: impl Into<Text>) -> Self {
        let name = field.meta().name;
        self.tips.retain(|(n, _)| *n != name);
        self.tips.push((name, text.into()));
        self
    }

    /// Adds an id field edited with a searchable combobox whose list shows several columns, such
    /// as code, name and city. `columns` are the headers; each [`LookupItem`] gives its values
    /// with [`LookupItem::cells`], in the same order. The search goes to `action` on the server
    /// exactly as for [`Form::lookup`], with its permission checked on every search.
    ///
    /// ```ignore
    /// Form::of(order).lookup_columns(Order::customer, find_customers, [t!("code"), t!("name"), t!("city")])
    /// ```
    pub fn lookup_columns<T, A, C>(mut self, field: Field<M, T>, action: A, columns: C) -> Self
    where
        T: FieldType + LookupValue + 'static,
        A: Action<Input = LookupQuery, Output = Result<Page<LookupItem>>>,
        C: IntoIterator,
        C::Item: Into<Text>,
    {
        let headers: Vec<Text> = columns.into_iter().take(crate::ui::MAX_LOOKUP_COLUMNS).map(Into::into).collect();
        self.lookup_columns.push((field.meta().name, headers));
        self.lookup(field, action)
    }

    /// Adds a field of several ids edited with a searchable combobox that shows the chosen ones
    /// as removable tags.
    pub fn lookup_many<A>(mut self, field: Field<M, Vec<i64>>, _action: A) -> Self
    where
        A: Action<Input = LookupQuery, Output = Result<Page<LookupItem>>>,
    {
        self.items.push(FormItem::Field(FormField::of(field)));
        self.lookups.push((field.meta().name, Box::new(ActionSource::<A>::new()), true));
        self
    }

    /// Adds a lines field (order lines, invoice lines) edited as a table: add, remove and move
    /// lines, a line total and a footer total. Every line is checked with its model's rules.
    ///
    /// ```ignore
    /// Form::of(order).lines(Order::lines, |l| {
    ///     l.column(OrderLine::product).column(OrderLine::quantity).column(OrderLine::price)
    ///         .total(OrderLine::quantity, OrderLine::price)
    /// })
    /// ```
    pub fn lines<L: Model>(mut self, field: Field<M, Vec<L>>, build: impl FnOnce(Lines<L>) -> Lines<L>) -> Self {
        self.items.push(FormItem::Field(FormField::of(field)));
        self.lines.push((field.meta().name, Box::new(build(Lines::new()))));
        self
    }

    /// Adds a file field edited with an upload control: drag and drop or a file picker, progress,
    /// cancel and retry, previews for images. Files go to the store registered with the same
    /// `rules` on the feature ([`Feature::files`](crate::Feature::files)), which checks size,
    /// type and permission again on the server. A `Vec<File>` field takes up to
    /// `rules.files()` files.
    ///
    /// ```ignore
    /// Form::of(ticket).file(Ticket::attachments, ATTACHMENTS)
    /// ```
    pub fn file<T>(mut self, field: Field<M, T>, rules: crate::files::FileRules) -> Self
    where
        T: FieldType + crate::files::UploadValue + 'static,
    {
        self.items.push(FormItem::Field(FormField::of(field)));
        self.files.push((field.meta().name, rules, T::MANY));
        self
    }

    fn files_of(&self, name: &str) -> Option<(crate::files::FileRules, bool)> {
        self.files.iter().find(|(n, _, _)| *n == name).map(|(_, r, many)| (*r, *many))
    }

    fn lines_of(&self, name: &str) -> Option<&dyn LinesEditor> {
        self.lines.iter().find(|(n, _)| *n == name).map(|(_, e)| e.as_ref())
    }

    fn control_of(&self, name: &str) -> Option<Control> {
        self.controls.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
    }

    fn lookup_of(&self, name: &str) -> Option<(&dyn DataSource, bool)> {
        self.lookups.iter().find(|(n, _, _)| *n == name).map(|(_, s, many)| (s.as_ref(), *many))
    }

    fn options_of(&self, name: &str) -> Option<&DateOptions> {
        self.options.iter().find(|(n, _)| *n == name).map(|(_, o)| o)
    }

    /// Submit button and action. Does not compile unless the action takes `Valid<M>`.
    pub fn submit<A: FormAction<M>>(self, label: impl Into<Text>, _action: A) -> SubmitForm<M, A> {
        SubmitForm { form: self, label: label.into(), on_success: None, _action: PhantomData }
    }

    fn fields(&self) -> impl Iterator<Item = &FormField<M>> {
        self.items
            .iter()
            .flat_map(|item| match item {
                FormItem::Field(f) => std::slice::from_ref(f).iter(),
                FormItem::Row(fs) => fs.iter(),
                FormItem::Range(a, _) => std::slice::from_ref(a).iter(),
                FormItem::Step(_) | FormItem::Section(_) => [].iter(),
            })
            .chain(self.items.iter().filter_map(|item| match item {
                FormItem::Range(_, b) => Some(b),
                _ => None,
            }))
    }

    fn render_fields(&self, out: &mut String, r: &Render) {
        if M::SENSITIVE {
            if let Some(ctx) = r.ctx {
                ctx.record_read(&self.model);
            }
        }
        let titles: Vec<&Text> =
            self.items.iter().filter_map(|i| if let FormItem::Step(t) = i { Some(t) } else { None }).collect();
        if !titles.is_empty() {
            out.push_str(&format!(
                r#"<ol class="z-steps" data-zeyn-steps aria-label="{}" data-text-progress="{}">"#,
                r.text(&Text::key("zeyn.ui.steps.label")),
                r.text(&Text::key("zeyn.ui.steps.progress"))
            ));
            for (i, t) in titles.iter().enumerate() {
                let current = if i == 0 { r#" aria-current="step""# } else { "" };
                out.push_str(&format!(r#"<li data-step="{i}"{current}><span class="z-step-n" aria-hidden="true">{}</span><span class="z-step-title">{}</span></li>"#, i + 1, r.text(t)));
            }
            out.push_str("</ol>");
        }
        let mut step = 0usize;
        let mut open = false;
        let mut section = false;
        for item in &self.items {
            if section && matches!(item, FormItem::Step(_) | FormItem::Section(_)) {
                out.push_str("</div></details>");
                section = false;
            }
            match item {
                FormItem::Section(title) => {
                    let title = r.text(title);
                    out.push_str(&format!(
                        r#"<details class="z-form-section" open><summary class="z-form-section-title">{title}</summary><div class="z-form-section-body" role="group" aria-label="{title}">"#
                    ));
                    section = true;
                }
                FormItem::Step(title) => {
                    if open {
                        out.push_str("</fieldset>");
                    }
                    out.push_str(&format!(
                        r#"<fieldset class="z-step" data-zeyn-step="{step}"><legend class="z-step-legend">{}</legend>"#,
                        r.text(title)
                    ));
                    step += 1;
                    open = true;
                }
                FormItem::Field(f) => render_field(out, r, f, self),
                FormItem::Range(from, to) => crate::ui::render_range(
                    out,
                    r,
                    [(from.meta, (from.value)(&self.model)), (to.meta, (to.value)(&self.model))],
                    self.options_of(from.meta.name),
                ),
                FormItem::Row(fields) => {
                    out.push_str(r#"<div class="z-form-row">"#);
                    for f in fields {
                        render_field(out, r, f, self);
                    }
                    out.push_str("</div>");
                }
            }
        }
        if section {
            out.push_str("</div></details>");
        }
        if open {
            out.push_str("</fieldset>");
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
                FormItem::Range(a, b) => vec![a, b],
                FormItem::Step(title) => {
                    list.push(json!({ "row": row, "step": title }));
                    Vec::new()
                }
                FormItem::Section(title) => {
                    list.push(json!({ "row": row, "section": title }));
                    Vec::new()
                }
            };
            for f in fields {
                let mut entry = json!({ "row": row, "field": f.meta });
                if let Some(editor) = self.lines_of(f.meta.name) {
                    entry["lines"] = editor.schema();
                }
                list.push(entry);
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
    ///
    /// Masked fields are only taken from the client while they are empty; [`Form::read_for`]
    /// also takes them from users who may reveal them.
    pub fn read(
        &self,
        values: &Map<String, Json>,
        locale: &str,
        tz: TimeZone,
    ) -> std::result::Result<Valid<M>, ValidationErrors> {
        self.read_with(
            values,
            &Reading::new(locale, tz, DateTime::now(), &DateWords::from_i18n(crate::i18n::I18n::builtin(), locale)),
            &|_| false,
        )
    }

    /// [`Form::read`] for the user of a request: their language, time zone, clock and catalogs
    /// (so `today`, `+3d` and `now` are read as they mean it), and masked fields only when they
    /// have the field's unmask permission.
    pub fn read_for(&self, values: &Map<String, Json>, ctx: &Ctx) -> std::result::Result<Valid<M>, ValidationErrors> {
        let words = DateWords::from_i18n(ctx.i18n(), ctx.locale());
        self.read_with(values, &Reading::new(ctx.locale(), ctx.time_zone(), ctx.now(), &words), &|m| {
            crate::privacy::can_unmask(Some(ctx), m)
        })
    }

    fn read_with(
        &self,
        values: &Map<String, Json>,
        reading: &Reading,
        unmask: &dyn Fn(&FieldMeta) -> bool,
    ) -> std::result::Result<Valid<M>, ValidationErrors> {
        let (model, mut errors) = self.merge(values, reading, unmask)?;
        self.check_pickers(&model, reading.tz, &mut errors);
        if let Err(rules) = model.validate() {
            let unreadable: Vec<(&str, Rule)> = errors.errors.iter().map(|e| (e.field, e.rule)).collect();
            let shadowed = |e: &FieldError| {
                unreadable.iter().any(|(f, r)| *f == e.field && (!matches!(r, Rule::Line { .. }) || *r == e.rule))
            };
            errors.errors.extend(rules.errors.into_iter().filter(|e| !shadowed(e)));
        }
        errors.into_result()?;
        Valid::new(model)
    }

    /// Server side checks of picker options and ranges: days that cannot be chosen, minute
    /// steps and a `to` before its `from`.
    fn check_pickers(&self, model: &M, tz: TimeZone, errors: &mut ValidationErrors) {
        let unreadable: Vec<&str> = errors.errors.iter().map(|e| e.field).collect();
        let value_of = |name: &str| {
            M::fields().iter().position(|m| m.name == name).and_then(|i| model.value_at(i)).unwrap_or(Value::Null)
        };
        for (name, options) in &self.options {
            let Some(meta) = M::fields().iter().find(|m| m.name == *name) else { continue };
            if unreadable.contains(name) {
                continue;
            }
            if let Some(key) = options.problem(&value_of(name), tz) {
                let mut message = Text::key(key).arg("label", meta.label_text());
                if key == "zeyn.validation.time_step" {
                    message = message.arg("n", options.step().to_string());
                }
                errors.errors.push(FieldError { field: meta.name, rule: Rule::Custom, message });
            }
        }
        if !self.files.is_empty() {
            let object = serde_json::to_value(model).unwrap_or(Json::Null);
            for (name, rules, many) in &self.files {
                let count = object.get(*name).and_then(Json::as_array).map_or(0, Vec::len);
                let Some(meta) = M::fields().iter().find(|m| m.name == *name) else { continue };
                if *many && count > rules.files() as usize && !unreadable.contains(name) {
                    errors.errors.push(FieldError {
                        field: meta.name,
                        rule: Rule::Custom,
                        message: Text::key("zeyn.files.too_many")
                            .arg("label", meta.label_text())
                            .arg("n", rules.files().to_string()),
                    });
                }
            }
        }
        for item in &self.items {
            let FormItem::Range(from, to) = item else { continue };
            let (a, b) = (value_of(from.meta.name), value_of(to.meta.name));
            if crate::ui::range_reversed(&a, &b) && !unreadable.contains(&to.meta.name) {
                errors.errors.push(FieldError {
                    field: to.meta.name,
                    rule: Rule::Custom,
                    message: Text::key("zeyn.validation.range_order")
                        .arg("label", to.meta.label_text())
                        .arg("other", from.meta.label_text()),
                });
            }
        }
    }

    fn merge(
        &self,
        values: &Map<String, Json>,
        reading: &Reading,
        unmask: &dyn Fn(&FieldMeta) -> bool,
    ) -> std::result::Result<(M, ValidationErrors), ValidationErrors> {
        let mut object = match serde_json::to_value(&self.model) {
            Ok(Json::Object(o)) => o,
            _ => return Err(general_error()),
        };
        let mut errors = ValidationErrors::default();
        let empty = |name: &str| {
            matches!(object.get(name), None | Some(Json::Null))
                || object.get(name) == Some(&Json::String(String::new()))
        };
        let editable: Vec<&FieldMeta> = self
            .fields()
            .map(|f| f.meta)
            .filter(|m| !m.key && !m.readonly && !m.version && (m.mask.is_none() || unmask(m) || empty(m.name)))
            .collect();
        let versions = M::fields().iter().filter(|m| m.version);
        for meta in editable.into_iter().chain(versions) {
            if let Some(editor) = self.lines_of(meta.name) {
                let current = object.get(meta.name).cloned().unwrap_or(Json::Null);
                if let Some((lines, problems)) = editor.read(values, meta, reading, &current) {
                    object.insert(meta.name.to_string(), lines);
                    errors.errors.extend(problems);
                }
                continue;
            }
            let Some(raw) = values.get(meta.name) else { continue };
            let current = object.get(meta.name).cloned().unwrap_or(Json::Null);
            match parse_value(meta, raw, reading, &current) {
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
pub(crate) enum Unreadable {
    Format,
    SkippedTime,
}

impl From<()> for Unreadable {
    fn from(_: ()) -> Self {
        Unreadable::Format
    }
}

/// What reading typed values depends on: the user's language, time zone, clock and words.
pub(crate) struct Reading<'a> {
    pub(crate) locale: &'a str,
    pub(crate) tz: TimeZone,
    pub(crate) now: LocalDateTime,
    pub(crate) words: &'a DateWords,
}

impl<'a> Reading<'a> {
    pub(crate) fn new(locale: &'a str, tz: TimeZone, now: DateTime, words: &'a DateWords) -> Self {
        Reading { locale, tz, now: now.local(tz), words }
    }
}

/// Reads a typed date, time or date time the way form fields do, for the picker's check endpoint.
/// `Err(true)` means the wall clock time does not exist in the zone (daylight saving).
pub(crate) fn read_moment(kind: FieldKind, text: &str, reading: &Reading) -> std::result::Result<Value, bool> {
    let locale = reading.locale;
    let t = text.trim();
    match kind {
        FieldKind::Date => Date::parse_input(t, locale, reading.now.date, reading.words).map(Value::Date).ok_or(false),
        FieldKind::Time => Time::parse_input(t, locale, reading.now.time, reading.words).map(Value::Time).ok_or(false),
        FieldKind::LocalDateTime => {
            LocalDateTime::parse_input(t, locale, reading.now, reading.words).map(Value::LocalDateTime).ok_or(false)
        }
        FieldKind::DateTime => {
            if let Some(exact) = DateTime::parse(t) {
                return Ok(Value::DateTime(exact));
            }
            let local = LocalDateTime::parse_input(t, locale, reading.now, reading.words).ok_or(false)?;
            DateTime::from_local(local, reading.tz).map(Value::DateTime).map_err(|LocalTimeError::Skipped| true)
        }
        _ => Err(false),
    }
}

/// Reads one submitted value as the field's type. Text arrives as typed by the user; wall clock
/// date times are read in the user's time zone.
pub(crate) fn parse_value(
    meta: &FieldMeta,
    raw: &Json,
    reading: &Reading,
    current: &Json,
) -> std::result::Result<Json, Unreadable> {
    let locale = reading.locale;
    if let FieldKind::Bool = meta.kind {
        return raw.as_bool().map(Json::Bool).ok_or(Unreadable::Format);
    }
    if let FieldKind::Custom { editor: editor @ ("file" | "files") } = meta.kind {
        return crate::files::parse_files(raw, editor == "files", meta.optional).ok_or(Unreadable::Format);
    }
    if let FieldKind::Choices(options) = meta.kind {
        return crate::ui::read_choices(options, raw).ok_or(Unreadable::Format);
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
        FieldKind::Custom { editor: "ids" } => {
            crate::ui::parse_ids(trimmed).map(|ids| Json::from(ids)).ok_or(Unreadable::Format)
        }
        FieldKind::Text => match meta.mask_pattern() {
            Some(pattern) => Ok(Json::String(crate::ui::mask_raw(pattern, &text))),
            None => Ok(Json::String(text)),
        },
        FieldKind::Custom { .. } => Ok(Json::String(text)),
        FieldKind::Decimal => {
            let (_, scale) = meta.decimal_limits();
            let d = crate::Decimal::parse_input(trimmed, locale).ok_or(Unreadable::Format)?;
            let d = if d.scale() < scale { d.round(scale) } else { d };
            serde_json::to_value(d).map_err(|_| Unreadable::Format)
        }
        FieldKind::Integer | FieldKind::Id => {
            let group = crate::i18n::NumberFormat::for_locale(locale).group;
            let digits: String = trimmed.chars().filter(|c| *c != group && !c.is_whitespace()).collect();
            digits.parse::<i64>().map(Json::from).map_err(|_| Unreadable::Format)
        }
        FieldKind::Date | FieldKind::Time | FieldKind::LocalDateTime | FieldKind::DateTime => {
            match read_moment(meta.kind, trimmed, reading) {
                Ok(v) => serde_json::to_value(v).map_err(|_| Unreadable::Format),
                Err(true) => Err(Unreadable::SkippedTime),
                Err(false) => Err(Unreadable::Format),
            }
        }
        FieldKind::Money => {
            let currency: Currency =
                current.get("currency").and_then(|c| serde_json::from_value(c.clone()).ok()).unwrap_or_default();
            let money = Money::parse(trimmed, locale, currency)?;
            serde_json::to_value(money).map_err(|_| Unreadable::Format)
        }
        FieldKind::Choices(_) => Err(Unreadable::Format),
        FieldKind::Choice(options) => options
            .iter()
            .find(|o| o.value == trimmed)
            .map(|o| Json::String(o.value.to_string()))
            .ok_or(Unreadable::Format),
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
        Value::Number(d) => d.format(locale),
    }
}

/// Writes the input that fits the field kind, with the validation attributes the client mirrors.
fn render_field<M: Model>(out: &mut String, r: &Render, f: &FormField<M>, form: &Form<M>) {
    let m = f.meta;
    let scope = r.scope;
    let value = (f.value)(&form.model);
    let options = form.options_of(m.name);
    let id = format!("{scope}-{}", m.name);
    if m.key {
        let v = match &value {
            Value::Int(n) => n.to_string(),
            _ => String::new(),
        };
        out.push_str(&format!(r#"<input type="hidden" name="{}" value="{v}">"#, m.name));
        return;
    }
    if m.mask.is_some() && value != Value::Null && value != Value::Text(String::new()) {
        crate::privacy::form_field(out, r, m, &value);
        return;
    }

    let required = m.is_required();
    let mut common =
        format!(r#"id="{id}" name="{}" data-testid="{scope}.field.{}" aria-describedby="{id}-error""#, m.name, m.name);
    if required {
        common.push_str(r#" required aria-required="true""#);
    }
    if m.readonly {
        common.push_str(" readonly");
    }

    let tip = form.tips.iter().find(|(n, _)| *n == m.name).map(|(_, t)| {
        let (attrs, html) = crate::ui::tip(r, &format!("{id}-tip"), t);
        let about = r.text(&Text::key("zeyn.ui.tip.about").arg("label", m.label_text()));
        (format!(r#"<button type="button" class="z-tip-button" aria-label="{about}"{attrs}><span aria-hidden="true">i</span></button>"#), html)
    });
    if tip.is_some() {
        common = common.replacen(r#"aria-describedby=""#, &format!(r#"aria-describedby="{id}-tip "#), 1);
    }
    out.push_str(r#"<div class="z-field">"#);
    let star = if required { r#"<span class="z-required" aria-hidden="true">*</span>"# } else { "" };
    let label = match &tip {
        Some((button, html)) => format!(
            r#"<span class="z-label-row"><label for="{id}" id="{id}-label">{}{star}</label>{button}</span>{html}"#,
            r.label(m)
        ),
        None => format!(r#"<label for="{id}" id="{id}-label">{}{star}</label>"#, r.label(m)),
    };
    let text = |v: &Value| match v {
        Value::Text(s) => escape(s),
        _ => String::new(),
    };
    let slot = Slot { m, value: &value, common: &common, id: &id, label: &label, control: form.control_of(m.name) };
    if let Some(editor) = form.lines_of(m.name) {
        let current = serde_json::to_value(&form.model).ok().and_then(|v| v.get(m.name).cloned()).unwrap_or(Json::Null);
        editor.render(out, r, m, &current);
        out.push_str(&format!(r#"<p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#));
        return;
    }
    if let Some((rules, many)) = form.files_of(m.name) {
        let current = serde_json::to_value(&form.model).ok().and_then(|v| v.get(m.name).cloned()).unwrap_or(Json::Null);
        crate::files::render_upload(out, r, m, &id, &rules, many, &current);
        out.push_str(&format!(r#"<p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#));
        return;
    }
    if let Some((_, many)) = form.lookup_of(m.name) {
        let columns = form.lookup_columns.iter().find(|(n, _)| *n == m.name).map(|(_, c)| c.as_slice());
        crate::ui::render_lookup(out, r, &slot, many, columns.unwrap_or_default());
        out.push_str(&format!(r#"<p class="z-field-error" id="{id}-error" aria-live="polite"></p></div>"#));
        return;
    }

    match m.kind {
        FieldKind::Bool => crate::ui::render_bool(out, &slot),
        FieldKind::Choice(_) => crate::ui::render_choice(out, r, &slot),
        FieldKind::Choices(_) => crate::ui::render_choices(out, r, &slot),
        FieldKind::Money | FieldKind::Integer | FieldKind::Id | FieldKind::Decimal => {
            crate::ui::render_number(out, r, &slot)
        }
        FieldKind::Text => crate::ui::render_text(out, r, &slot),
        FieldKind::Date | FieldKind::Time | FieldKind::DateTime | FieldKind::LocalDateTime => {
            out.push_str(&label);
            crate::ui::render_date_input(out, r, m, &value, &common, &id, options);
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
        let valid = self.form.read_for(&values, &ctx);
        if let Ok(saved) = &valid {
            ctx.record_changes(&self.form.model, saved);
        }
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
        for m in self.form.fields().map(|f| f.meta).filter(|m| !m.key && !m.readonly && !m.version && m.mask.is_none())
        {
            let (kind, hint) = match m.kind {
                FieldKind::Bool => ("boolean", String::new()),
                FieldKind::Integer | FieldKind::Id => ("string", "Whole number.".to_string()),
                FieldKind::Money => (
                    "string",
                    "Amount as typed by a user of the current language, for example 1.234,56 in Turkish.".to_string(),
                ),
                FieldKind::Decimal => {
                    let (_, scale) = m.decimal_limits();
                    (
                        "string",
                        format!(
                            "Number as typed by a user of the current language, at most {scale} decimals, for example 1.234,5 in Turkish."
                        ),
                    )
                }
                FieldKind::Choice(options) => {
                    let values: Vec<&str> = options.iter().map(|o| o.value).collect();
                    ("string", format!("One of: {}.", values.join(", ")))
                }
                FieldKind::Choices(options) => {
                    let values: Vec<&str> = options.iter().map(|o| o.value).collect();
                    ("string", format!("Values separated by commas, each one of: {}.", values.join(", ")))
                }
                FieldKind::Text if m.mask_pattern().is_some() => (
                    "string",
                    format!(
                        "Fills the pattern {}: 9 is a digit, a a letter, A an upper case letter, * either; send only those characters.",
                        m.mask_pattern().unwrap_or_default()
                    ),
                ),
                FieldKind::Custom { editor: "lines" } => (
                    "string",
                    format!(
                        "Send \"rows\" here and one value per cell as {}.<line>.<column>, lines numbered from 0.",
                        m.name
                    ),
                ),
                FieldKind::Custom { editor: "ids" } => ("string", "Ids separated by commas.".to_string()),
                FieldKind::Custom { editor: "file" } => (
                    "string",
                    "A file uploaded before, as JSON {\"id\",\"name\",\"size\",\"type\"}; models cannot upload."
                        .to_string(),
                ),
                FieldKind::Custom { editor: "files" } => (
                    "string",
                    "Files uploaded before, as a JSON list of {\"id\",\"name\",\"size\",\"type\"}.".to_string(),
                ),
                FieldKind::Text | FieldKind::Custom { .. } => ("string", String::new()),
                FieldKind::Date => ("string", "Date as YYYY-MM-DD.".to_string()),
                FieldKind::Time => ("string", "Time as HH:MM.".to_string()),
                FieldKind::LocalDateTime => ("string", "Date and time as YYYY-MM-DDTHH:MM, no time zone.".to_string()),
                FieldKind::DateTime => (
                    "string",
                    "Moment in RFC 3339 with offset, or YYYY-MM-DDTHH:MM in the user's time zone.".to_string(),
                ),
            };
            properties.insert(
                m.name.to_string(),
                json!({ "type": kind, "description": format!("{}. {hint}", m.label).trim().to_string() }),
            );
            if m.is_required() {
                required.push(m.name);
            }
        }
        for m in M::fields().iter().filter(|m| m.version) {
            properties.insert(
                m.name.to_string(),
                json!({ "type": "string", "description": "Row version as loaded. Send it back unchanged." }),
            );
        }
        json!({ "type": "object", "properties": properties, "required": required })
    }
}

/// Checks one step of a stepped form: reads every submitted value the way a save would and
/// returns the errors of the step's fields only. Nothing is saved; the submit action's
/// permission is required, as for saving.
impl<M: Model, A> DataSource for SubmitForm<M, A>
where
    A: Action<Input = Valid<M>>,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let step = input.get("step").and_then(Json::as_u64).unwrap_or(u64::MAX) as usize;
        let values = input.get("values").and_then(Json::as_object).cloned().unwrap_or_default();
        let fields = self.form.steps().get(step).cloned();
        let result = fields.map(|fields| match self.form.read_for(&values, &ctx) {
            Ok(_) => Vec::new(),
            Err(errors) => errors
                .errors
                .into_iter()
                .filter(|e| fields.contains(&e.field))
                .map(|e| json!({ "field": e.field, "rule": e.rule, "message": ctx.t(&e.message) }))
                .collect(),
        });
        Box::pin(async move {
            let errors = result.ok_or(Error::NotFound)?;
            Ok(json!({ "errors": errors }))
        })
    }
}

impl<M: Model> Component for Form<M> {
    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        self.lookup_of(key).map(|(s, _)| s)
    }

    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<form class="z-form" data-testid="{}.form" novalidate>"#, r.scope));
        self.render_fields(out, r);
        out.push_str("</form>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "form", "model": M::NAME, "fields": self.fields_schema(), "submit": null })
    }

    fn reveal(&self, field: &str) -> Option<crate::privacy::Revealed> {
        self.fields().find(|f| f.meta.name == field)?;
        crate::privacy::reveal_in(&self.model, field)
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
        let steps = if self.form.steps().is_empty() {
            String::new()
        } else {
            format!(
                r#"<button type="button" class="z-button" data-zeyn-step-back hidden data-testid="{0}.form.back">{1}</button><button type="button" class="z-button z-primary" data-zeyn-step-next hidden data-testid="{0}.form.next">{2}</button>"#,
                r.scope,
                r.text(&Text::key("zeyn.ui.steps.back")),
                r.text(&Text::key("zeyn.ui.steps.next"))
            )
        };
        out.push_str(&format!(
            r#"<div class="z-form-actions">{steps}<button type="submit" class="z-button z-primary" data-testid="{}.form.submit">{}</button></div></form>"#,
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

    fn reveal(&self, field: &str) -> Option<crate::privacy::Revealed> {
        self.form.reveal(field)
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        if key == "_step" && !self.form.steps().is_empty() {
            return Some(self);
        }
        self.form.lookup_of(key).map(|(s, _)| s)
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
