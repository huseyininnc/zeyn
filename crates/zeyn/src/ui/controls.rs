//! Form controls: every field kind gets a Zeyn control, with the native element underneath so the
//! form still works without JavaScript and still submits plain values.
//!
//! | Field | Default control | Other controls |
//! |---|---|---|
//! | `bool` | checkbox | [`Control::Switch`] |
//! | `#[choice]` | select (ARIA select only combobox) | [`Control::Radio`], [`Control::Segmented`], [`Control::Listbox`] |
//! | `i32`, `i64`, `Money`, `Decimal` | number input formatted while typing, with step buttons | [`Control::Percent`], [`Control::Rating`] (whole numbers) |
//! | `String` | text input; a growing text area with a counter above 255 characters; a masked input with the `mask` rule | [`Control::Password`], [`Control::NewPassword`], [`Control::PasswordMeter`], [`Control::OneTimeCode`], [`Control::Code`] |

use crate::i18n::{NumberFormat, Text};
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
    /// A password input for text fields: never shown, never kept in drafts, `autocomplete` set
    /// to `current-password`.
    Password,
    /// A new password input for text fields (`autocomplete="new-password"`), for sign up and
    /// password changes.
    NewPassword,
    /// A one time code input for text fields: numeric keyboard, `autocomplete="one-time-code"`,
    /// never kept in drafts.
    OneTimeCode,
    /// The user name or e-mail of a sign in form: `autocomplete="username"`, no capitals or
    /// spell check, so password managers fill it with the password.
    Username,
    /// A number shown as a percentage, for `Decimal` and whole number fields. The value is in
    /// percent points: `12.5` is shown as `12.5 %` (`%12,5` in Turkish) and stored as `12.5`.
    Percent,
    /// A new password input with a strength meter. The meter is a hint computed in the browser;
    /// the field's rules (`min_len`, a model `validate` function) are what the server enforces.
    PasswordMeter,
    /// Stars from 1 to N for whole number fields, and half stars from 0.5 to N for `Decimal`
    /// fields (give them `scale = 1`; the server turns away values between the halves), as a
    /// radio group: one tab stop, arrow keys move. N is the field's `max` rule (5 without one, at most 10); give the field
    /// `#[rule(min = 1, max = N)]` so the server checks the value too. No star checked sends no
    /// value, which a `required` rule turns away.
    Rating,
    /// A growing text area for text fields of any length.
    Multiline,
    /// A color for text fields: a hex input (`#RRGGBB` or `#RRGGBBAA`) and a picker with red,
    /// green, blue and alpha sliders, the colors used last and the app's `palette`. The server
    /// takes only a valid hex color ([`is_hex_color`]).
    ///
    /// ```ignore
    /// .control(Shape::fill, Control::Color { palette: &["#1E88E5", "#43A047"] })
    /// ```
    Color {
        palette: &'static [&'static str],
    },
    /// A length for `Decimal` fields: a number with a unit switch. The value is stored in pixels
    /// (96 per inch) whatever unit the user reads; the first of `units` is shown until the user
    /// picks another, which the browser remembers. Arrow keys step by 0.1, with Shift by 1; the
    /// `min` and `max` rules are in pixels.
    ///
    /// ```ignore
    /// .control(Band::height, Control::Length { units: &[LengthUnit::Mm, LengthUnit::Pt] })
    /// ```
    Length {
        units: &'static [LengthUnit],
    },
    /// A code editor for text fields, such as SQL or a report expression: highlighting, line
    /// numbers, completion (`Form::complete`) and error markers (`Form::check`, or
    /// `ValidationErrors::add_at` in validation). See [`crate::ui::CodeEditor`].
    ///
    /// ```ignore
    /// Form::of(field)
    ///     .control(Calc::formula, Control::Code(CodeLang::Expression))
    ///     .complete(Calc::formula, complete_fields)
    /// ```
    Code(crate::ui::CodeLang),
}

/// A unit of [`Control::Length`]. Lengths are stored in CSS pixels, 96 to the inch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Mm,
    Cm,
    In,
    Px,
    Pt,
}

