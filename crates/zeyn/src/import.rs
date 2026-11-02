//! CSV and XLSX import: preview, validation of every row with the model's own rules, a row error report
//! and an import through an action.
//!
//! ```ignore
//! #[action(permission = "crm.customer.import", audit)]
//! pub async fn import_customers(rows: Vec<Valid<Customer>>, store: Customers) -> Result<usize> { ... }
//!
//! Screen::new("Import").body(Import::of(import_customers).max_rows(2000))
//! ```

use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::form::Form;
use crate::i18n::Text;
use crate::model::{FieldMeta, Model, Valid, ValidationErrors};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use crate::value::FieldKind;
use serde::Serialize;
use serde_json::{Map, Value as Json, json};
use std::marker::PhantomData;

/// Rows an import takes unless set with `max_rows`.
pub const DEFAULT_MAX_ROWS: usize = 5_000;
/// Largest row limit an import may set.
pub const HARD_MAX_ROWS: usize = 50_000;
/// Largest CSV text accepted, in bytes.
pub const MAX_CSV_BYTES: usize = 8 * 1024 * 1024;
const PREVIEW_ROWS: usize = 10;
#[cfg(feature = "xlsx")]
const ACCEPT: &str = ".csv,text/csv,.xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
#[cfg(not(feature = "xlsx"))]
const ACCEPT: &str = ".csv,text/csv";
const MAX_ERRORS_SHOWN: usize = 200;

/// Why a CSV text could not be read at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CsvError {
    /// A quoted value is not closed.
    UnclosedQuote,
}

/// Reads CSV text: a UTF-8 byte order mark is dropped, the separator (`,`, `;` or tab) is taken
/// from the header line, values may be quoted with `"` (doubled inside), quoted values may hold
/// separators and line breaks, and both `\r\n` and `\n` end a row. Empty lines are skipped.
///
/// ```
/// let rows = zeyn::parse_csv("\u{feff}ad;şehir\r\n\"Öz; Ltd\";İzmir\n\"a \"\"b\"\"\";\"x\ny\"\n").unwrap();
/// assert_eq!(rows, vec![vec!["ad", "şehir"], vec!["Öz; Ltd", "İzmir"], vec!["a \"b\"", "x\ny"]]);
/// ```
pub fn parse_csv(text: &str) -> std::result::Result<Vec<Vec<String>>, CsvError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let sep = separator(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => quoted = false,
                _ => cell.push(c),
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => quoted = true,
            c if c == sep => row.push(std::mem::take(&mut cell)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                row.push(std::mem::take(&mut cell));
                if row.iter().any(|v| !v.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            _ => cell.push(c),
        }
    }
    if quoted {
        return Err(CsvError::UnclosedQuote);
    }
    row.push(cell);
    if row.iter().any(|v| !v.trim().is_empty()) {
        rows.push(row);
    }
    Ok(rows)
}

/// The separator that occurs most often outside quotes in the first line; `,` on a tie.
fn separator(text: &str) -> char {
    let mut counts = [(',', 0usize), (';', 0), ('\t', 0)];
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => break,
            _ if !quoted => {
                if let Some(entry) = counts.iter_mut().find(|(s, _)| *s == c) {
                    entry.1 += 1;
                }
            }
            _ => {}
        }
    }
    counts.iter().max_by_key(|(s, n)| (*n, *s == ',')).map(|(s, _)| *s).unwrap_or(',')
}

/// Header text in a form that matches across case and Turkish dotted and dotless i.
fn fold(s: &str) -> String {
    s.trim().chars().map(|c| if matches!(c, 'İ' | 'I' | 'ı') { 'i' } else { c }).collect::<String>().to_lowercase()
}

/// One problem in one row. `row` counts data rows from 1; the header is not counted.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RowError {
    pub row: usize,
    pub field: String,
    pub message: String,
}

/// Result of reading and validating a CSV file.
pub struct Checked<M> {
    pub columns: Vec<&'static FieldMeta>,
    pub valid: Vec<(usize, Valid<M>)>,
    pub errors: Vec<RowError>,
    pub total: usize,
    raw: Vec<Vec<String>>,
}

impl<M> Checked<M> {
    /// Number of rows with at least one error.
    pub fn invalid(&self) -> usize {
        self.total - self.valid.len()
    }
}

