use serde::Serialize;

/// Escapes text and attribute values for HTML. User data always goes through here.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
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
    /// Path of the node being rendered, such as `1.0.2`; empty outside a node.
    pub path: &'a str,
    /// First pages of grids loaded before rendering, by node path.
    pub loaded: Option<&'a std::collections::HashMap<String, crate::screen::LoadedPage>>,
}

impl<'a> Render<'a> {
    pub fn new(scope: &'a str, locale: &'a str, i18n: &'a crate::i18n::I18n) -> Self {
        Self { scope, locale, i18n, tz: crate::time::TimeZone::UTC, path: "", loaded: None }
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
        use crate::i18n::Text;
        use crate::value::{FieldKind, Value};
        match (value, kind) {
            (Value::Null, _) => String::new(),
            (Value::Bool(true), _) => self.text(&Text::key("zeyn.yes")),
            (Value::Bool(false), _) => self.text(&Text::key("zeyn.no")),
            (Value::Int(n), _) => n.to_string(),
            (Value::Decimal(x), _) => {
                let hundredths = (x * 100.0).round();
                if hundredths.is_finite() {
                    escape(&crate::value::Money::new(hundredths as i64, Default::default()).format_plain(self.locale))
                } else {
                    String::new()
                }
            }
            (Value::Text(s), _) => escape(s),
            (Value::Money(m), _) => escape(&m.format(self.locale)),
            (Value::Date(d), _) => format!(r#"<time datetime="{d}">{}</time>"#, d.format(self.locale)),
            (Value::Time(t), _) => format!(r#"<time datetime="{t}">{}</time>"#, t.format(self.locale)),
            (Value::LocalDateTime(l), _) => format!(r#"<time datetime="{l}">{}</time>"#, l.format(self.locale)),
            (Value::DateTime(d), _) => format!(
                r#"<time datetime="{d}" title="{d}">{}</time>"#,
                d.format(self.tz, self.locale)
            ),
            (Value::Choice(v), FieldKind::Choice(options)) => match options.iter().find(|o| o.value == *v) {
                Some(o) => self.text(&Text::key_or(o.key, o.label)),
                None => escape(v),
            },
            (Value::Choice(v), _) => escape(v),
        }
    }
}
