//! Masked text: a pattern such as `(999) 999 99 99` or `aa99 9999 9999 9999 9999 9999 99` that
//! the input fills while the user types. Only the raw characters (`5321234567`) are submitted and
//! stored; the server checks them against the same pattern with the field's `mask` rule.
//!
//! | Pattern character | Means |
//! |---|---|
//! | `9` | a digit |
//! | `a` | a letter |
//! | `A` | a letter, written upper case |
//! | `*` | a letter or a digit |
//! | backslash | the next character is a literal |
//! | anything else | a literal, shown but not stored |

use super::controls::Slot;
use crate::render::escape;

/// One position of a mask pattern.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tok {
    Digit,
    Letter,
    Upper,
    Any,
    Lit(char),
}

fn tokens(pattern: &str) -> Vec<Tok> {
    let mut out = Vec::with_capacity(pattern.len());
    let mut escaped = false;
    for c in pattern.chars() {
        if escaped {
            out.push(Tok::Lit(c));
            escaped = false;
            continue;
        }
        out.push(match c {
            '\\' => {
                escaped = true;
                continue;
            }
            '9' => Tok::Digit,
            'a' => Tok::Letter,
            'A' => Tok::Upper,
            '*' => Tok::Any,
            c => Tok::Lit(c),
        });
    }
    out
}

impl Tok {
    fn takes(self, c: char) -> bool {
        match self {
            Tok::Digit => c.is_ascii_digit(),
            Tok::Letter => c.is_alphabetic(),
            Tok::Upper => c.is_alphabetic() && !c.is_lowercase(),
            Tok::Any => c.is_alphanumeric(),
            Tok::Lit(_) => false,
        }
    }
}

/// Whether `raw` fills every slot of `pattern` exactly, with no literals.
///
/// ```
/// use zeyn::ui::mask_fits;
/// assert!(mask_fits("(999) 999 99 99", "5321234567"));
/// assert!(!mask_fits("(999) 999 99 99", "(532) 123 45 67"));
/// assert!(!mask_fits("(999) 999 99 99", "532123"));
/// assert!(mask_fits("AA99", "TR33"));
/// assert!(!mask_fits("AA99", "tr33"));
/// ```
pub fn mask_fits(pattern: &str, raw: &str) -> bool {
    let mut slots = tokens(pattern).into_iter().filter(|t| !matches!(t, Tok::Lit(_)));
    let mut chars = raw.chars();
    loop {
        match (slots.next(), chars.next()) {
            (None, None) => return true,
            (Some(t), Some(c)) if t.takes(c) => {}
            _ => return false,
        }
    }
}

/// The raw characters of a value typed with or without the literals: `(532) 123 45 67` gives
/// `5321234567`, and letters in `A` slots become upper case. A value that does not follow the
/// pattern is returned as typed, so the `mask` rule reports it.
///
/// ```
/// use zeyn::ui::mask_raw;
/// assert_eq!(mask_raw("(999) 999 99 99", "(532) 123 45 67"), "5321234567");
/// assert_eq!(mask_raw("(999) 999 99 99", "5321234567"), "5321234567");
/// assert_eq!(mask_raw("AA99 9999", "tr33 0001"), "TR330001");
/// ```
pub fn mask_raw(pattern: &str, typed: &str) -> String {
    let toks = tokens(pattern);
    let mut raw = String::with_capacity(typed.len());
    let mut at = 0usize;
    'chars: for c in typed.trim().chars() {
        while let Some(Tok::Lit(l)) = toks.get(at) {
            at += 1;
            if *l == c {
                continue 'chars;
            }
        }
        match toks.get(at) {
            Some(Tok::Upper) if c.is_alphabetic() => raw.extend(c.to_uppercase()),
            Some(t) if t.takes(c) => raw.push(c),
            _ if !c.is_alphanumeric() => continue,
            _ => return typed.to_string(),
        }
        at += 1;
    }
    raw
}

/// The pattern as shown in messages and placeholders: every slot as `_`.
///
/// ```
/// use zeyn::ui::mask_example;
/// assert_eq!(mask_example("(999) 999 99 99"), "(___) ___ __ __");
/// ```
pub fn mask_example(pattern: &str) -> String {
    tokens(pattern).into_iter().map(|t| if let Tok::Lit(c) = t { c } else { '_' }).collect()
}

/// Writes a masked text input, always left to right (phone numbers, IBANs and codes read the same
/// way in every language): a plain text input holding the raw value (what the form submits
/// and what works without JavaScript) that the runtime shows through the pattern.
pub(crate) fn render_mask(out: &mut String, s: &Slot, pattern: &str) {
    let text = match s.value {
        crate::value::Value::Text(t) => t.as_str(),
        _ => "",
    };
    let numeric = tokens(pattern).iter().all(|t| matches!(t, Tok::Digit | Tok::Lit(_)));
    let example = mask_example(pattern);
    out.push_str(s.label);
    out.push_str(&format!(
        r#"<div class="z-mask" data-zeyn-mask="{}" data-example="{}"><input type="text" class="z-mask-value" dir="ltr" inputmode="{}" autocomplete="off" spellcheck="false" {} value="{}"></div>"#,
        escape(pattern),
        escape(&example),
        if numeric { "numeric" } else { "text" },
        s.common,
        escape(text)
    ));
}
