//! [`AlphaIndex`]: a bar of letters, each a link to a screen for the items under that letter.

use super::layout::into_node;
use super::widgets::screen_href;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node, ScreenDef};
use serde_json::{Value as Json, json};

into_node!(AlphaIndex);

const LATIN: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

const TURKISH: &[&str] = &[
    "A", "B", "C", "Ç", "D", "E", "F", "G", "Ğ", "H", "I", "İ", "J", "K", "L", "M", "N", "O", "Ö",
    "P", "R", "S", "Ş", "T", "U", "Ü", "V", "Y", "Z",
];

const SPANISH: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "Ñ", "O", "P", "Q", "R",
    "S", "T", "U", "V", "W", "X", "Y", "Z",
];

const RUSSIAN: &[&str] = &[
    "А", "Б", "В", "Г", "Д", "Е", "Ё", "Ж", "З", "И", "Й", "К", "Л", "М", "Н", "О", "П", "Р", "С",
    "Т", "У", "Ф", "Х", "Ц", "Ч", "Ш", "Щ", "Ъ", "Ы", "Ь", "Э", "Ю", "Я",
];

const ARABIC: &[&str] = &[
    "ا", "ب", "ت", "ث", "ج", "ح", "خ", "د", "ذ", "ر", "ز", "س", "ش", "ص", "ض", "ط", "ظ", "ع", "غ",
    "ف", "ق", "ك", "ل", "م", "ن", "ه", "و", "ي",
];

/// The letters of a locale's alphabet in its order: Turkish (`tr`, also `az`) with `Ç Ğ I İ Ö Ş
/// Ü`, Spanish with `Ñ`, Russian, Arabic, and the Latin `A` to `Z` for every other language.
///
/// ```
/// use zeyn::ui::alphabet;
/// assert_eq!(alphabet("tr").len(), 29);
/// assert_eq!(alphabet("tr")[11], "İ");
/// assert_eq!(alphabet("en-US").len(), 26);
/// assert_eq!(alphabet("ja"), alphabet("en"));
/// ```
pub fn alphabet(locale: &str) -> &'static [&'static str] {
    match locale
        .split(['-', '_'])
        .next()
        .unwrap_or("")
    {
        "tr" | "az" => TURKISH,
        "es" => SPANISH,
        "ru" => RUSSIAN,
        "ar" => ARABIC,
        _ => LATIN,
    }
}

type ParamsFn = Box<dyn Fn(&str) -> Json + Send + Sync>;

/// A bar of letters from the user's alphabet ([`alphabet`]), each a link to a screen with the
/// letter in its params, such as a list of people whose names start with it. The current letter
/// is marked with `aria-current="page"`; `.other()` adds a `#` link for names that start with
/// anything else. The links open without a page reload and wrap onto more lines on phones.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::AlphaIndex;
///
/// #[params]
/// pub struct LetterParams {
///     pub letter: String,
/// }
///
/// #[screen(route = "/people/{letter}", public)]
/// pub fn people_by_letter(p: LetterParams) -> Screen {
///     Screen::new("People").body(
///         AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() })
///             .current(&p.letter)
///             .other(),
///     )
/// }
/// ```
pub struct AlphaIndex {
    screen: &'static str,
    params: ParamsFn,
    current: Option<String>,
    other: bool,
}

/// The letter of the `#` link that [`AlphaIndex::other`] adds.
pub const OTHER_LETTER: &str = "#";

impl AlphaIndex {
    /// Letters that open `screen` with the params `params(letter)` returns. Params of the wrong
    /// type do not compile.
    pub fn new<S: ScreenDef>(
        _screen: S,
        params: impl Fn(&str) -> S::Params + Send + Sync + 'static,
    ) -> Self {
        AlphaIndex {
            screen: S::META.name,
            params: Box::new(move |l| serde_json::to_value(params(l)).unwrap_or(Json::Null)),
            current: None,
            other: false,
        }
    }

    /// The letter of the screen being shown, marked as the current one.
    pub fn current(mut self, letter: &str) -> Self {
        self.current = (!letter.is_empty()).then(|| letter.to_string());
        self
    }

    /// Adds a last link, `#` ([`OTHER_LETTER`]), for names that start with a digit or a letter
    /// outside the alphabet.
    pub fn other(mut self) -> Self {
        self.other = true;
        self
    }
}

impl Component for AlphaIndex {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.alpha.{key}")));
        out.push_str(&format!(
            r#"<nav class="z-alpha" aria-label="{}"><ul class="z-alpha-list">"#,
            t("label")
        ));
        let alphabet = alphabet(r.locale);
        let current = self
            .current
            .as_deref()
            .map(|c| match c {
                "i" if alphabet == TURKISH => "İ".to_string(),
                "ı" if alphabet == TURKISH => "I".to_string(),
                c => c.to_uppercase(),
            });
        for letter in alphabet
            .iter()
            .copied()
            .chain(self.other.then_some(OTHER_LETTER))
        {
            let current = current.as_deref() == Some(letter);
            let named = if letter == OTHER_LETTER {
                format!(r#" aria-label="{}""#, t("other"))
            } else {
                String::new()
            };
            out.push_str(&format!(
                r#"<li><a class="z-alpha-link" href="{}" data-zeyn-base-href{}{named}>{}</a></li>"#,
                screen_href(r, self.screen, &(self.params)(letter)),
                if current {
                    r#" aria-current="page""#
                } else {
                    ""
                },
                escape(letter)
            ));
        }
        out.push_str("</ul></nav>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "alpha_index", "screen": self.screen, "current": self.current, "other": self.other })
    }
}
