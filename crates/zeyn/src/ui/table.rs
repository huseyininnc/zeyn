//! [`Table`]: a small table of text the screen already holds.

use super::dom_id;
use super::layout::into_node;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Table);

/// Most rows a [`Table`] holds; rows added after that are left out.
pub const MAX_TABLE_ROWS: usize = 200;

/// Most columns a [`Table`] holds; cells after that are left out.
pub const MAX_TABLE_COLUMNS: usize = 12;

/// A small table of text the screen already holds: the plans of a price list, the opening hours
/// of a shop, the totals of a report. It has no paging, sorting or filters; rows that come from
/// an action, or that the user works with, belong in `DataGrid`.
///
/// The caption is required and names the table. `.head` gives the column headers (`th` with
/// `scope="col"`), and `.row_headers()` makes the first cell of every row its header
/// (`scope="row"`), so screen readers say the column and the row of each cell. `.numeric(index)`
/// lines a column of numbers up at the end, in tabular figures. A row shorter than the head is
/// filled with empty cells and a longer one is cut. The table scrolls sideways inside its own
/// box on narrow screens, and the box takes focus so the keyboard can scroll it.
///
/// ```
/// use zeyn::ui::Table;
/// let hours = Table::new("Opening hours")
///     .head(["Day", "Opens", "Closes"])
///     .row(["Monday to Friday", "09:00", "18:00"])
///     .row(["Saturday", "10:00", "14:00"])
///     .row_headers()
///     .numeric(1)
///     .numeric(2);
/// ```
pub struct Table {
    caption: Text,
    head: Vec<Text>,
    rows: Vec<Vec<Text>>,
    numeric: Vec<usize>,
    row_headers: bool,
    striped: bool,
}

fn cells<I, T>(cells: I) -> Vec<Text>
where
    I: IntoIterator<Item = T>,
    T: Into<Text>,
{
    cells
        .into_iter()
        .take(MAX_TABLE_COLUMNS)
        .map(Into::into)
        .collect()
}

impl Table {
    /// A table named by its caption.
    pub fn new(caption: impl Into<Text>) -> Self {
        Table {
            caption: caption.into(),
            head: Vec::new(),
            rows: Vec::new(),
            numeric: Vec::new(),
            row_headers: false,
            striped: false,
        }
    }

    /// The column headers, at most [`MAX_TABLE_COLUMNS`].
    pub fn head<I, T>(mut self, head: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        self.head = cells(head);
        self
    }

    /// Adds a row of cells. At most [`MAX_TABLE_ROWS`] rows.
    pub fn row<I, T>(mut self, row: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        if self.rows.len() < MAX_TABLE_ROWS {
            self.rows.push(cells(row));
        }
        self
    }

    /// Lines the column at `index` (from 0) up at the end, for numbers.
    pub fn numeric(mut self, index: usize) -> Self {
        if index < MAX_TABLE_COLUMNS && !self.numeric.contains(&index) {
            self.numeric.push(index);
        }
        self
    }

    /// Makes the first cell of every row the header of that row.
    pub fn row_headers(mut self) -> Self {
        self.row_headers = true;
        self
    }

    /// Shades every second row.
    pub fn striped(mut self) -> Self {
        self.striped = true;
        self
    }

    /// How many columns show: those of the head, or of the longest row without one.
    fn columns(&self) -> usize {
        if self.head.is_empty() {
            self.rows
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0)
        } else {
            self.head.len()
        }
    }

    fn class(&self, column: usize) -> &'static str {
        if self.numeric.contains(&column) {
            r#" class="z-table-num""#
        } else {
            ""
        }
    }
}

impl Component for Table {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "table");
        let columns = self.columns();
        out.push_str(&format!(
            r#"<div class="z-table-scroll" role="group" aria-labelledby="{id}" tabindex="0"><table class="z-table{}"><caption class="z-table-caption" id="{id}">{}</caption>"#,
            if self.striped { " z-table-striped" } else { "" },
            r.text(&self.caption)
        ));
        if !self.head.is_empty() {
            out.push_str("<thead><tr>");
            for (i, title) in self.head.iter().enumerate() {
                out.push_str(&format!(
                    r#"<th scope="col"{}>{}</th>"#,
                    self.class(i),
                    r.text(title)
                ));
            }
            out.push_str("</tr></thead>");
        }
        out.push_str("<tbody>");
        for row in &self.rows {
            out.push_str("<tr>");
            for i in 0..columns {
                let text = row
                    .get(i)
                    .map(|t| r.text(t))
                    .unwrap_or_default();
                if i == 0 && self.row_headers {
                    out.push_str(&format!(r#"<th scope="row"{}>{text}</th>"#, self.class(i)));
                } else {
                    out.push_str(&format!("<td{}>{text}</td>", self.class(i)));
                }
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table></div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "table",
            "caption": self.caption,
            "head": self.head,
            "rows": self.rows,
            "numeric": self.numeric,
            "row_headers": self.row_headers,
            "striped": self.striped,
        })
    }
}
