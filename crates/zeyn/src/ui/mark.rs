//! [`Highlight`]: a text with the words of a search marked in it.

use super::layout::into_node;
use crate::render::{Render, escape_into};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};
use std::ops::Range;

into_node!(Highlight);

/// Most words of a query a [`Highlight`] looks for; the rest are ignored.
pub const MAX_HIGHLIGHT_TERMS: usize = 8;

/// Longest word of a query a [`Highlight`] looks for, in characters; a longer one is ignored.
pub const MAX_HIGHLIGHT_TERM_CHARS: usize = 64;

/// The words of a query, each as its lower case characters.
fn terms(query: &str) -> Vec<Vec<char>> {
    let mut terms: Vec<Vec<char>> = query
        .split_whitespace()
        .map(|word| {
            word.chars()
                .flat_map(char::to_lowercase)
                .collect::<Vec<char>>()
        })
        .filter(|term| !term.is_empty() && term.len() <= MAX_HIGHLIGHT_TERM_CHARS)
        .take(MAX_HIGHLIGHT_TERMS)
        .collect();
    terms.sort_by_key(|term| std::cmp::Reverse(term.len()));
    terms
}

/// Where `term` ends in `text` when it starts at its first byte, or `None`. Letters are compared
/// in lower case, and a match ends on a character of the text, never inside one.
fn match_end(text: &str, term: &[char]) -> Option<usize> {
    let mut left = term;
    for (at, c) in text.char_indices() {
        for lower in c.to_lowercase() {
            match left.split_first() {
                Some((first, rest)) if *first == lower => left = rest,
                _ => return None,
            }
        }
        if left.is_empty() {
            return Some(at + c.len_utf8());
        }
    }
    None
}

/// The byte ranges of `text` that hold a word of `query`, in order and without overlaps. Words
/// are split at white space and matched anywhere in the text without regard to case; where two
/// words match at the same place the longer one wins. At most [`MAX_HIGHLIGHT_TERMS`] words are
/// looked for.
///
/// ```
/// use zeyn::ui::highlight_ranges;
/// assert_eq!(highlight_ranges("Green tea and TEA cups", "tea"), [6..9, 14..17]);
/// assert_eq!(highlight_ranges("Çay bardağı", "ÇAY bar"), [0..4, 5..8]);
/// assert!(highlight_ranges("Green tea", "  ").is_empty());
/// ```
pub fn highlight_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let terms = terms(query);
    let mut ranges = Vec::new();
    if terms.is_empty() {
        return ranges;
    }
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let found = terms
            .iter()
            .find_map(|term| match_end(rest, term));
        match found {
            Some(len) => {
                ranges.push(at..at + len);
                at += len;
            }
            None => {
                at += rest
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8)
            }
        }
    }
    ranges
}

/// A text with the words of a search marked in it (`<mark>`), such as the name of a product in
/// a list of results. The text and the query are user data: both are escaped, and the query is
/// only compared, never written to the page. Matching ignores case and marks every place a word
/// of the query is found; an empty query marks nothing.
///
/// Matching compares lower case letters as Unicode defines them, so the Turkish pairs `I ı` and
/// `İ i` are not folded together.
///
/// ```
/// use zeyn::ui::Highlight;
/// let name = Highlight::new("Green tea, 100 g", "tea");
/// ```
pub struct Highlight {
    text: String,
    query: String,
}

impl Highlight {
    /// `text` with the words of `query` marked.
    pub fn new(text: impl Into<String>, query: impl Into<String>) -> Self {
        Highlight {
            text: text.into(),
            query: query.into(),
        }
    }
}

impl Component for Highlight {
    fn render(&self, out: &mut String, _r: &Render) {
        out.push_str(r#"<span class="z-highlight">"#);
        let mut at = 0;
        for range in highlight_ranges(&self.text, &self.query) {
            escape_into(out, &self.text[at..range.start]);
            out.push_str("<mark>");
            escape_into(out, &self.text[range.clone()]);
            out.push_str("</mark>");
            at = range.end;
        }
        escape_into(out, &self.text[at..]);
        out.push_str("</span>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "highlight", "text": self.text, "query": self.query })
    }
}
