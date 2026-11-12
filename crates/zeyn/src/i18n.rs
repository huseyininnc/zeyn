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
        Text::Key {
            key: key.into(),
            fallback: None,
            args: Vec::new(),
        }
    }

    /// Catalog key with a text to use when no catalog has it.
    pub fn key_or(
        key: impl Into<Cow<'static, str>>,
        fallback: impl Into<Cow<'static, str>>,
    ) -> Self {
        Text::Key {
            key: key.into(),
            fallback: Some(fallback.into()),
            args: Vec::new(),
        }
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

/// Fills a message: `{name}` takes the argument's text; `{n, plural, one {# file} other {# files}}`
/// picks a case by the CLDR plural category of the integer `n` in `locale` (`=0` style cases
/// match exactly and win), with `#` standing for the number; `{g, select, female {..} other {..}}`
/// picks the case named by the argument's text. `other` is the case when nothing else matches.
/// Cases may hold further placeholders. A placeholder without its argument stays as written.
///
/// ```
/// use zeyn::format_message;
/// let m = "{n, plural, =0 {No files} one {# file} other {# files}}";
/// assert_eq!(format_message("en", m, &[("n", "0".into())]), "No files");
/// assert_eq!(format_message("en", m, &[("n", "1".into())]), "1 file");
/// assert_eq!(format_message("en", m, &[("n", "5".into())]), "5 files");
/// let r = "{n, plural, one {# файл} few {# файла} many {# файлов} other {# файла}}";
/// assert_eq!(format_message("ru", r, &[("n", "21".into())]), "21 файл");
/// assert_eq!(format_message("ru", r, &[("n", "3".into())]), "3 файла");
/// assert_eq!(format_message("ru", r, &[("n", "11".into())]), "11 файлов");
/// let s = "{who, select, ada {Ada wrote} other {Someone wrote}}";
/// assert_eq!(format_message("en", s, &[("who", "ada".into())]), "Ada wrote");
/// ```
pub fn format_message(locale: &str, template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(len) = closing(&rest[start..]) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &rest[start + 1..start + len];
        match expand(locale, inner, args) {
            Some(text) => out.push_str(&text),
            None => out.push_str(&rest[start..=start + len]),
        }
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    out
}

/// The length up to the brace that closes the one at the start of `s`, nested braces counted.
fn closing(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The text of one placeholder's inside, or `None` when its argument is missing.
fn expand(locale: &str, inner: &str, args: &[(&str, String)]) -> Option<String> {
    let value = |name: &str| {
        args.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    };
    let mut parts = inner.splitn(3, ',');
    let name = parts.next()?.trim();
    let Some(kind) = parts.next() else {
        return value(name).map(str::to_string);
    };
    let cases = parse_cases(parts.next().unwrap_or(""));
    let arg = value(name)?;
    let pick = |key: &str| {
        cases
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    match kind.trim() {
        "plural" => {
            let n: i64 = arg.trim().parse().ok()?;
            let body = pick(&format!("={n}"))
                .or_else(|| pick(plural_category(locale, n)))
                .or_else(|| pick("other"))?;
            Some(format_message(
                locale,
                &body.replace('#', &n.to_string()),
                args,
            ))
        }
        "select" => {
            let body = pick(arg.trim()).or_else(|| pick("other"))?;
            Some(format_message(locale, body, args))
        }
        _ => None,
    }
}

/// The cases of a plural or select placeholder: `one {..} other {..}` as `(key, text)` pairs.
pub fn parse_cases(s: &str) -> Vec<(&str, &str)> {
    let mut cases = Vec::new();
    let mut rest = s.trim_start();
    while let Some(open) = rest.find('{') {
        let key = rest[..open].trim();
        let Some(len) = closing(&rest[open..]) else {
            break;
        };
        cases.push((key, &rest[open + 1..open + len]));
        rest = rest[open + len + 1..].trim_start();
    }
    cases
}

/// The CLDR cardinal plural category of an integer in a language: `zero`, `one`, `two`, `few`,
/// `many` or `other`, for the languages Zeyn ships; other languages use `one` for 1.
pub fn plural_category(locale: &str, n: i64) -> &'static str {
    let lang = locale
        .split(['-', '_'])
        .next()
        .unwrap_or(locale);
    let i = n.unsigned_abs();
    let (m10, m100) = (i % 10, i % 100);
    match lang {
        "ja" | "ko" | "zh" => "other",
        "hi" => {
            if i <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "ru" => {
            if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "ar" => match i {
            0 => "zero",
            1 => "one",
            2 => "two",
            _ if (3..=10).contains(&m100) => "few",
            _ if (11..=99).contains(&m100) => "many",
            _ => "other",
        },
        _ => {
            if i == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// An integer a plural message counts. `t!` passes plural arguments through it, so a text or a
/// float is a compile error that says what to pass.
///
/// ```
/// let one = zeyn::t!("zeyn.files.too_many", label = "Photos", n = 1u32);
/// assert_eq!(one.english(), "Photos takes at most 1 file.");
/// let many = zeyn::t!("zeyn.files.too_many", label = "Photos", n = 4usize);
/// assert_eq!(many.english(), "Photos takes at most 4 files.");
/// ```
///
/// ```compile_fail
/// let _ = zeyn::t!("zeyn.files.too_many", label = "Photos", n = "two");
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a whole number, and a plural message counts whole numbers",
    label = "a plural argument must be an integer",
    note = "pass the count as an integer, such as `n = items.len()`; format money and decimals in a plain `{{name}}` placeholder"
)]
pub trait PluralCount {
    fn count(&self) -> String;
}

macro_rules! plural_count {
    ($($t:ty),*) => {
        $(impl PluralCount for $t {
            fn count(&self) -> String {
                self.to_string()
            }
        })*
    };
}

plural_count!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

impl<T: PluralCount + ?Sized> PluralCount for &T {
    fn count(&self) -> String {
        (**self).count()
    }
}

/// Used by `t!` for plural arguments.
///
/// ```compile_fail
/// let _ = zeyn::__plural(&"five");
/// ```
#[doc(hidden)]
pub fn __plural<T: PluralCount + ?Sized>(value: &T) -> String {
    value.count()
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
            "tr" | "de" | "es" | "it" | "nl" | "pt" | "id" => NumberFormat {
                decimal: ',',
                group: '.',
                symbol_first: false,
            },
            "fr" | "ru" | "pl" | "cs" | "sv" | "fi" | "nb" | "uk" => NumberFormat {
                decimal: ',',
                group: '\u{202f}',
                symbol_first: false,
            },
            _ => NumberFormat {
                decimal: '.',
                group: ',',
                symbol_first: true,
            },
        }
    }

    /// Whether the percent sign comes before the number, as in Turkish `%15`.
    pub fn percent_first(locale: &str) -> bool {
        matches!(primary(locale), "tr" | "eu")
    }
}

fn primary(locale: &str) -> &str {
    locale
        .split(['-', '_'])
        .next()
        .unwrap_or(locale)
}

/// The framework's own catalogs compiled into this build: English and Turkish always, the other
/// languages behind their Cargo feature (`lang-zh`, `lang-ru`, `lang-es`, `lang-ko`, `lang-ja`,
/// `lang-hi`, `lang-ar`, or `lang-all` for every one).
const BUILTIN_CATALOGS: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en.json")),
    ("tr", include_str!("../locales/tr.json")),
    #[cfg(feature = "lang-zh")]
    ("zh", include_str!("../locales/zh.json")),
    #[cfg(feature = "lang-ru")]
    ("ru", include_str!("../locales/ru.json")),
    #[cfg(feature = "lang-es")]
    ("es", include_str!("../locales/es.json")),
    #[cfg(feature = "lang-ko")]
    ("ko", include_str!("../locales/ko.json")),
    #[cfg(feature = "lang-ja")]
    ("ja", include_str!("../locales/ja.json")),
    #[cfg(feature = "lang-hi")]
    ("hi", include_str!("../locales/hi.json")),
    #[cfg(feature = "lang-ar")]
    ("ar", include_str!("../locales/ar.json")),
];

/// Message catalogs. Built in `zeyn.*` messages exist in English and Turkish, and in the
/// languages turned on with the `lang-*` Cargo features; apps add their own.
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
    ///
    /// English, Turkish and the default language are read at once. The other built in languages
    /// are read when the app adds a catalog for them, so a build with `lang-all` keeps in memory
    /// only the languages the app offers.
    pub fn new(default: &str) -> Self {
        let mut i18n = I18n {
            default: default.to_string(),
            locales: Vec::new(),
            catalogs: HashMap::new(),
        };
        for locale in ["en", "tr", primary(default)] {
            i18n.load_builtin(locale);
        }
        i18n.locales.push(default.to_string());
        i18n
    }

    /// Reads the framework's own catalog of a language once, if this build has it.
    fn load_builtin(&mut self, locale: &str) {
        let locale = primary(locale);
        if self.catalogs.contains_key(locale) {
            return;
        }
        if let Some((_, json)) = BUILTIN_CATALOGS
            .iter()
            .find(|(l, _)| *l == locale)
        {
            self.merge(locale, json, false)
                .expect("built in catalog");
        }
    }

    /// Locales whose framework texts are compiled into this build, English first.
    ///
    /// ```
    /// assert!(zeyn::I18n::builtin_locales().any(|l| l == "tr"));
    /// ```
    pub fn builtin_locales() -> impl Iterator<Item = &'static str> {
        BUILTIN_CATALOGS
            .iter()
            .map(|(locale, _)| *locale)
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
        if offered {
            self.load_builtin(locale);
        }
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| format!("catalog `{locale}` is not valid JSON: {e}"))?;
        let catalog = self
            .catalogs
            .entry(locale.to_string())
            .or_default();
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
            .find_map(|l| {
                self.catalogs
                    .get(l)
                    .and_then(|c| c.get(key))
            })
            .map(String::as_str)
    }

    /// Resolves a text in a locale. Missing keys show the fallback, or the key itself.
    pub fn text(&self, locale: &str, text: &Text) -> String {
        match text {
            Text::Literal(s) => s.to_string(),
            Text::Key {
                key,
                fallback,
                args,
            } => {
                let template = self
                    .lookup(locale, key)
                    .or(fallback.as_deref())
                    .unwrap_or(key);
                let values: Vec<(&str, String)> = args
                    .iter()
                    .map(|(n, v)| (*n, self.text(locale, v)))
                    .collect();
                format_message(locale, template, &values)
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
                    .find_map(|p| {
                        p.trim()
                            .strip_prefix("q=")
                            .and_then(|q| q.parse::<f32>().ok())
                    })
                    .unwrap_or(1.0);
                (!tag.is_empty() && q > 0.0).then_some((q, tag))
            })
            .collect();
        wanted.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, tag) in wanted {
            if let Some(l) = self
                .locales
                .iter()
                .find(|l| l.eq_ignore_ascii_case(tag))
            {
                return l.clone();
            }
            if let Some(l) = self
                .locales
                .iter()
                .find(|l| primary(l).eq_ignore_ascii_case(primary(tag)))
            {
                return l.clone();
            }
        }
        self.default.clone()
    }

    /// Keys that exist in the default locale but are missing in another offered locale.
    pub fn missing_keys(&self) -> Vec<(String, String)> {
        let Some(base) = self.catalogs.get(&self.default) else {
            return Vec::new();
        };
        let mut missing = Vec::new();
        for locale in self
            .locales
            .iter()
            .filter(|l| **l != self.default)
        {
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

fn flatten(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut HashMap<String, String>,
) -> Result<(), String> {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
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
