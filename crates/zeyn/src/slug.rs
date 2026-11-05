//! URL slugs: `Slug::from_title("Çay Bardağı")` is `cay-bardagi`.

use crate::value::{FieldKind, FieldType, Value};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// A readable URL part: lowercase ASCII letters and digits, words joined by `-`, at most
/// [`Slug::MAX_LEN`] bytes. Turkish letters are mapped (`ı`, `İ` to `i`, `ş` to `s`, `ğ` to `g`,
/// `ç` to `c`, `ö` to `o`, `ü` to `u`), other accented Latin letters lose their accent, spaces
/// and punctuation become `-`, and everything else is dropped.
///
/// As a route segment (`/catalog/{slug}`) only the canonical form parses; anything else answers
/// 404, so a screen can redirect old slugs with [`crate::Screen::moved_to`].
///
/// ```
/// use zeyn::Slug;
/// assert_eq!(Slug::from_title("Çay Bardağı, 6'lı").as_str(), "cay-bardagi-6-li");
/// assert_eq!(Slug::from_title("  IŞIK & Gölge  ").as_str(), "isik-golge");
/// assert_eq!(Slug::from_title("Crème Brûlée").as_str(), "creme-brulee");
/// assert!(Slug::parse("cay-bardagi").is_some());
/// assert!(Slug::parse("Cay-Bardagi").is_none());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Slug(String);

impl Slug {
    /// Longest slug in bytes. Longer titles are cut at the last word that fits.
    pub const MAX_LEN: usize = 80;

    /// The slug of a title or name. May be empty when the title holds no letters or digits.
    pub fn from_title(title: &str) -> Slug {
        let mut out = String::with_capacity(title.len().min(Self::MAX_LEN));
        let mut gap = false;
        for c in title.chars() {
            if c == 'İ' {
                push(&mut out, &mut gap, "i");
                continue;
            }
            for c in c.to_lowercase() {
                if c.is_ascii_alphanumeric() {
                    let mut buf = [0u8; 4];
                    push(&mut out, &mut gap, c.encode_utf8(&mut buf));
                } else if c.is_whitespace() || c.is_ascii_punctuation() {
                    gap = true;
                } else if let Some(folded) = crate::text::letter(c) {
                    push(&mut out, &mut gap, folded);
                }
            }
            if out.len() > Self::MAX_LEN {
                break;
            }
        }
        if out.len() > Self::MAX_LEN {
            let cut = out[..=Self::MAX_LEN]
                .rfind('-')
                .unwrap_or(Self::MAX_LEN);
            out.truncate(cut);
        }
        while out.ends_with('-') {
            out.pop();
        }
        Slug(out)
    }

    /// The slug if `s` already is one in canonical form, else `None`.
    pub fn parse(s: &str) -> Option<Slug> {
        let slug = Slug::from_title(s);
        (!slug.0.is_empty() && slug.0 == s).then_some(slug)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Appends a word part, with a `-` first when a separator came before it.
fn push(out: &mut String, gap: &mut bool, part: &str) {
    if *gap && !out.is_empty() {
        out.push('-');
    }
    *gap = false;
    out.push_str(part);
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Slug {
    fn from(title: &str) -> Self {
        Slug::from_title(title)
    }
}

/// Reads any string and makes it a slug, so a form or JSON value is cleaned on the way in.
impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Slug::from_title(&String::deserialize(d)?))
    }
}

impl FieldType for Slug {
    const KIND: FieldKind = FieldKind::Text;
    fn to_value(&self) -> Value {
        Value::Text(self.0.clone())
    }
    fn __is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn __text(&self) -> Option<&str> {
        Some(&self.0)
    }
}

impl crate::value::SupportsRequired for Slug {}
impl crate::value::SupportsText for Slug {}

impl crate::screen::PathValue for Slug {
    fn from_segment(raw: &str) -> Option<Self> {
        Slug::parse(raw)
    }
}