fn general(ctx: &Ctx, key: &str, args: &[(&'static str, String)]) -> Error {
    let mut text = Text::key(format!("zeyn.import.{key}"));
    for (k, v) in args {
        text = text.arg(*k, v.clone());
    }
    let mut errors = ValidationErrors::default();
    errors.add_general(ctx.t(&text));
    Error::Validation(errors)
}

/// Fields a CSV may fill: what a form would take, never keys or row versions.
fn importable<M: Model>() -> impl Iterator<Item = &'static FieldMeta> {
    M::fields().iter().filter(|m| !m.key && !m.readonly && !m.version)
}

fn bool_text(cell: &str) -> Option<bool> {
    match fold(cell).as_str() {
        "true" | "1" | "yes" | "y" | "x" | "evet" | "e" | "doğru" => Some(true),
        "false" | "0" | "no" | "n" | "" | "hayir" | "hayır" | "h" | "yanliş" | "yanlış" => Some(false),
        _ => None,
    }
}

/// A file to import: CSV text, or XLSX bytes with the `xlsx` feature.
#[derive(Clone, Debug)]
pub enum ImportFile {
    Csv(String),
    #[cfg(feature = "xlsx")]
    Xlsx(Vec<u8>),
}

/// Reads a CSV file and validates every row as a form of `M` would, in the user's language and
/// time zone. Columns are matched by field name or label, in any case.
pub fn check_csv<M: Model>(ctx: &Ctx, csv: &str, max_rows: usize) -> Result<Checked<M>> {
    if csv.len() > MAX_CSV_BYTES {
        return Err(general(ctx, "too_large", &[]));
    }
    let mut rows = parse_csv(csv).map_err(|_| general(ctx, "bad_quotes", &[]))?;
    if rows.len() < 2 {
        return Err(general(ctx, "empty", &[]));
    }
    let header = rows.remove(0);
    if rows.len() > max_rows {
        return Err(general(ctx, "too_many", &[("max", max_rows.to_string())]));
    }
    let columns = columns_of::<M>(ctx, &header)?;
    Ok(check_rows(ctx, columns, rows))
}

/// Reads a CSV or XLSX file and validates every row as [`check_csv`] does. XLSX cells keep their
/// types: numbers, dates and booleans are read as such, whatever the user's locale.
pub fn check_file<M: Model>(ctx: &Ctx, file: &ImportFile, max_rows: usize) -> Result<Checked<M>> {
    match file {
        ImportFile::Csv(csv) => check_csv(ctx, csv, max_rows),
        #[cfg(feature = "xlsx")]
        ImportFile::Xlsx(bytes) => {
            use crate::xlsx::XlsxReadError;
            let (header, cells) = crate::xlsx::read(bytes, max_rows).map_err(|e| match e {
                XlsxReadError::TooLarge => general(ctx, "too_large", &[]),
                XlsxReadError::TooMany => general(ctx, "too_many", &[("max", max_rows.to_string())]),
                XlsxReadError::Unreadable => general(ctx, "bad_file", &[]),
            })?;
            if cells.is_empty() {
                return Err(general(ctx, "empty", &[]));
            }
            let columns = columns_of::<M>(ctx, &header)?;
            let locale = ctx.locale();
            let rows = cells
                .iter()
                .map(|row| {
                    row.iter()
                        .enumerate()
                        .map(|(i, cell)| match columns.get(i) {
                            Some(m) => crate::xlsx::cell_text(cell, &m.kind, locale),
                            None if *cell == crate::xlsx::XlsxCell::Empty => String::new(),
                            None => "x".to_string(),
                        })
                        .collect()
                })
                .collect();
            Ok(check_rows(ctx, columns, rows))
        }
    }
}

/// The importable field each header cell names, by field name or label in any case.
fn columns_of<M: Model>(ctx: &Ctx, header: &[String]) -> Result<Vec<&'static FieldMeta>> {
    let mut columns = Vec::new();
    for name in header {
        let key = fold(name);
        let meta = importable::<M>()
            .find(|m| fold(m.name) == key || fold(m.label) == key || fold(&ctx.t(&m.label_text())) == key);
        match meta {
            Some(m) if !columns.iter().any(|c: &&FieldMeta| c.name == m.name) => columns.push(m),
            _ => return Err(general(ctx, "unknown_column", &[("name", name.trim().to_string())])),
        }
    }
    Ok(columns)
}

/// A choice cell by value, or by label in the catalog or in the user's language, so exported
/// files import again.
fn choice_value(ctx: &Ctx, options: &[crate::value::ChoiceOption], cell: &str) -> Option<&'static str> {
    let key = fold(cell);
    options
        .iter()
        .find(|o| o.value == cell.trim())
        .or_else(|| options.iter().find(|o| fold(o.label) == key || fold(&ctx.t(&Text::key_or(o.key, o.label))) == key))
        .map(|o| o.value)
}

