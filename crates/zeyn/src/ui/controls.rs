//! Form controls: every field kind gets a Zeyn control, with the native element underneath so the
//! form still works without JavaScript and still submits plain values.
//!
//! | Field | Default control | Other controls |
//! |---|---|---|
//! | `bool` | checkbox | [`Control::Switch`] |
//! | `#[choice]` | select (ARIA select only combobox) | [`Control::Radio`], [`Control::Segmented`], [`Control::Listbox`] |
//! | `i32`, `i64`, `Money` | number input formatted while typing, with step buttons | |
//! | `String` | text input; a growing text area with a counter above 255 characters | |

use crate::i18n::{NumberFormat, Text};
use crate::model::FieldMeta;
use crate::render::{Render, escape};
use crate::value::{FieldKind, Value};

/// A control other than the default for a field. See the table in the module docs.
///
/// ```ignore
/// Form::of(order).field(Order::urgent).control(Order::urgent, Control::Switch)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Checkbox,
    /// An on and off switch for `bool` fields (ARIA switch).
    Switch,
    /// A drop down list for `#[choice]` fields (ARIA select only combobox).
    Select,
    /// Radio buttons for short `#[choice]` lists.
    Radio,
    /// Radio buttons drawn as one row of buttons, for two to five short options.
    Segmented,
    /// An always open list with a search box for long `#[choice]` lists.
    Listbox,
}

/// What a control needs to render one field.
pub(crate) struct Slot<'a> {
    pub m: &'static FieldMeta,
    pub value: &'a Value,
    /// Id, name, test id and validation attributes of the main input.
    pub common: &'a str,
    pub id: &'a str,
    /// The `<label>` element.
    pub label: &'a str,
    pub control: Option<Control>,
}

fn ui(r: &Render, key: &str) -> String {
    r.text(&Text::key(format!("zeyn.ui.{key}")))
}

/// `bool` fields: a checkbox, or a switch.
pub(crate) fn render_bool(out: &mut String, s: &Slot) {
    let checked = if *s.value == Value::Bool(true) { " checked" } else { "" };
    let common = s.common;
    if s.control == Some(Control::Switch) {
        out.push_str(&format!(
            r#"<span class="z-check z-switch-wrap"><input type="checkbox" role="switch" class="z-switch" {common}{checked}><span class="z-switch-track" aria-hidden="true"></span>{}</span>"#,
            s.label
        ));
    } else {
        out.push_str(&format!(
            r#"<span class="z-check"><input type="checkbox" class="z-checkbox" {common}{checked}>{}</span>"#,
            s.label
        ));
    }
}

