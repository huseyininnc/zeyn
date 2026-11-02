//! XLSX export and import, behind the `xlsx` Cargo feature.
//!
//! Export writes a worksheet row by row in constant memory mode: rows go to a temporary file as
//! they arrive and the workbook is zipped straight into the response at the end, so RAM does not
//! grow with the row count. Cells keep their types: numbers, money with a currency format, dates
//! and times as real Excel dates, and booleans.
//!
//! Import reads the first worksheet cell by cell and stops at the row limit, after checking how
//! far the file would unpack.

use crate::grid::ExportColumn;
use crate::i18n::NumberFormat;
use crate::time::{Date, LocalDateTime, Time};
use crate::value::{Currency, FieldKind, Value};
use rust_xlsxwriter::{ExcelDateTime, Format, Workbook, Worksheet, XlsxError};
use std::io::{Read, Seek, Write};

/// Largest XLSX file accepted for import, in bytes.
pub const MAX_XLSX_BYTES: usize = 8 * 1024 * 1024;
/// Most bytes the parts of an imported workbook may unpack to.
const MAX_UNPACKED: u64 = 64 * 1024 * 1024;
/// Most parts an imported workbook may hold.
const MAX_PARTS: usize = 1_000;
/// Most characters an Excel cell holds.
const MAX_CELL_CHARS: usize = 32_767;
/// Widest column an import reads; cells further right count as extra cells.
const MAX_COLUMNS: usize = 512;
/// Largest shared strings part accepted, unpacked: every text of a workbook is held in memory
/// while it is read.
const MAX_SHARED_STRINGS: u64 = 16 * 1024 * 1024;

/// Number formats of one export, in the user's locale.
struct Formats {
    head: Format,
    money: [Format; 3],
    decimal: Format,
    date: Format,
    time: Format,
    local: Format,
}

impl Formats {
    fn new(locale: &str) -> Formats {
        let first = NumberFormat::for_locale(locale).symbol_first;
        let money = |c: Currency| {
            let s = c.symbol();
            let code = if first {
                format!("\"{s}\"#,##0.00;-\"{s}\"#,##0.00")
            } else {
                format!("#,##0.00 \"{s}\";-#,##0.00 \"{s}\"")
            };
            Format::new().set_num_format(code)
        };
        let (order, sep, pad) = Date::pattern(locale);
        let (d, m) = if pad { ("dd", "mm") } else { ("d", "m") };
        let date = match order {
            "dmy" => format!("{d}{sep}{m}{sep}yyyy"),
            "mdy" => format!("{m}{sep}{d}{sep}yyyy"),
            _ => "yyyy-mm-dd".to_string(),
        };
        let time = if Time::twelve_hour(locale) { "h:mm:ss AM/PM" } else { "hh:mm:ss" };
        Formats {
            head: Format::new().set_bold(),
            money: [money(Currency::Try), money(Currency::Usd), money(Currency::Eur)],
            decimal: Format::new().set_num_format("#,##0.00"),
            local: Format::new().set_num_format(format!("{date} {time}")),
            date: Format::new().set_num_format(date),
            time: Format::new().set_num_format(time),
        }
    }
}

/// A worksheet written row by row in constant memory mode, then zipped into any writer.
///
/// ```
/// use zeyn::{Money, Value, ExportColumn, XlsxSheet};
/// let columns = [ExportColumn { label: "Price".into(), width: Some(120), numeric: true }];
/// let mut sheet = XlsxSheet::new(&columns, "en").unwrap();
/// sheet.write_rows(&[vec![Value::Money(Money::tl(1250))]]).unwrap();
/// let mut out = Vec::new();
/// sheet.finish(&mut out).unwrap();
/// assert!(out.starts_with(b"PK"));
/// ```
pub struct XlsxSheet {
    workbook: Workbook,
    sheet: Worksheet,
    formats: Formats,
    row: u32,
}