/// Validates every data row as a form of `M` would.
fn check_rows<M: Model>(ctx: &Ctx, columns: Vec<&'static FieldMeta>, rows: Vec<Vec<String>>) -> Checked<M> {
    let form = Form::auto(M::default());
    let mut valid = Vec::new();
    let mut errors = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let mut values = Map::new();
        for (meta, cell) in columns.iter().zip(row.iter().chain(std::iter::repeat(&String::new()))) {
            let value = match meta.kind {
                FieldKind::Bool => bool_text(cell).map(Json::Bool).unwrap_or_else(|| Json::String(cell.clone())),
                FieldKind::Choice(options) => Json::String(
                    choice_value(ctx, options, cell).map_or_else(|| cell.trim().to_string(), str::to_string),
                ),
                _ => Json::String(cell.trim().to_string()),
            };
            values.insert(meta.name.to_string(), value);
        }
        if row.len() > columns.len() && row[columns.len()..].iter().any(|c| !c.trim().is_empty()) {
            errors.push(RowError {
                row: i + 1,
                field: String::new(),
                message: ctx.t(&Text::key("zeyn.import.extra_cells")),
            });
            continue;
        }
        match form.read_for(&values, ctx) {
            Ok(v) => valid.push((i + 1, v)),
            Err(e) => errors.extend(e.errors.iter().map(|e| RowError {
                row: i + 1,
                field: e.field.to_string(),
                message: ctx.t(&e.message),
            })),
        }
    }
    Checked { columns, valid, errors, total: rows.len(), raw: rows }
}

/// Error report as CSV (`row;field;message`), safe to open in a spreadsheet.
pub fn error_report(ctx: &Ctx, errors: &[RowError]) -> String {
    let cell = |v: &str| {
        let mut v = v.to_string();
        if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            v.insert(0, '\'');
        }
        if v.contains([';', '"', '\n', '\r']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v }
    };
    let head = ["row", "field", "message"].map(|k| cell(&ctx.t(&Text::key(format!("zeyn.import.report_{k}")))));
    let mut out = format!("\u{feff}{}\r\n", head.join(";"));
    for e in errors {
        out.push_str(&format!("{};{};{}\r\n", e.row, cell(&e.field), cell(&e.message)));
    }
    out
}

/// An import component on a screen, as the server sees it.
pub trait ImportSource: Send + Sync {
    fn action(&self) -> ActionMeta;

    /// Reads and validates the file and describes the result. Nothing is saved.
    fn preview(&self, ctx: &Ctx, file: &ImportFile) -> Result<Json>;

    /// Reads and validates the file again and imports through the action.
    fn commit(&self, ctx: Ctx, file: ImportFile) -> BoxFuture<Result<Json>>;
}

/// CSV import of `M` rows through an action that takes `Vec<Valid<M>>`.
pub struct Import<M, A> {
    max_rows: usize,
    all_or_nothing: bool,
    _types: PhantomData<fn() -> (M, A)>,
}

impl<M: Model, A: Action<Input = Vec<Valid<M>>>> Import<M, A> {
    /// Import that sends the valid rows to `action`; rows with errors are skipped and reported.
    pub fn of(_action: A) -> Self {
        Import { max_rows: DEFAULT_MAX_ROWS, all_or_nothing: false, _types: PhantomData }
    }

    /// Refuses files with more rows. 5000 unless set; never more than 50000.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = rows.clamp(1, HARD_MAX_ROWS);
        self
    }

    /// Imports nothing unless every row is valid.
    pub fn all_or_nothing(mut self) -> Self {
        self.all_or_nothing = true;
        self
    }
}