impl LengthUnit {
    /// Every unit, in the order the unit switch lists them.
    pub const ALL: &'static [LengthUnit] = &[
        LengthUnit::Mm,
        LengthUnit::Cm,
        LengthUnit::In,
        LengthUnit::Px,
        LengthUnit::Pt,
    ];

    /// Pixels in one unit.
    ///
    /// ```
    /// use zeyn::ui::LengthUnit;
    /// assert_eq!(LengthUnit::In.px(), 96.0);
    /// assert_eq!(LengthUnit::Pt.px(), 96.0 / 72.0);
    /// ```
    pub fn px(self) -> f64 {
        match self {
            LengthUnit::Mm => 96.0 / 25.4,
            LengthUnit::Cm => 96.0 / 2.54,
            LengthUnit::In => 96.0,
            LengthUnit::Px => 1.0,
            LengthUnit::Pt => 96.0 / 72.0,
        }
    }

    /// A length in this unit as pixels.
    ///
    /// ```
    /// use zeyn::ui::LengthUnit;
    /// assert_eq!(LengthUnit::In.to_px(0.5), 48.0);
    /// ```
    pub fn to_px(self, value: f64) -> f64 {
        value * self.px()
    }

    /// Pixels in this unit.
    pub fn from_px(self, px: f64) -> f64 {
        px / self.px()
    }

    /// The unit's short name: `mm`, `cm`, `in`, `px` or `pt`.
    pub fn code(self) -> &'static str {
        match self {
            LengthUnit::Mm => "mm",
            LengthUnit::Cm => "cm",
            LengthUnit::In => "in",
            LengthUnit::Px => "px",
            LengthUnit::Pt => "pt",
        }
    }

    /// The unit with this short name.
    pub fn parse(code: &str) -> Option<LengthUnit> {
        Self::ALL
            .iter()
            .copied()
            .find(|u| u.code() == code)
    }
}