impl XlsxSheet {
    /// A sheet with a bold, frozen header row of these columns. Number and date formats follow
    /// `locale`.
    pub fn new(columns: &[ExportColumn], locale: &str) -> Result<XlsxSheet, XlsxError> {
        let mut workbook = Workbook::new();
        let mut sheet = workbook.new_worksheet_with_constant_memory()?;
        let formats = Formats::new(locale);
        for (i, c) in columns.iter().enumerate() {
            let col = i as u16;
            if let Some(w) = c.width {
                sheet.set_column_width_pixels(col, w)?;
            }
            sheet.write_string_with_format(0, col, clip(&c.label), &formats.head)?;
        }
        sheet.set_freeze_panes(1, 0)?;
        Ok(XlsxSheet { workbook, sheet, formats, row: 1 })
    }

    /// Writes rows under the ones already written. Choices should already be labels and moments
    /// already in the user's time zone (as [`crate::GridSource::export_values`] gives them).
    /// Text is always written as text, so a value that looks like a formula stays text.
    pub fn write_rows(&mut self, rows: &[Vec<Value>]) -> Result<(), XlsxError> {
        for values in rows {
            for (i, v) in values.iter().enumerate() {
                self.write(i as u16, v)?;
            }
            self.row += 1;
        }
        Ok(())
    }

    fn write(&mut self, col: u16, value: &Value) -> Result<(), XlsxError> {
        let (row, f, s) = (self.row, &self.formats, &mut self.sheet);
        match value {
            Value::Null => {}
            Value::Bool(b) => {
                s.write_boolean(row, col, *b)?;
            }
            Value::Int(n) => {
                s.write_number(row, col, *n as f64)?;
            }
            Value::Decimal(x) => {
                s.write_number_with_format(row, col, *x, &f.decimal)?;
            }
            Value::Number(d) => {
                s.write_number(row, col, d.to_f64())?;
            }
            Value::Money(m) => {
                let format = &f.money[m.currency as usize];
                s.write_number_with_format(row, col, m.minor as f64 / 100.0, format)?;
            }
            Value::Text(t) => {
                s.write_string(row, col, clip(t))?;
            }
            Value::Choice(c) => {
                s.write_string(row, col, *c)?;
            }
            Value::Date(d) => {
                s.write_datetime_with_format(row, col, excel_date(d)?, &f.date)?;
            }
            Value::Time(t) => {
                let t = ExcelDateTime::from_hms(t.hour().into(), t.minute(), t.second())?;
                s.write_datetime_with_format(row, col, t, &f.time)?;
            }
            Value::LocalDateTime(l) => {
                let at = excel_date(&l.date)?.and_hms(l.time.hour().into(), l.time.minute(), l.time.second())?;
                s.write_datetime_with_format(row, col, at, &f.local)?;
            }
            Value::DateTime(d) => {
                let l = d.local(crate::time::TimeZone::default());
                let at = excel_date(&l.date)?.and_hms(l.time.hour().into(), l.time.minute(), l.time.second())?;
                s.write_datetime_with_format(row, col, at, &f.local)?;
            }
        }
        Ok(())
    }

    /// Rows written so far, without the header.
    pub fn rows(&self) -> u32 {
        self.row - 1
    }

    /// Zips the workbook into `out`. The rows are read back from the temporary file in chunks.
    pub fn finish<W: Write + Send>(mut self, out: W) -> Result<(), XlsxError> {
        self.workbook.push_worksheet(self.sheet);
        self.workbook.save_to_writer(out)
    }
}

fn excel_date(d: &Date) -> Result<ExcelDateTime, XlsxError> {
    ExcelDateTime::from_ymd(d.year().clamp(1900, 9999) as u16, d.month(), d.day())
}

fn clip(text: &str) -> &str {
    match text.char_indices().nth(MAX_CELL_CHARS) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

/// Why an XLSX file could not be imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XlsxReadError {
    /// Not a workbook, password protected, or broken.
    Unreadable,
    /// Bigger than [`MAX_XLSX_BYTES`] or would unpack to too much.
    TooLarge,
    /// More data rows than allowed.
    TooMany,
}

/// One cell read from a workbook.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum XlsxCell {
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
    /// Year, month, day, hour, minute, second, and whether the cell is a time of day only.
    Moment(u16, u8, u8, u8, u8, u8, bool),
}