impl<M: Model, A> ImportSource for Import<M, A>
where
    A: Action<Input = Vec<Valid<M>>>,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn preview(&self, ctx: &Ctx, file: &ImportFile) -> Result<Json> {
        let checked = check_file::<M>(ctx, file, self.max_rows)?;
        let mut table = String::from(r#"<table class="z-import-preview"><thead><tr><th scope="col">#</th>"#);
        for m in &checked.columns {
            table.push_str(&format!(r#"<th scope="col">{}</th>"#, escape(&ctx.t(&m.label_text()))));
        }
        table.push_str("</tr></thead><tbody>");
        for (i, row) in checked.raw.iter().take(PREVIEW_ROWS).enumerate() {
            let bad = checked.errors.iter().any(|e| e.row == i + 1);
            table.push_str(&format!(
                r#"<tr{}><th scope="row">{}</th>"#,
                if bad { r#" class="z-row-danger""# } else { "" },
                i + 1
            ));
            for (m, cell) in checked.columns.iter().zip(row.iter().chain(std::iter::repeat(&String::new()))) {
                let shown = if m.mask.is_some() && !cell.trim().is_empty() {
                    crate::privacy::masked_text(m, cell)
                } else {
                    cell.clone()
                };
                table.push_str(&format!("<td>{}</td>", escape(&shown)));
            }
            table.push_str("</tr>");
        }
        table.push_str("</tbody></table>");
        let invalid = checked.invalid();
        let summary = Text::key("zeyn.import.summary")
            .arg("total", checked.total.to_string())
            .arg("valid", checked.valid.len().to_string())
            .arg("invalid", invalid.to_string());
        Ok(json!({
            "total": checked.total,
            "valid": checked.valid.len(),
            "invalid": invalid,
            "summary": ctx.t(&summary),
            "preview": table,
            "errors": checked.errors.iter().take(MAX_ERRORS_SHOWN).collect::<Vec<_>>(),
            "report": if checked.errors.is_empty() { Json::Null } else { Json::String(error_report(ctx, &checked.errors)) },
            "can_import": !checked.valid.is_empty() && (!self.all_or_nothing || invalid == 0),
        }))
    }

    fn commit(&self, ctx: Ctx, file: ImportFile) -> BoxFuture<Result<Json>> {
        let checked = check_file::<M>(&ctx, &file, self.max_rows);
        let all_or_nothing = self.all_or_nothing;
        Box::pin(async move {
            let checked = checked?;
            if checked.valid.is_empty() || (all_or_nothing && checked.invalid() > 0) {
                let key = if checked.valid.is_empty() { "nothing_valid" } else { "fix_first" };
                return Err(general(&ctx, key, &[]));
            }
            let skipped = checked.invalid();
            let rows: Vec<Valid<M>> = checked.valid.into_iter().map(|(_, v)| v).collect();
            let count = rows.len();
            let output = A::call(ctx.clone(), rows).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            let message =
                Text::key("zeyn.import.imported").arg("count", count.to_string()).arg("skipped", skipped.to_string());
            Ok(json!({ "imported": count, "skipped": skipped, "message": ctx.t(&message) }))
        })
    }
}

impl<M: Model, A> Component for Import<M, A>
where
    A: Action<Input = Vec<Valid<M>>>,
    A::Output: Outcome,
{
    fn render(&self, out: &mut String, r: &Render) {
        let scope = r.scope;
        let text = |k: &str| r.text(&Text::key(format!("zeyn.import.{k}")));
        let columns: Vec<String> = importable::<M>().map(|m| r.label(m)).collect();
        let hint = r.text(
            &Text::key("zeyn.import.hint").arg("columns", columns.join(", ")).arg("max", self.max_rows.to_string()),
        );
        out.push_str(&format!(
            r#"<div class="z-import" data-zeyn-import data-testid="{scope}.import"><div class="z-field"><label for="{scope}-import-file">{file}</label><input type="file" id="{scope}-import-file" accept="{ACCEPT}" data-testid="{scope}.import.file" aria-describedby="{scope}-import-hint"><p class="z-hint" id="{scope}-import-hint">{hint}</p></div><p class="z-import-status" role="status" aria-live="polite" data-zeyn-import-status></p><div data-zeyn-import-preview></div><div class="z-form-actions"><button type="button" class="z-button" data-zeyn-import-report hidden>{report}</button><button type="button" class="z-button z-primary" data-zeyn-import-commit data-testid="{scope}.import.commit" disabled>{commit}</button></div></div>"#,
            file = text(if cfg!(feature = "xlsx") { "file_any" } else { "file" }),
            report = text("report"),
            commit = text(if self.all_or_nothing { "commit_all" } else { "commit" }),
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "import",
            "model": M::NAME,
            "action": A::META,
            "max_rows": self.max_rows,
            "all_or_nothing": self.all_or_nothing,
            "columns": importable::<M>().map(|m| m.name).collect::<Vec<_>>(),
        })
    }

    fn import(&self) -> Option<&dyn ImportSource> {
        Some(self)
    }
}

impl<M: Model, A> From<Import<M, A>> for Node
where
    A: Action<Input = Vec<Valid<M>>>,
    A::Output: Outcome,
{
    fn from(i: Import<M, A>) -> Self {
        Node::new(i)
    }
}