/// `#[choice]` fields: select, radio buttons, segmented buttons or an open list.
pub(crate) fn render_choice(out: &mut String, r: &Render, s: &Slot) {
    let FieldKind::Choice(options) = s.m.kind else { return };
    let label_of = |key: &'static str, fallback: &'static str| r.text(&Text::key_or(key, fallback));
    match s.control {
        Some(Control::Radio | Control::Segmented) => {
            let class = if s.control == Some(Control::Segmented) { "z-radios z-segmented" } else { "z-radios" };
            let star = if s.m.is_required() { r#"<span class="z-required" aria-hidden="true">*</span>"# } else { "" };
            out.push_str(&format!(
                r#"<fieldset class="{class}" aria-describedby="{}-error"><legend>{}{star}</legend><div class="z-radios-list">"#,
                s.id,
                r.label(s.m)
            ));
            let required = if s.m.is_required() { " required" } else { "" };
            for (i, o) in options.iter().enumerate() {
                let id = if i == 0 { s.id.to_string() } else { format!("{}-{i}", s.id) };
                let checked = if *s.value == Value::Choice(o.value) { " checked" } else { "" };
                out.push_str(&format!(
                    r#"<label class="z-radio" for="{id}"><input type="radio" id="{id}" name="{}" value="{}" data-testid="{}.field.{}.{}"{checked}{required}><span>{}</span></label>"#,
                    s.m.name,
                    o.value,
                    r.scope,
                    s.m.name,
                    o.value,
                    label_of(o.key, o.label)
                ));
            }
            out.push_str("</div></fieldset>");
        }
        _ => {
            let listbox = s.control == Some(Control::Listbox);
            out.push_str(s.label);
            let (class, attr) = if listbox {
                ("z-listbox-field", format!(r#" data-zeyn-listbox size="{}""#, options.len().clamp(2, 8) + usize::from(s.m.optional)))
            } else {
                ("z-select", " data-zeyn-select".to_string())
            };
            out.push_str(&format!(
                r#"<div class="{class}" data-text-none="{}" data-text-search="{}" data-text-no-results="{}"><select {}{attr}>"#,
                ui(r, "select.none"),
                ui(r, "select.search"),
                ui(r, "lookup.no_results"),
                s.common
            ));
            if s.m.optional {
                out.push_str(&format!(r#"<option value="">{}</option>"#, ui(r, "select.none")));
            }
            for o in options {
                let selected = if *s.value == Value::Choice(o.value) { " selected" } else { "" };
                out.push_str(&format!(r#"<option value="{}"{selected}>{}</option>"#, o.value, label_of(o.key, o.label)));
            }
            out.push_str("</select></div>");
        }
    }
}

/// Whole numbers and money: formatted while typing in the user's language, with step buttons.
/// Key fields are not grouped (`10234`, not `10.234`).
pub(crate) fn render_number(out: &mut String, r: &Render, s: &Slot) {
    let f = NumberFormat::for_locale(r.locale);
    let money = s.m.kind == FieldKind::Money;
    let group = s.m.kind != FieldKind::Id;
    let text = match s.value {
        Value::Money(m) => m.format_plain(r.locale),
        Value::Int(n) if group => crate::value::Money::new(n.saturating_mul(100), Default::default())
            .format_plain(r.locale)
            .rsplit_once(f.decimal)
            .map(|(a, _)| a.to_string())
            .unwrap_or_default(),
        Value::Int(n) => n.to_string(),
        _ => String::new(),
    };
    let limits = [("min", s.m.min()), ("max", s.m.max())]
        .iter()
        .filter_map(|(k, v)| v.map(|n| format!(r#" data-{k}="{n}" aria-value{k}="{n}""#)))
        .collect::<String>();
    out.push_str(s.label);
    out.push_str(&format!(
        r#"<div class="z-number" data-zeyn-number data-decimal="{}" data-group="{}" data-scale="{}" data-grouped="{group}"><input type="text" role="spinbutton" inputmode="{}" autocomplete="off" class="z-num" {}{limits} value="{}">"#,
        escape(&f.decimal.to_string()),
        escape(&f.group.to_string()),
        if money { 2 } else { 0 },
        if money { "decimal" } else { "numeric" },
        s.common,
        escape(&text)
    ));
    if !s.m.readonly {
        out.push_str(&format!(
            r#"<span class="z-steppers"><button type="button" tabindex="-1" data-zeyn-step="1" aria-label="{}"><span aria-hidden="true">+</span></button><button type="button" tabindex="-1" data-zeyn-step="-1" aria-label="{}"><span aria-hidden="true">&#8722;</span></button></span>"#,
            ui(r, "number.up"),
            ui(r, "number.down")
        ));
    }
    out.push_str("</div>");
}

/// Text: an input, an email input, or a growing text area with a counter for long texts.
pub(crate) fn render_text(out: &mut String, r: &Render, s: &Slot) {
    let text = match s.value {
        Value::Text(t) => escape(t),
        _ => String::new(),
    };
    let max = s.m.max_len();
    let maxlength = max.map(|n| format!(r#" maxlength="{n}""#)).unwrap_or_default();
    let minlength = s.m.min_len().map(|n| format!(r#" minlength="{n}""#)).unwrap_or_default();
    out.push_str(s.label);
    match max {
        Some(n) if n > 255 => {
            let id = s.id;
            let common = s.common.replace(&format!(r#"aria-describedby="{id}-error""#), &format!(r#"aria-describedby="{id}-count {id}-error""#));
            let count = match s.value {
                Value::Text(t) => t.chars().count(),
                _ => 0,
            };
            out.push_str(&format!(
                r#"<textarea rows="3" class="z-textarea" data-zeyn-autosize {common}{minlength}{maxlength}>{text}</textarea><p class="z-counter" id="{id}-count" data-zeyn-counter="{n}" data-text="{}">{}</p>"#,
                ui(r, "text.count"),
                escape(&r.i18n.text(r.locale, &Text::key("zeyn.ui.text.count").arg("n", count.to_string()).arg("max", n.to_string())))
            ));
        }
        _ => {
            let kind = if s.m.rules.contains(&crate::model::Rule::Email) { "email" } else { "text" };
            out.push_str(&format!(r#"<input type="{kind}" {}{minlength}{maxlength} value="{text}">"#, s.common));
        }
    }
}
