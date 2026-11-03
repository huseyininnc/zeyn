use serde::Serialize;

/// Escapes text and attribute values for HTML. User data always goes through here.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    escape_into(&mut out, s);
    out
}

/// Appends `s` to `out` escaped for HTML, as [`escape`] does, without a new string: runs without
/// a character to escape are copied in one piece.
pub fn escape_into(out: &mut String, s: &str) {
    let mut clean = 0;
    for (i, b) in s.bytes().enumerate() {
        let entity = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&s[clean..i]);
        out.push_str(entity);
        clean = i + 1;
    }
    out.push_str(&s[clean..]);
}

/// Serializes a value to JSON and escapes it for use inside an attribute.
pub fn attr<T: Serialize>(value: &T) -> String {
    escape(&serde_json::to_string(value).expect("value must serialize to JSON"))
}

/// Simplifies a label for test ids, including Turkish letters: `"Yeni Ürün"` -> `"yeni-urun"`.
pub fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'ç' | 'Ç' => 'c',
            'ğ' | 'Ğ' => 'g',
            'ı' | 'I' | 'İ' | 'i' => 'i',
            'ö' | 'Ö' => 'o',
            'ş' | 'Ş' => 's',
            'ü' | 'Ü' => 'u',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            _ => '-',
        };
        if mapped == '-' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else {
            out.push(mapped);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// What a component needs while rendering: test id scope and the user's language.
#[derive(Clone, Copy)]
pub struct Render<'a> {
    pub scope: &'a str,
    pub locale: &'a str,
    pub i18n: &'a crate::i18n::I18n,
    /// Zone in which moments (`DateTime`) are shown.
    pub tz: crate::time::TimeZone,
    /// Who the page is for. Components leave out actions this identity may not run. `None` (tests
    /// and tools) shows everything; the server always sets it.
    pub identity: Option<&'a crate::ctx::Identity>,
    /// The request, when rendering for a user: decides who may reveal masked values and records
    /// reads of sensitive records. `None` treats the viewer as having no permissions.
    pub ctx: Option<&'a crate::ctx::Ctx>,
    /// Path of the node being rendered, such as `1.0.2`; empty outside a node.
    pub path: &'a str,
    /// First pages of grids loaded before rendering, by node path.
    pub loaded: Option<&'a std::collections::HashMap<String, crate::screen::LoadedPage>>,
}

impl<'a> Render<'a> {
    pub fn new(scope: &'a str, locale: &'a str, i18n: &'a crate::i18n::I18n) -> Self {
        Self {
            scope,
            locale,
            i18n,
            tz: crate::time::TimeZone::UTC,
            identity: None,
            ctx: None,
            path: "",
            loaded: None,
        }
    }

    /// Sets who the page is for, so components can leave out what this user may not do.
    pub fn with_identity(mut self, identity: &'a crate::ctx::Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Whether the page's user passes an access rule. Without an identity, true.
    pub fn can(&self, access: &crate::action::Access) -> bool {
        self.identity
            .is_none_or(|i| i.can(access))
    }

    /// Renders for the user of this request.
    pub fn with_ctx(mut self, ctx: &'a crate::ctx::Ctx) -> Self {
        self.ctx = Some(ctx);
        self
    }

    pub fn with_tz(mut self, tz: crate::time::TimeZone) -> Self {
        self.tz = tz;
        self
    }

    /// English with the built in catalogs. For tests and tools.
    pub fn english(scope: &'a str) -> Render<'a> {
        Render::new(scope, "en", crate::i18n::I18n::builtin())
    }

    /// Resolves a text and escapes it for HTML.
    pub fn text(&self, text: &crate::i18n::Text) -> String {
        escape(&self.i18n.text(self.locale, text))
    }

    /// Field label in the user's language, escaped.
    pub fn label(&self, meta: &crate::model::FieldMeta) -> String {
        self.text(&meta.label_text())
    }

    /// Value as shown to the user, escaped: money and yes/no in the user's language, choices by label.
    pub fn value(&self, value: &crate::value::Value, kind: &crate::value::FieldKind) -> String {
        let mut out = String::new();
        self.value_into(&mut out, value, kind);
        out
    }

    /// Appends [`Render::value`] to `out` without building a string for the value first, for
    /// renderers that write many values, such as grid rows.
    pub fn value_into(
        &self,
        out: &mut String,
        value: &crate::value::Value,
        kind: &crate::value::FieldKind,
    ) {
        use crate::i18n::Text;
        use crate::value::{FieldKind, Value};
        use std::fmt::Write;
        match (value, kind) {
            (Value::Null, _) => {}
            (Value::Bool(true), _) => escape_into(
                out,
                &self
                    .i18n
                    .text(self.locale, &Text::key("zeyn.yes")),
            ),
            (Value::Bool(false), _) => escape_into(
                out,
                &self
                    .i18n
                    .text(self.locale, &Text::key("zeyn.no")),
            ),
            (Value::Int(n), _) => {
                let _ = write!(out, "{n}");
            }
            (Value::Decimal(x), _) => {
                let hundredths = (x * 100.0).round();
                if hundredths.is_finite() {
                    let plain = crate::value::Money::new(hundredths as i64, Default::default())
                        .format_plain(self.locale);
                    escape_into(out, &plain);
                }
            }
            (Value::Text(s), FieldKind::Choices(_)) => {
                escape_into(out, &crate::ui::choices_text(s, kind, self))
            }
            (Value::Text(s), _) => escape_into(out, s),
            (Value::Money(m), _) => escape_into(out, &m.format(self.locale)),
            (Value::Number(d), _) => escape_into(out, &d.format(self.locale)),
            (Value::Date(d), _) => {
                let _ = write!(
                    out,
                    r#"<time datetime="{d}">{}</time>"#,
                    d.format(self.locale)
                );
            }
            (Value::Time(t), _) => {
                let _ = write!(
                    out,
                    r#"<time datetime="{t}">{}</time>"#,
                    t.format(self.locale)
                );
            }
            (Value::LocalDateTime(l), _) => {
                let _ = write!(
                    out,
                    r#"<time datetime="{l}">{}</time>"#,
                    l.format(self.locale)
                );
            }
            (Value::DateTime(d), _) => {
                let _ = write!(
                    out,
                    r#"<time datetime="{d}" title="{d}">{}</time>"#,
                    d.format(self.tz, self.locale)
                );
            }
            (Value::Choice(v), FieldKind::Choice(options)) => {
                match options.iter().find(|o| o.value == *v) {
                    Some(o) => escape_into(
                        out,
                        &self
                            .i18n
                            .text(self.locale, &Text::key_or(o.key, o.label)),
                    ),
                    None => escape_into(out, v),
                }
            }
            (Value::Choice(v), _) => escape_into(out, v),
        }
    }
}
