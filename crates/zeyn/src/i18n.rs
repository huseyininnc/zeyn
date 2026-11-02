use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Text shown to users: either a literal or a catalog key resolved in the user's language.
///
/// String literals convert into `Text` directly. Use the `t!` macro for catalog keys; it checks
/// at compile time that every file in `locales/` has the key.
///
/// ```
/// use zeyn::{I18n, Text};
/// let i18n = I18n::new("en");
/// let text = Text::key("zeyn.grid.range_total").arg("from", "1").arg("to", "50").arg("total", "230");
/// assert_eq!(i18n.text("en", &text), "1-50 of 230");
/// assert_eq!(i18n.text("tr", &text), "1-50 / 230");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Text {
    Literal(Cow<'static, str>),
    Key {
        key: Cow<'static, str>,
        /// Used when no catalog has the key.
        fallback: Option<Cow<'static, str>>,
        args: Vec<(&'static str, Text)>,
    },
}

impl Text {
    /// Catalog key. Prefer the `t!` macro, which checks the key at compile time.
    pub fn key(key: impl Into<Cow<'static, str>>) -> Self {
        Text::Key { key: key.into(), fallback: None, args: Vec::new() }
    }

    /// Catalog key with a text to use when no catalog has it.
    pub fn key_or(key: impl Into<Cow<'static, str>>, fallback: impl Into<Cow<'static, str>>) -> Self {
        Text::Key { key: key.into(), fallback: Some(fallback.into()), args: Vec::new() }
    }

    /// Fills `{name}` in the message. No effect on literals.
    pub fn arg(mut self, name: &'static str, value: impl Into<Text>) -> Self {
        if let Text::Key { args, .. } = &mut self {
            args.push((name, value.into()));
        }
        self
    }

    /// The text in English using the built in catalogs. For logs, tests and `Display`.
    pub fn english(&self) -> String {
        I18n::builtin().text("en", self)
    }
}

impl From<&'static str> for Text {
    fn from(s: &'static str) -> Self {
        Text::Literal(Cow::Borrowed(s))
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::Literal(Cow::Owned(s))
    }
}

impl Serialize for Text {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.english())
    }
}

/// Separators used to show and read numbers in a language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberFormat {
    pub decimal: char,
    pub group: char,
    /// Whether the currency symbol comes before the amount.
    pub symbol_first: bool,
}

impl NumberFormat {
    /// Format for a locale such as `tr`, `tr-TR` or `en-US`.
    pub fn for_locale(locale: &str) -> Self {
        match primary(locale) {
            "tr" | "de" | "es" | "it" | "nl" | "pt" | "id" => NumberFormat { decimal: ',', group: '.', symbol_first: false },
            "fr" | "ru" | "pl" | "cs" | "sv" | "fi" | "nb" | "uk" => {
                NumberFormat { decimal: ',', group: '\u{202f}', symbol_first: false }
            }
            _ => NumberFormat { decimal: '.', group: ',', symbol_first: true },
        }
    }
}

fn primary(locale: &str) -> &str {
    locale.split(['-', '_']).next().unwrap_or(locale)
}

/// Message catalogs. Built in `zeyn.*` messages exist in English and Turkish; apps add their own.
///
/// ```
/// use zeyn::I18n;
/// let i18n = I18n::new("en").catalog("tr", r#"{ "products": { "title": "Ürünler" } }"#).unwrap();
/// assert_eq!(i18n.lookup("tr-TR", "products.title"), Some("Ürünler"));
/// ```
#[derive(Clone, Debug)]
pub struct I18n {
    default: String,
    locales: Vec<String>,
    catalogs: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    /// Catalogs with the built in messages. `default` is used when a key is missing in the user's language.
    pub fn new(default: &str) -> Self {
        let mut i18n = I18n { default: default.to_string(), locales: Vec::new(), catalogs: HashMap::new() };
        i18n.merge("en", include_str!("../locales/en.json"), false).expect("built in catalog");
        i18n.merge("tr", include_str!("../locales/tr.json"), false).expect("built in catalog");
        i18n.locales.clear();
        i18n.locales.push(default.to_string());
        i18n
    }