/// Header and data rows of the first worksheet: at most `max_rows` data rows, empty rows skipped.
/// The header is cut at [`MAX_COLUMNS`] and data rows at the header's width; anything further
/// right becomes one extra cell. A header with more than `max_columns` cells ends the reading
/// with no data rows, since no import can take it, so memory stays at rows times header width.
pub(crate) fn read(
    bytes: &[u8],
    max_rows: usize,
    max_columns: usize,
) -> Result<(Vec<String>, Vec<Vec<XlsxCell>>), XlsxReadError> {
    if bytes.len() > MAX_XLSX_BYTES {
        return Err(XlsxReadError::TooLarge);
    }
    check_unpacked(std::io::Cursor::new(bytes))?;
    use calamine::Reader;
    let mut book = calamine::Xlsx::new(std::io::Cursor::new(bytes)).map_err(|_| XlsxReadError::Unreadable)?;
    let first = book.sheet_names().into_iter().next().ok_or(XlsxReadError::Unreadable)?;
    let mut cells = book.worksheet_cells_reader(&first).map_err(|_| XlsxReadError::Unreadable)?;
    let mut rows: Vec<Vec<XlsxCell>> = Vec::new();
    let mut current: Option<(u32, Vec<XlsxCell>)> = None;
    let push = |rows: &mut Vec<Vec<XlsxCell>>, row: Vec<XlsxCell>| -> Result<(), XlsxReadError> {
        if row.iter().any(|c| *c != XlsxCell::Empty && *c != XlsxCell::Text(String::new())) {
            if rows.len() > max_rows {
                return Err(XlsxReadError::TooMany);
            }
            rows.push(row);
        }
        Ok(())
    };
    while let Some(cell) = cells.next_cell().map_err(|_| XlsxReadError::Unreadable)? {
        let (r, c) = cell.get_position();
        if current.as_ref().is_some_and(|(at, _)| *at != r) {
            let (_, row) = current.take().unwrap_or_default();
            push(&mut rows, row)?;
            if rows.len() == 1 {
                let header = &mut rows[0];
                while header.last().is_some_and(|c| *c == XlsxCell::Empty) {
                    header.pop();
                }
                if header.len() > max_columns {
                    let header = rows.remove(0).into_iter().map(|c| text_of(&c)).collect();
                    return Ok((header, Vec::new()));
                }
            }
        }
        let width = rows.first().map_or(MAX_COLUMNS, Vec::len);
        let row = &mut current.get_or_insert_with(|| (r, Vec::new())).1;
        let c = (c as usize).min(width);
        if row.len() <= c {
            row.resize(c + 1, XlsxCell::Empty);
        }
        let value = cell_of(cell.get_value());
        if row[c] == XlsxCell::Empty {
            row[c] = value;
        }
    }
    if let Some((_, row)) = current {
        push(&mut rows, row)?;
    }
    if rows.is_empty() {
        return Ok((Vec::new(), rows));
    }
    let header = rows.remove(0).into_iter().map(|c| text_of(&c)).collect();
    Ok((header, rows))
}

/// Refuses archives whose parts would unpack to more than [`MAX_UNPACKED`] bytes or that hold
/// too many parts, before anything is unpacked. The sizes are the ones the archive declares.
fn check_unpacked<R: Read + Seek>(reader: R) -> Result<(), XlsxReadError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|_| XlsxReadError::Unreadable)?;
    if zip.len() > MAX_PARTS {
        return Err(XlsxReadError::TooLarge);
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let part = zip.by_index_raw(i).map_err(|_| XlsxReadError::Unreadable)?;
        total = total.saturating_add(part.size());
        let shared = part.name().is_ok_and(|n| n.eq_ignore_ascii_case("xl/sharedStrings.xml"));
        if total > MAX_UNPACKED || (shared && part.size() > MAX_SHARED_STRINGS) {
            return Err(XlsxReadError::TooLarge);
        }
    }
    Ok(())
}

