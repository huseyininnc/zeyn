//! Lookups: a combobox that searches a large list on the server, for entity id fields.
//!
//! The options come from an action that takes a [`LookupQuery`] and returns a page of
//! [`LookupItem`]s. The browser calls it through `/_zeyn/data`, so the screen's access, the
//! feature's guards and the action's permission are checked on every keystroke's search. The
//! form submits plain ids; the save action must still check that the user may use them.

use super::controls::Slot;
use crate::action::{Cursor, Page};
use crate::i18n::Text;
use crate::render::{Render, attr};
use crate::value::{FieldKind, FieldType, SupportsRequired, Value};
use serde::{Deserialize, Serialize};

/// Most options one lookup search returns, whatever the client asks for.
pub const MAX_LOOKUP_PAGE: u32 = 50;

/// Most ids a client may ask labels for at once.
pub const MAX_LOOKUP_IDS: usize = 100;

/// Most columns a lookup list shows (see `Form::lookup_columns`).
pub const MAX_LOOKUP_COLUMNS: usize = 6;

/// What a lookup action receives: the text typed so far and the page wanted, or the ids whose
/// labels are needed (when a form opens with values already chosen).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LookupQuery {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub ids: Vec<i64>,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub limit: u32,
}

impl LookupQuery {
    /// Requested page size, kept between 1 and [`MAX_LOOKUP_PAGE`]. Never use the raw value.
    pub fn limit(&self) -> u32 {
        self.limit.clamp(1, MAX_LOOKUP_PAGE)
    }

    /// The ids asked for, at most [`MAX_LOOKUP_IDS`].
    pub fn ids(&self) -> &[i64] {
        &self.ids[..self.ids.len().min(MAX_LOOKUP_IDS)]
    }

    /// Filters and pages an in memory list: by ids when ids are asked for, otherwise by the text
    /// in the label or hint (ignoring case, Turkish letters included). Meant for small data and
    /// samples; databases search in SQL.
    ///
    /// ```
    /// use zeyn::ui::{LookupItem, LookupQuery};
    /// let all = vec![LookupItem::new(1, "İstanbul"), LookupItem::new(2, "Ankara")];
    /// let q = LookupQuery { text: "istan".into(), limit: 10, ..Default::default() };
    /// assert_eq!(q.apply(all).items[0].id, 1);
    /// ```
    pub fn apply(&self, items: Vec<LookupItem>) -> Page<LookupItem> {
        let needle = fold(self.text.trim());
        let matching: Vec<LookupItem> = if !self.ids.is_empty() {
            items
                .into_iter()
                .filter(|i| self.ids().contains(&i.id))
                .collect()
        } else {
            items
                .into_iter()
                .filter(|i| {
                    needle.is_empty()
                        || fold(&i.label).contains(&needle)
                        || i.hint
                            .as_deref()
                            .is_some_and(|h| fold(h).contains(&needle))
                        || i.cells
                            .iter()
                            .any(|c| fold(c).contains(&needle))
                })
                .collect()
        };
        let total = matching.len();
        let start = usize::try_from(self.offset)
            .unwrap_or(usize::MAX)
            .min(total);
        let size = if self.ids.is_empty() {
            self.limit() as usize
        } else {
            MAX_LOOKUP_IDS
        };
        let end = start.saturating_add(size).min(total);
        let page = Page::new(matching[start..end].to_vec()).total(total as u64);
        if end < total {
            page.next(Cursor(end.to_string()))
        } else {
            page
        }
    }
}

/// Lower case for searching, with Turkish dotted and dotless i treated as plain i.
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ı' | 'i' | '\u{307}' => 'i',
            c => c,
        })
        .filter(|c| *c != '\u{307}')
        .collect()
}

/// One lookup option.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LookupItem {
    pub id: i64,
    pub label: String,
    /// A second line such as a code or a city.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Values of the list's columns, for lookups made with `Form::lookup_columns`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<String>,
}

impl LookupItem {
    pub fn new(id: i64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            hint: None,
            cells: Vec::new(),
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Values of the columns of a multi column lookup, in the order of its headers. At most
    /// [`MAX_LOOKUP_COLUMNS`] are kept.
    ///
    /// ```
    /// use zeyn::ui::LookupItem;
    /// let item = LookupItem::new(7, "Ayşe Yılmaz").cells(["C-7", "Ayşe Yılmaz", "İzmir"]);
    /// assert_eq!(item.cells.len(), 3);
    /// ```
    pub fn cells<I>(mut self, cells: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.cells = cells
            .into_iter()
            .take(MAX_LOOKUP_COLUMNS)
            .map(Into::into)
            .collect();
        self
    }
}

/// Field types a single lookup can fill: entity ids, `i64` and their `Option` forms.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be filled by a lookup",
    label = "not an id field",
    note = "`Form::lookup` works on entity ids (such as `CustomerId`), i64 and their Option forms; use `Form::lookup_many` with a `Vec<i64>` field for several values"
)]
pub trait LookupValue {}
impl LookupValue for i64 {}
impl<T: LookupValue> LookupValue for Option<T> {}

/// Several ids chosen in a lookup, such as the tags of a product.
impl FieldType for Vec<i64> {
    const KIND: FieldKind = FieldKind::Custom { editor: "ids" };

    fn to_value(&self) -> Value {
        Value::Text(
            self.iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    fn __is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl SupportsRequired for Vec<i64> {}

/// Reads `3,7,9` as ids. At most 1000 ids; anything else is refused.
pub(crate) fn parse_ids(text: &str) -> Option<Vec<i64>> {
    let ids: Option<Vec<i64>> = text
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect();
    ids.filter(|v| v.len() <= 1000)
}

/// Writes a lookup: a plain text input holding the ids (what works without JavaScript and what
/// the form submits) that the runtime turns into a searchable combobox.
pub(crate) fn render_lookup(
    out: &mut String,
    r: &Render,
    s: &Slot,
    multiple: bool,
    columns: &[Text],
) {
    let value = match s.value {
        Value::Int(n) => n.to_string(),
        Value::Text(t) => t.clone(),
        _ => String::new(),
    };
    let t = |k: &str| {
        r.i18n
            .text(r.locale, &Text::key(format!("zeyn.ui.lookup.{k}")))
    };
    let texts = serde_json::json!({
        "open": t("open"),
        "no_results": t("no_results"),
        "results": t("results"),
        "remove": t("remove"),
        "more": t("more"),
        "selected": t("selected"),
        "loading": r.i18n.text(r.locale, &Text::key("zeyn.ui.loading")),
        "clear": r.i18n.text(r.locale, &Text::key("zeyn.ui.date.clear")),
    });
    let columns = if columns.is_empty() {
        String::new()
    } else {
        let headers: Vec<String> = columns
            .iter()
            .map(|c| r.i18n.text(r.locale, c))
            .collect();
        format!(r#" data-columns="{}""#, attr(&headers))
    };
    out.push_str(s.label);
    out.push_str(&format!(
        r#"<div class="z-lookup" data-zeyn-lookup="{}" data-multiple="{multiple}"{columns} data-texts="{}"><input type="text" class="z-lookup-value" inputmode="numeric" autocomplete="off" {} value="{}"></div>"#,
        s.m.name,
        attr(&texts),
        s.common,
        crate::render::escape(&value)
    ));
}