    /// Shared catalogs with only the built in messages, default English.
    pub fn builtin() -> &'static I18n {
        static BUILTIN: OnceLock<I18n> = OnceLock::new();
        BUILTIN.get_or_init(|| I18n::new("en"))
    }

    /// Adds an app catalog (nested JSON objects of strings). App keys override built in ones.
    pub fn catalog(mut self, locale: &str, json: &str) -> Result<Self, String> {
        self.merge(locale, json, true)?;
        Ok(self)
    }

    /// Adds several catalogs at once, such as the `CATALOGS` constant of a shared library.
    /// Later catalogs override earlier ones, so add libraries first and the app last.
    ///
    /// ```
    /// use zeyn::I18n;
    /// const LIB: &[(&str, &str)] = &[("en", r#"{ "Customer": { "name": "Name" } }"#), ("tr", r#"{ "Customer": { "name": "Ad" } }"#)];
    /// let i18n = I18n::new("en").catalogs(LIB).unwrap();
    /// assert_eq!(i18n.lookup("tr", "Customer.name"), Some("Ad"));
    /// ```
    pub fn catalogs(mut self, catalogs: &[(&str, &str)]) -> Result<Self, String> {
        for (locale, json) in catalogs {
            self.merge(locale, json, true)?;
        }
        Ok(self)
    }

    fn merge(&mut self, locale: &str, json: &str, offered: bool) -> Result<(), String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("catalog `{locale}` is not valid JSON: {e}"))?;
        let catalog = self.catalogs.entry(locale.to_string()).or_default();
        flatten("", &value, catalog).map_err(|e| format!("catalog `{locale}`: {e}"))?;
        if offered && !self.locales.iter().any(|l| l == locale) {
            self.locales.push(locale.to_string());
        }
        Ok(())
    }

    pub fn default_locale(&self) -> &str {
        &self.default
    }

    /// Locales the app offers, default first.
    pub fn locales(&self) -> &[String] {
        &self.locales
    }

    /// Message for a key: exact locale, then its language (`tr` for `tr-TR`), then the default locale.
    pub fn lookup(&self, locale: &str, key: &str) -> Option<&str> {
        [locale, primary(locale), self.default.as_str()]
            .into_iter()
            .find_map(|l| self.catalogs.get(l).and_then(|c| c.get(key)))
            .map(String::as_str)
    }

    /// Resolves a text in a locale. Missing keys show the fallback, or the key itself.
    pub fn text(&self, locale: &str, text: &Text) -> String {
        match text {
            Text::Literal(s) => s.to_string(),
            Text::Key { key, fallback, args } => {
                let template = self.lookup(locale, key).or(fallback.as_deref()).unwrap_or(key);
                let mut out = template.to_string();
                for (name, value) in args {
                    out = out.replace(&format!("{{{name}}}"), &self.text(locale, value));
                }
                out
            }
        }
    }

    /// Picks the locale: a supported cookie value first, then `Accept-Language`, then the default.
    pub fn negotiate(&self, cookie: Option<&str>, accept_language: Option<&str>) -> String {
        if let Some(c) = cookie.filter(|c| self.locales.iter().any(|l| l == c)) {
            return c.to_string();
        }
        let mut wanted: Vec<(f32, &str)> = accept_language
            .unwrap_or("")
            .split(',')
            .filter_map(|part| {
                let mut pieces = part.trim().split(';');
                let tag = pieces.next()?.trim();
                let q = pieces
                    .find_map(|p| p.trim().strip_prefix("q=").and_then(|q| q.parse::<f32>().ok()))
                    .unwrap_or(1.0);
                (!tag.is_empty() && q > 0.0).then_some((q, tag))
            })
            .collect();
        wanted.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, tag) in wanted {
            if let Some(l) = self.locales.iter().find(|l| l.eq_ignore_ascii_case(tag)) {
                return l.clone();
            }
            if let Some(l) = self.locales.iter().find(|l| primary(l).eq_ignore_ascii_case(primary(tag))) {
                return l.clone();
            }
        }
        self.default.clone()
    }

    /// Keys that exist in the default locale but are missing in another offered locale.
    pub fn missing_keys(&self) -> Vec<(String, String)> {
        let Some(base) = self.catalogs.get(&self.default) else { return Vec::new() };
        let mut missing = Vec::new();
        for locale in self.locales.iter().filter(|l| **l != self.default) {
            let catalog = self.catalogs.get(locale);
            for key in base.keys() {
                if !catalog.is_some_and(|c| c.contains_key(key)) {
                    missing.push((locale.clone(), key.clone()));
                }
            }
        }
        missing.sort();
        missing
    }
}

fn flatten(prefix: &str, value: &serde_json::Value, out: &mut HashMap<String, String>) -> Result<(), String> {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, v, out)?;
            }
            Ok(())
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
            Ok(())
        }
        _ => Err(format!("`{prefix}` must be a string or an object")),
    }
}