fn cell_of(data: &calamine::DataRef<'_>) -> XlsxCell {
    use calamine::DataRef as D;
    match data {
        D::Int(n) => XlsxCell::Number(*n as f64),
        D::Float(x) => XlsxCell::Number(*x),
        D::String(s) => XlsxCell::Text(s.clone()),
        D::SharedString(s) => XlsxCell::Text((*s).to_string()),
        D::Bool(b) => XlsxCell::Bool(*b),
        D::DateTime(d) if d.is_datetime() => {
            let (y, mo, da, h, mi, s, _) = d.to_ymd_hms_milli();
            XlsxCell::Moment(y, mo, da, h, mi, s, d.as_f64() < 1.0)
        }
        D::DateTime(d) => XlsxCell::Number(d.as_f64()),
        D::DateTimeIso(s) | D::DurationIso(s) => XlsxCell::Text(s.clone()),
        D::Error(e) => XlsxCell::Text(format!("#{e:?}")),
        D::Empty => XlsxCell::Empty,
    }
}

fn text_of(cell: &XlsxCell) -> String {
    match cell {
        XlsxCell::Empty => String::new(),
        XlsxCell::Text(s) => s.clone(),
        XlsxCell::Number(x) => x.to_string(),
        XlsxCell::Bool(b) => b.to_string(),
        XlsxCell::Moment(y, mo, d, h, mi, s, _) => format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}"),
    }
}

/// A cell as the text a form reads for a field of this kind in `locale`: numbers in the locale's
/// format, dates as ISO dates, wall clock times as ISO date and time (read in the user's time
/// zone), booleans as `true` or `false`.
pub(crate) fn cell_text(cell: &XlsxCell, kind: &FieldKind, locale: &str) -> String {
    match (cell, kind) {
        (XlsxCell::Number(x), FieldKind::Money) => {
            let decimal = NumberFormat::for_locale(locale).decimal;
            format!("{x:.2}").replace('.', &decimal.to_string())
        }
        (XlsxCell::Number(x), _) if x.fract() == 0.0 && x.abs() < 1e15 => format!("{}", *x as i64),
        (XlsxCell::Number(x), _) => {
            let decimal = NumberFormat::for_locale(locale).decimal;
            x.to_string().replace('.', &decimal.to_string())
        }
        (XlsxCell::Moment(y, mo, d, ..), FieldKind::Date) => format!("{y:04}-{mo:02}-{d:02}"),
        (XlsxCell::Moment(.., h, mi, s, _), FieldKind::Time) => format!("{h:02}:{mi:02}:{s:02}"),
        (XlsxCell::Moment(y, mo, d, h, mi, s, false), FieldKind::Text) if *h == 0 && *mi == 0 && *s == 0 => {
            Date::new((*y).into(), *mo, *d).map(|d| d.format(locale)).unwrap_or_default()
        }
        (XlsxCell::Moment(y, mo, d, h, mi, s, _), FieldKind::Text) => {
            match (Date::new((*y).into(), *mo, *d), Time::new(*h, *mi, *s)) {
                (Some(d), Some(t)) => LocalDateTime::new(d, t).format(locale),
                _ => String::new(),
            }
        }
        (c, _) => text_of(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_become_the_text_a_form_reads() {
        let n = XlsxCell::Number(1234.5);
        assert_eq!(cell_text(&n, &FieldKind::Money, "tr"), "1234,50");
        assert_eq!(cell_text(&n, &FieldKind::Money, "en"), "1234.50");
        assert_eq!(cell_text(&XlsxCell::Number(42.0), &FieldKind::Integer, "tr"), "42");
        assert_eq!(cell_text(&XlsxCell::Number(42.0), &FieldKind::Text, "tr"), "42");
        let m = XlsxCell::Moment(2026, 3, 9, 14, 30, 0, false);
        assert_eq!(cell_text(&m, &FieldKind::Date, "tr"), "2026-03-09");
        assert_eq!(cell_text(&m, &FieldKind::DateTime, "tr"), "2026-03-09T14:30:00");
        assert_eq!(cell_text(&m, &FieldKind::Time, "tr"), "14:30:00");
        assert_eq!(cell_text(&XlsxCell::Bool(true), &FieldKind::Bool, "tr"), "true");
    }

    #[test]
    fn long_text_is_clipped_to_what_a_cell_holds() {
        let long = "ş".repeat(MAX_CELL_CHARS + 5);
        assert_eq!(clip(&long).chars().count(), MAX_CELL_CHARS);
        assert_eq!(clip("abc"), "abc");
    }
}