/// Whether `text` is a hex color as [`Control::Color`] keeps it: `#` and six or eight hex
/// digits (`#RRGGBB`, `#RRGGBBAA`).
///
/// ```
/// use zeyn::ui::is_hex_color;
/// assert!(is_hex_color("#1e88e5"));
/// assert!(is_hex_color("#1E88E580"));
/// assert!(!is_hex_color("#fff"));
/// assert!(!is_hex_color("red"));
/// ```
pub fn is_hex_color(text: &str) -> bool {
    text.strip_prefix('#')
        .is_some_and(|h| (h.len() == 6 || h.len() == 8) && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Whether a choice value is the chosen one: model fields hold a [`Value::Choice`], runtime
/// fields the value as text.
fn is_chosen(value: &Value, option: &str) -> bool {
    match value {
        Value::Choice(c) => *c == option,
        Value::Text(t) => t == option,
        _ => false,
    }
}

/// What a control needs to render one field: the field's spec (a model field or a runtime
/// [`super::FieldSpec`]) and the parts of its frame.
pub(crate) struct Slot<'a> {
    pub f: &'a super::FieldSpec,
    pub value: &'a Value,
    /// Id, name, test id and validation attributes of the main input.
    pub common: &'a str,
    pub id: &'a str,
    /// The `<label>` element.
    pub label: &'a str,
}

fn ui(r: &Render, key: &str) -> String {
    r.text(&Text::key(format!("zeyn.ui.{key}")))
}

/// `bool` fields: a checkbox, or a switch.
pub(crate) fn render_bool(out: &mut String, s: &Slot) {
    let checked = if *s.value == Value::Bool(true) {
        " checked"
    } else {
        ""
    };
    let common = s.common;
    if s.f.control == Some(Control::Switch) {
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
    let FieldKind::Choice(_) = s.f.kind else {
        return;
    };
    let options = s.f.choice_options(r);
    match s.f.control {
        Some(Control::Radio | Control::Segmented) => {
            let class = if s.f.control == Some(Control::Segmented) {
                "z-radios z-segmented"
            } else {
                "z-radios"
            };
            let star = if s.f.is_required() {
                r#"<span class="z-required" aria-hidden="true">*</span>"#
            } else {
                ""
            };
            out.push_str(&format!(
                r#"<fieldset class="{class}" aria-describedby="{}-error"><legend>{}{star}</legend><div class="z-radios-list">"#,
                s.id,
                r.text(&s.f.label)
            ));
            let required = if s.f.is_required() { " required" } else { "" };
            for (i, (value, text)) in options.iter().enumerate() {
                let id = if i == 0 {
                    s.id.to_string()
                } else {
                    format!("{}-{i}", s.id)
                };
                let checked = if is_chosen(s.value, value) {
                    " checked"
                } else {
                    ""
                };
                out.push_str(&format!(
                    r#"<label class="z-radio" for="{id}"><input type="radio" id="{id}" name="{}" value="{}" data-testid="{}.field.{}.{}"{checked}{required}><span>{}</span></label>"#,
                    s.f.name,
                    escape(value),
                    r.scope,
                    s.f.name,
                    escape(value),
                    text
                ));
            }
            out.push_str("</div></fieldset>");
        }
        _ => {
            let listbox = s.f.control == Some(Control::Listbox);
            out.push_str(s.label);
            let (class, attr) = if listbox {
                (
                    "z-listbox-field",
                    format!(
                        r#" data-zeyn-listbox size="{}""#,
                        options.len().clamp(2, 8) + usize::from(s.f.optional)
                    ),
                )
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
            if s.f.optional {
                out.push_str(&format!(
                    r#"<option value="">{}</option>"#,
                    ui(r, "select.none")
                ));
            }
            for (value, text) in &options {
                let selected = if is_chosen(s.value, value) {
                    " selected"
                } else {
                    ""
                };
                out.push_str(&format!(
                    r#"<option value="{}"{selected}>{text}</option>"#,
                    escape(value)
                ));
            }
            out.push_str("</select></div>");
        }
    }
}

/// Whole numbers, money and decimals: formatted while typing in the user's language, with step
/// buttons. Key fields are not grouped (`10234`, not `10.234`). Decimals keep their field's scale.
pub(crate) fn render_number(out: &mut String, r: &Render, s: &Slot) {
    if s.f.control == Some(Control::Rating)
        && matches!(s.f.kind, FieldKind::Integer | FieldKind::Decimal)
    {
        return super::rating::render_rating(out, r, s);
    }
    if let Some(Control::Length { units }) = s.f.control
        && s.f.kind == FieldKind::Decimal
    {
        return render_length(out, r, s, units);
    }
    let f = NumberFormat::for_locale(r.locale);
    let money = s.f.kind == FieldKind::Money;
    let group = s.f.kind != FieldKind::Id;
    let scale = match s.f.kind {
        FieldKind::Money => 2,
        FieldKind::Decimal => s.f.decimal_limits().1,
        _ => 0,
    };
    let percent = s.f.control == Some(Control::Percent);
    let text = match s.value {
        Value::Money(m) => m.format_plain(r.locale),
        Value::Number(d) => d.format(r.locale),
        Value::Int(n) if group => {
            crate::value::Money::new(n.saturating_mul(100), Default::default())
                .format_plain(r.locale)
                .rsplit_once(f.decimal)
                .map(|(a, _)| a.to_string())
                .unwrap_or_default()
        }
        Value::Int(n) => n.to_string(),
        _ => String::new(),
    };
    let limits = [("min", s.f.min_rule()), ("max", s.f.max_rule())]
        .iter()
        .filter_map(|(k, v)| v.map(|n| format!(r#" data-{k}="{n}" aria-value{k}="{n}""#)))
        .collect::<String>();
    let _ = money;
    out.push_str(s.label);
    let (class, common) = if percent {
        let unit = format!("{}-unit", s.id);
        let common = s.common.replacen(
            r#"aria-describedby=""#,
            &format!(r#"aria-describedby="{unit} "#),
            1,
        );
        (
            if NumberFormat::percent_first(r.locale) {
                "z-number z-percent z-affix-first"
            } else {
                "z-number z-percent"
            },
            common,
        )
    } else {
        ("z-number", s.common.to_string())
    };
    out.push_str(&format!(
        r#"<div class="{class}" data-zeyn-number data-decimal="{}" data-group="{}" data-scale="{scale}" data-grouped="{group}"><input type="text" role="spinbutton" inputmode="{}" autocomplete="off" class="z-num" {common}{limits} value="{}">"#,
        escape(&f.decimal.to_string()),
        escape(&f.group.to_string()),
        if scale > 0 { "decimal" } else { "numeric" },
        escape(&text)
    ));
    if percent {
        out.push_str(&format!(
            r#"<span class="z-affix" aria-hidden="true">%</span><span class="z-sr-only" id="{}-unit">{}</span>"#,
            s.id,
            ui(r, "number.percent")
        ));
    }
    if !s.f.readonly {
        out.push_str(&format!(
            r#"<span class="z-steppers"><button type="button" tabindex="-1" data-zeyn-step="1" aria-label="{}"><span aria-hidden="true">+</span></button><button type="button" tabindex="-1" data-zeyn-step="-1" aria-label="{}"><span aria-hidden="true">&#8722;</span></button></span>"#,
            ui(r, "number.up"),
            ui(r, "number.down")
        ));
    }
    out.push_str("</div>");
}

/// [`Control::Code`]: the code editor, with the keys of the node's completion and check sources
/// when the user may run them. Model forms and runtime schemas draw it the same way.
pub(crate) fn render_code(
    out: &mut String,
    r: &Render,
    s: &Slot,
    lang: crate::ui::CodeLang,
    complete: Option<&str>,
    check: Option<&str>,
) {
    let maxlength =
        s.f.max_len_rule()
            .map(|n| format!(r#" maxlength="{n}""#))
            .unwrap_or_default();
    let attrs = format!("{}{maxlength}", s.common);
    let value = match s.value {
        Value::Text(t) => t.as_str(),
        _ => "",
    };
    out.push_str(s.label);
    crate::ui::render_editor(
        out,
        r,
        &crate::ui::EditorHtml {
            id: s.id,
            attrs: &attrs,
            lang,
            value,
            rows: 6,
            complete,
            check,
        },
    );
}

/// Text: an input, an email input, or a growing text area with a counter for long texts.
pub(crate) fn render_text(out: &mut String, r: &Render, s: &Slot) {
    if let Some(Control::Code(lang)) = s.f.control {
        return render_code(out, r, s, lang, None, None);
    }
    if let Some(pattern) = s.f.mask_pattern()
        && s.f.control.is_none()
    {
        crate::ui::inputs::render_mask(out, s, pattern);
        return;
    }
    if let Some(Control::Color { palette }) = s.f.control {
        return render_color(out, r, s, palette);
    }
    let text = match s.value {
        Value::Text(t) => escape(t),
        _ => String::new(),
    };
    let max = s.f.max_len_rule();
    let maxlength = max
        .map(|n| format!(r#" maxlength="{n}""#))
        .unwrap_or_default();
    let minlength =
        s.f.min_len()
            .map(|n| format!(r#" minlength="{n}""#))
            .unwrap_or_default();
    out.push_str(s.label);
    match max {
        None if s.f.control == Some(Control::Multiline) => {
            out.push_str(&format!(
                r#"<textarea rows="3" class="z-textarea" data-zeyn-autosize {}{minlength}>&#10;{text}</textarea>"#,
                s.common
            ));
        }
        Some(n)
            if (n > 255 || s.f.control == Some(Control::Multiline))
                && !matches!(
                    s.f.control,
                    Some(Control::Password | Control::NewPassword | Control::OneTimeCode)
                ) =>
        {
            let id = s.id;
            let common = s.common.replacen(
                r#"aria-describedby=""#,
                &format!(r#"aria-describedby="{id}-count "#),
                1,
            );
            let count = match s.value {
                Value::Text(t) => t.chars().count(),
                _ => 0,
            };
            out.push_str(&format!(
                r#"<textarea rows="3" class="z-textarea" data-zeyn-autosize {common}{minlength}{maxlength}>&#10;{text}</textarea><p class="z-counter" id="{id}-count" data-zeyn-counter="{n}" data-text="{}">{}</p>"#,
                ui(r, "text.count"),
                escape(&r.i18n.text(r.locale, &Text::key("zeyn.ui.text.count").arg("n", count.to_string()).arg("max", n.to_string())))
            ));
        }
        _ if matches!(
            s.f.control,
            Some(Control::Password | Control::NewPassword | Control::PasswordMeter)
        ) =>
        {
            let auto = if s.f.control == Some(Control::Password) {
                "current-password"
            } else {
                "new-password"
            };
            let meter = s.f.control == Some(Control::PasswordMeter);
            let id = s.id;
            let common = if meter {
                s.common.replacen(
                    r#"aria-describedby=""#,
                    &format!(r#"aria-describedby="{id}-strength "#),
                    1,
                )
            } else {
                s.common.to_string()
            };
            out.push_str(&format!(
                r#"<div class="z-password" data-zeyn-password><input type="password" autocomplete="{auto}" spellcheck="false" {common}{minlength}{maxlength} value=""><button type="button" class="z-password-toggle" aria-pressed="false" aria-controls="{id}" aria-label="{}" data-testid="{}.field.{}.toggle" hidden><span class="z-eye" aria-hidden="true"></span></button></div><p class="z-caps" data-zeyn-caps aria-live="polite" hidden>{}</p>"#,
                ui(r, "password.show"),
                r.scope,
                s.f.name,
                ui(r, "password.caps")
            ));
            if meter {
                let levels =
                    ["weak", "fair", "good", "strong"].map(|k| ui(r, &format!("password.{k}")));
                out.push_str(&format!(
                    r#"<div class="z-strength" data-zeyn-strength="{id}" data-levels="{}" hidden><span class="z-strength-bar" aria-hidden="true"><span></span><span></span><span></span><span></span></span><span class="z-strength-text" id="{id}-strength" aria-live="polite"></span></div>"#,
                    crate::render::attr(&levels)
                ));
            }
        }

        _ if s.f.control == Some(Control::OneTimeCode) => {
            out.push_str(&format!(r#"<input type="text" inputmode="text" autocomplete="one-time-code" spellcheck="false" {}{maxlength} value="">"#, s.common));
        }
        _ => {
            let email =
                s.f.rules
                    .contains(&crate::model::Rule::Email);
            let (kind, auto) = match (s.f.control == Some(Control::Username), email) {
                (true, true) => (
                    "email",
                    r#" autocomplete="username" autocapitalize="none" spellcheck="false""#,
                ),
                (true, false) => (
                    "text",
                    r#" autocomplete="username" autocapitalize="none" spellcheck="false""#,
                ),
                (false, true) => ("email", r#" autocomplete="email""#),
                (false, false) => ("text", ""),
            };
            out.push_str(&format!(
                r#"<input type="{kind}"{auto} {}{minlength}{maxlength} value="{text}">"#,
                s.common
            ));
        }
    }
}

/// Most colors of a [`Control::Color`] palette; the rest are dropped.
pub const MAX_PALETTE: usize = 64;

/// A color: the hex input the form submits (what works without JavaScript) with a chip of the
/// color and a button that the runtime (`colors.js`) shows to open the picker.
fn render_color(out: &mut String, r: &Render, s: &Slot, palette: &[&str]) {
    let text = match s.value {
        Value::Text(t) => escape(t),
        _ => String::new(),
    };
    let palette: Vec<&str> = palette
        .iter()
        .copied()
        .filter(|c| is_hex_color(c))
        .take(MAX_PALETTE)
        .collect();
    let texts = serde_json::json!({
        "dialog": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.dialog")),
        "red": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.red")),
        "green": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.green")),
        "blue": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.blue")),
        "alpha": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.alpha")),
        "recent": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.recent")),
        "palette": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.palette")),
        "done": r.i18n.text(r.locale, &Text::key("zeyn.ui.color.done")),
    });
    out.push_str(s.label);
    out.push_str(&format!(
        r##"<div class="z-color" data-zeyn-color data-palette="{}" data-texts="{}"><span class="z-color-chip" aria-hidden="true"></span><input type="text" class="z-color-hex" dir="ltr" autocomplete="off" autocapitalize="none" spellcheck="false" maxlength="9" pattern="#([0-9A-Fa-f]{{6}}|[0-9A-Fa-f]{{8}})" {} value="{text}"><button type="button" class="z-button z-color-open" aria-haspopup="dialog" aria-expanded="false" aria-label="{}" data-testid="{}.field.{}.picker" hidden><span class="z-color-open-mark" aria-hidden="true"></span></button></div>"##,
        crate::render::attr(&palette),
        crate::render::attr(&texts),
        s.common,
        ui(r, "color.open"),
        r.scope,
        s.f.name
    ));
}

/// A length: the number in pixels that the form submits (what works without JavaScript) and a
/// `px` sign, which the runtime (`length.js`) turns into a number in the chosen unit with a unit
/// switch.
fn render_length(out: &mut String, r: &Render, s: &Slot, units: &[LengthUnit]) {
    let f = NumberFormat::for_locale(r.locale);
    let scale = s.f.decimal_limits().1;
    let text = match s.value {
        Value::Number(d) => d
            .to_string()
            .replace('.', &f.decimal.to_string()),
        _ => String::new(),
    };
    let units: Vec<&str> = if units.is_empty() {
        LengthUnit::ALL.iter()
    } else {
        units.iter()
    }
    .map(|u| u.code())
    .collect();
    let limits = [("min", s.f.min_rule()), ("max", s.f.max_rule())]
        .iter()
        .filter_map(|(k, v)| v.map(|n| format!(r#" data-{k}="{n}""#)))
        .collect::<String>();
    out.push_str(s.label);
    out.push_str(&format!(
        r#"<div class="z-number z-length" data-zeyn-length data-units="{}" data-decimal="{}" data-scale="{scale}"{limits} data-text-unit="{}"><input type="text" inputmode="decimal" autocomplete="off" dir="ltr" class="z-num z-length-px" {} value="{}"><span class="z-affix" aria-hidden="true">px</span></div>"#,
        crate::render::attr(&units),
        escape(&f.decimal.to_string()),
        ui(r, "length.unit"),
        s.common,
        escape(&text)
    ));
}

/// Several options of a `#[choice]`: a native multiple select (what works without JavaScript)
/// that the runtime turns into chips and a multi select list (APG listbox, `aria-multiselectable`).
pub(crate) fn render_choices(out: &mut String, r: &Render, s: &Slot) {
    let FieldKind::Choices(options) = s.f.kind else {
        return;
    };
    let chosen: Vec<&str> = match s.value {
        Value::Text(t) => t
            .split(',')
            .filter(|v| !v.is_empty())
            .collect(),
        _ => Vec::new(),
    };
    let texts = serde_json::json!({
        "none": ui(r, "select.none"),
        "remove": ui(r, "lookup.remove"),
        "selected": ui(r, "lookup.selected"),
        "count": ui(r, "multi.count"),
        "open": ui(r, "lookup.open"),
    });
    out.push_str(s.label);
    out.push_str(&format!(
        r#"<div class="z-multi" data-zeyn-multi data-texts="{}"><select multiple size="{}" {}>"#,
        crate::render::attr(&texts),
        options.len().clamp(2, 8),
        s.common
    ));
    for o in options {
        let selected = if chosen.contains(&o.value) {
            " selected"
        } else {
            ""
        };
        out.push_str(&format!(
            r#"<option value="{}"{selected}>{}</option>"#,
            o.value,
            r.text(&Text::key_or(o.key, o.label))
        ));
    }
    out.push_str("</select></div>");
}

/// Reads several choices sent as a JSON array or as values joined with commas. Every value must
/// be an option; repeats are dropped. `None` when a value is unknown.
pub(crate) fn read_choices(
    options: &[crate::value::ChoiceOption],
    raw: &serde_json::Value,
) -> Option<serde_json::Value> {
    let parts: Vec<String> = match raw {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string))
            .collect::<Option<_>>()?,
        serde_json::Value::String(s) => s
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect(),
        serde_json::Value::Null => Vec::new(),
        _ => return None,
    };
    let mut values: Vec<&str> = Vec::with_capacity(parts.len().min(options.len()));
    for p in &parts {
        let option = options
            .iter()
            .find(|o| o.value == p.trim())?;
        if !values.contains(&option.value) {
            values.push(option.value);
        }
    }
    Some(serde_json::json!(values))
}

/// Several choices as the user reads them: their labels in the user's language, joined with
/// commas.
pub(crate) fn choices_text(value: &str, kind: &FieldKind, r: &Render) -> String {
    let FieldKind::Choices(options) = kind else {
        return value.to_string();
    };
    value
        .split(',')
        .filter(|v| !v.is_empty())
        .map(|v| match options.iter().find(|o| o.value == v) {
            Some(o) => r
                .i18n
                .text(r.locale, &Text::key_or(o.key, o.label)),
            None => v.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
