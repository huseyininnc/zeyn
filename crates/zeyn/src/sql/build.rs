//! SQL text for entities and [`DataQuery`], shared by every database.
//!
//! Identifiers come only from an entity's [`FieldMeta`] list and its table name, are checked and
//! quoted. Every value is a bound parameter. A [`Dialect`] supplies the few pieces that differ
//! between databases: placeholders, quoting, date conversion, paging and column types.

use super::decode::{Arg, Col, args_of};
use crate::action::{Aggregate, Cursor, DataQuery, Error, MAX_FILTERS, PageRequest, Result};
use crate::grid::{Cond, Key, MAX_SORT, TextOp};
use crate::i18n::Text;
use crate::model::{Entity, FieldError, FieldMeta, Model, Rule, ValidationErrors};
use crate::time::{Date, DateTime, LocalDateTime, Time};
use crate::value::{FieldKind, Value};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

/// How a value converts between its column and the form Zeyn binds and reads: dates as ISO text,
/// numbers as 64 bit integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cast {
    /// Text, yes/no and choices: no conversion.
    Plain,
    /// Whole numbers, keys and money amounts.
    Int,
    /// `2026-03-31`.
    Date,
    /// `14:30:00`.
    Time,
    /// A moment in UTC: `2026-03-31T11:30:00.000Z`.
    Moment,
    /// A wall clock date and time: `2026-03-31T14:30:00`.
    Local,
    /// An exact decimal, bound as `12345E-2` and read as text with `.`.
    Decimal,
}

impl Cast {
    pub fn of(kind: &FieldKind) -> Cast {
        match kind {
            FieldKind::Id | FieldKind::Integer | FieldKind::Money => Cast::Int,
            FieldKind::Date => Cast::Date,
            FieldKind::Time => Cast::Time,
            FieldKind::DateTime => Cast::Moment,
            FieldKind::LocalDateTime => Cast::Local,
            FieldKind::Decimal => Cast::Decimal,
            _ => Cast::Plain,
        }
    }
}

/// What differs between databases. Everything else in the SQL is shared.
pub trait Dialect: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    /// Writes the placeholder of parameter `n` (from 1), converted to the column type for `cast`.
    fn param(&self, out: &mut String, n: usize, cast: Cast);

    /// Writes a checked identifier quoted.
    fn quote(&self, out: &mut String, ident: &str);

    /// Longest identifier the database accepts.
    fn max_ident(&self) -> usize;

    /// Writes `expr` converted to the form read back for `cast`.
    fn read(&self, out: &mut String, expr: &str, cast: Cast);

    /// Writes `expr` read as a floating point number.
    fn float(&self, out: &mut String, expr: &str);

    /// Writes a whole number `expr` as text, for `contains` filters on numbers.
    fn text(&self, out: &mut String, expr: &str);

    /// Limits `sql` to `limit` rows after skipping `offset`, binding both.
    fn page(&self, sql: &mut Sql, limit: i64, offset: i64);

    /// Whether the empty string is stored as NULL.
    fn empty_is_null(&self) -> bool {
        false
    }

    /// Column type for a field of `kind`. `max_len` comes from the field's `max_len` rule.
    fn column_type(&self, kind: &FieldKind, max_len: Option<usize>) -> String;

    /// Column type of a `Decimal` field with `precision` digits and `scale` decimals.
    fn decimal_type(&self, precision: u8, scale: u8) -> String {
        format!("NUMERIC({precision}, {scale})")
    }

    /// Column definition of a generated whole number key, without the name.
    fn generated_key(&self) -> &'static str;

    /// Literal for `false` in a column default.
    fn false_literal(&self) -> &'static str;

    /// Statements to run after `CREATE TABLE`, such as a key sequence.
    fn after_create(&self, _table: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Whether `INSERT` and `UPDATE` can return the written row (`RETURNING` with a select
    /// list). Without it the row is read again after the write.
    fn returning(&self) -> bool {
        true
    }
}

/// PostgreSQL 12 and later.
#[derive(Clone, Copy, Debug, Default)]
pub struct Postgres;

impl Dialect for Postgres {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn param(&self, out: &mut String, n: usize, cast: Cast) {
        let ty = match cast {
            Cast::Plain | Cast::Int => {
                out.push('$');
                out.push_str(&n.to_string());
                return;
            }
            Cast::Date => "date",
            Cast::Time => "time",
            Cast::Moment => "timestamptz",
            Cast::Local => "timestamp",
            Cast::Decimal => "numeric",
        };
        out.push_str("CAST($");
        out.push_str(&n.to_string());
        out.push_str(" AS ");
        out.push_str(ty);
        out.push(')');
    }

    fn quote(&self, out: &mut String, ident: &str) {
        out.push('"');
        out.push_str(ident);
        out.push('"');
    }

    fn max_ident(&self) -> usize {
        63
    }

    fn read(&self, out: &mut String, expr: &str, cast: Cast) {
        let (start, end) = match cast {
            Cast::Plain => ("", ""),
            Cast::Int => ("CAST(", " AS BIGINT)"),
            Cast::Date => ("to_char(", ", 'YYYY-MM-DD')"),
            Cast::Time => ("to_char(", ", 'HH24:MI:SS')"),
            Cast::Moment => ("to_char(", " AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')"),
            Cast::Local => ("to_char(", ", 'YYYY-MM-DD\"T\"HH24:MI:SS')"),
            Cast::Decimal => ("CAST(", " AS TEXT)"),
        };
        out.push_str(start);
        out.push_str(expr);
        out.push_str(end);
    }

    fn float(&self, out: &mut String, expr: &str) {
        out.push_str("CAST(");
        out.push_str(expr);
        out.push_str(" AS DOUBLE PRECISION)");
    }

    fn text(&self, out: &mut String, expr: &str) {
        out.push_str("CAST(");
        out.push_str(expr);
        out.push_str(" AS TEXT)");
    }

    fn page(&self, sql: &mut Sql, limit: i64, offset: i64) {
        sql.push(" LIMIT ");
        sql.bind(self, Arg::Int(limit), Cast::Plain);
        sql.push(" OFFSET ");
        sql.bind(self, Arg::Int(offset), Cast::Plain);
    }

    fn column_type(&self, kind: &FieldKind, max_len: Option<usize>) -> String {
        match kind {
            FieldKind::Id | FieldKind::Integer | FieldKind::Money => "BIGINT".into(),
            FieldKind::Decimal => "NUMERIC".into(),
            FieldKind::Bool => "BOOLEAN".into(),
            FieldKind::Date => "DATE".into(),
            FieldKind::Time => "TIME(0)".into(),
            FieldKind::DateTime => "TIMESTAMPTZ(3)".into(),
            FieldKind::LocalDateTime => "TIMESTAMP(0)".into(),
            FieldKind::Choice(options) => format!("VARCHAR({})", choice_len(options)),
            FieldKind::Text => max_len.map_or_else(|| "TEXT".into(), |n| format!("VARCHAR({n})")),
            FieldKind::Custom { .. } | FieldKind::Choices(_) => "TEXT".into(),
        }
    }

    fn generated_key(&self) -> &'static str {
        "BIGINT GENERATED BY DEFAULT AS IDENTITY"
    }

    fn false_literal(&self) -> &'static str {
        "FALSE"
    }
}

/// Oracle Database version the SQL must run on, like EF Core's `UseOracleSQLCompatibility`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleCompat {
    /// 11g: `ROWNUM` paging and a sequence per table for keys.
    V11,
    /// 12c and later: `OFFSET ... FETCH` paging and identity keys.
    V12,
}

/// Oracle Database. Identifiers are written upper case and quoted, at most 30 characters (the
/// limit before 12.2). The empty string is NULL, so text columns are nullable and sort with NULLs.
/// Yes/no is `NUMBER(1)`, `Time` is `VARCHAR2(8)`, moments are `TIMESTAMP(3)` in UTC.
///
/// ```
/// use zeyn::sql::{Dialect, Oracle, OracleCompat};
/// let mut out = String::new();
/// Oracle { compat: OracleCompat::V11 }.quote(&mut out, "code");
/// assert_eq!(out, "\"CODE\"");
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Oracle {
    pub compat: OracleCompat,
}

impl Oracle {
    /// Name of the key sequence of `table` on 11g: `<table>_seq`, written upper case.
    pub fn sequence(table: &str) -> String {
        format!("{table}_seq")
    }
}

impl Dialect for Oracle {
    fn name(&self) -> &'static str {
        "oracle"
    }

    fn param(&self, out: &mut String, n: usize, cast: Cast) {
        let (start, end) = match cast {
            Cast::Plain | Cast::Int | Cast::Time => ("", ""),
            Cast::Date => ("TO_DATE(", ", 'YYYY-MM-DD')"),
            Cast::Moment => ("TO_TIMESTAMP(", ", 'YYYY-MM-DD\"T\"HH24:MI:SS.FF3\"Z\"')"),
            Cast::Local => ("TO_TIMESTAMP(", ", 'YYYY-MM-DD\"T\"HH24:MI:SS')"),
            Cast::Decimal => ("TO_NUMBER(", ")"),
        };
        out.push_str(start);
        out.push(':');
        out.push_str(&n.to_string());
        out.push_str(end);
    }

    fn quote(&self, out: &mut String, ident: &str) {
        out.push('"');
        out.push_str(&ident.to_ascii_uppercase());
        out.push('"');
    }

    fn max_ident(&self) -> usize {
        30
    }

    fn read(&self, out: &mut String, expr: &str, cast: Cast) {
        let (start, end) = match cast {
            Cast::Plain | Cast::Int | Cast::Time => ("", ""),
            Cast::Date => ("TO_CHAR(", ", 'YYYY-MM-DD')"),
            Cast::Moment => ("TO_CHAR(CAST(", " AS TIMESTAMP(3)), 'YYYY-MM-DD\"T\"HH24:MI:SS.FF3\"Z\"')"),
            Cast::Local => ("TO_CHAR(", ", 'YYYY-MM-DD\"T\"HH24:MI:SS')"),
            Cast::Decimal => ("TO_CHAR(", ", 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''')"),
        };
        out.push_str(start);
        out.push_str(expr);
        out.push_str(end);
    }

    fn float(&self, out: &mut String, expr: &str) {
        out.push_str(expr);
    }

    fn text(&self, out: &mut String, expr: &str) {
        out.push_str("TO_CHAR(");
        out.push_str(expr);
        out.push(')');
    }

    /// 12c: `OFFSET :o ROWS FETCH NEXT :n ROWS ONLY`. 11g: the `ROWNUM` wrapper, which numbers
    /// the ordered rows and keeps the window; the outer `ORDER BY` keeps their order.
    fn page(&self, sql: &mut Sql, limit: i64, offset: i64) {
        match self.compat {
            OracleCompat::V12 => {
                sql.push(" OFFSET ");
                sql.bind(self, Arg::Int(offset), Cast::Plain);
                sql.push(" ROWS FETCH NEXT ");
                sql.bind(self, Arg::Int(limit), Cast::Plain);
                sql.push(" ROWS ONLY");
            }
            OracleCompat::V11 => {
                sql.wrap("SELECT * FROM (SELECT zq.*, ROWNUM zeyn_rn FROM (", ") zq WHERE ROWNUM <= ");
                sql.bind(self, Arg::Int(offset.saturating_add(limit)), Cast::Plain);
                sql.push(") WHERE zeyn_rn > ");
                sql.bind(self, Arg::Int(offset), Cast::Plain);
                sql.push(" ORDER BY zeyn_rn");
            }
        }
    }

    fn empty_is_null(&self) -> bool {
        true
    }

    fn column_type(&self, kind: &FieldKind, max_len: Option<usize>) -> String {
        match kind {
            FieldKind::Id | FieldKind::Integer | FieldKind::Money => "NUMBER(19)".into(),
            FieldKind::Decimal => "NUMBER".into(),
            FieldKind::Bool => "NUMBER(1)".into(),
            FieldKind::Date => "DATE".into(),
            FieldKind::Time => "VARCHAR2(8)".into(),
            FieldKind::DateTime => "TIMESTAMP(3)".into(),
            FieldKind::LocalDateTime => "TIMESTAMP(0)".into(),
            FieldKind::Choice(options) => format!("VARCHAR2({} CHAR)", choice_len(options)),
            FieldKind::Text => match max_len {
                Some(n) if n <= 1000 => format!("VARCHAR2({n} CHAR)"),
                _ => "VARCHAR2(4000)".into(),
            },
            FieldKind::Custom { .. } | FieldKind::Choices(_) => "CLOB".into(),
        }
    }

    fn decimal_type(&self, precision: u8, scale: u8) -> String {
        format!("NUMBER({precision}, {scale})")
    }

    fn generated_key(&self) -> &'static str {
        match self.compat {
            OracleCompat::V11 => "NUMBER(19)",
            OracleCompat::V12 => "NUMBER(19) GENERATED BY DEFAULT ON NULL AS IDENTITY",
        }
    }

    fn false_literal(&self) -> &'static str {
        "0"
    }

    fn after_create(&self, table: &str) -> Result<Vec<String>> {
        Ok(match self.compat {
            OracleCompat::V11 => vec![format!("CREATE SEQUENCE {}", ident(self, &Oracle::sequence(table))?)],
            OracleCompat::V12 => Vec::new(),
        })
    }

    fn returning(&self) -> bool {
        false
    }
}

/// Room for the longest option value of a choice, at least 20 characters.
pub(crate) fn choice_len(options: &[crate::value::ChoiceOption]) -> usize {
    options.iter().map(|o| o.value.len()).max().unwrap_or(0).max(20)
}

/// SQL text with its parameters and the types of the columns it returns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sql {
    pub text: String,
    pub args: Vec<Arg>,
    pub cols: Vec<Col>,
}

impl Sql {
    fn new() -> Sql {
        Sql { text: String::with_capacity(256), args: Vec::new(), cols: Vec::new() }
    }

    /// Appends SQL text. Never called with values.
    pub fn push(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// Appends a placeholder and binds `arg` to it.
    pub fn bind(&mut self, d: &(impl Dialect + ?Sized), arg: Arg, cast: Cast) {
        self.args.push(arg);
        d.param(&mut self.text, self.args.len(), cast);
    }

    /// Wraps the statement in `prefix ... suffix` (text only, no parameters).
    fn wrap(&mut self, prefix: &str, suffix: &str) {
        self.text.insert_str(0, prefix);
        self.text.push_str(suffix);
    }

    fn select(&mut self, d: &dyn Dialect, expr: &str, cast: Cast, col: Col) {
        if !self.cols.is_empty() {
            self.push(", ");
        }
        d.read(&mut self.text, expr, cast);
        self.push(" AS c");
        self.push(&self.cols.len().to_string());
        self.cols.push(col);
    }
}

/// Checks a table or column name: letters, digits and `_`, not starting with a digit, at most
/// the dialect's length, optionally `schema.table`.
fn checked<'a>(d: &dyn Dialect, name: &'a str) -> Result<&'a str> {
    let ok = name.split('.').count() <= 2
        && name.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= d.max_ident()
                && !part.starts_with(|c: char| c.is_ascii_digit())
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        });
    if ok {
        Ok(name)
    } else {
        Err(Error::Internal(format!(
            "`{name}` cannot be used as a {} identifier: use letters, digits and `_`, at most {} characters",
            d.name(),
            d.max_ident()
        )))
    }
}

/// Quoted identifier, checked first.
pub(crate) fn ident(d: &dyn Dialect, name: &str) -> Result<String> {
    let name = checked(d, name)?;
    let mut out = String::with_capacity(name.len() + 4);
    for (i, part) in name.split('.').enumerate() {
        if i > 0 {
            out.push('.');
        }
        d.quote(&mut out, part);
    }
    Ok(out)
}

/// Name of the column that holds a money field's currency.
pub(crate) fn currency_column(name: &str) -> String {
    format!("{name}_currency")
}

fn column(d: &dyn Dialect, f: &FieldMeta) -> Result<String> {
    ident(d, f.name)
}

fn currency(d: &dyn Dialect, f: &FieldMeta) -> Result<String> {
    ident(d, &currency_column(f.name))
}

/// The select list of every field, in field order; money adds its currency column.
fn select_fields(d: &dyn Dialect, sql: &mut Sql, fields: &[FieldMeta]) -> Result<()> {
    for f in fields {
        sql.select(d, &column(d, f)?, Cast::of(&f.kind), Col::of(&f.kind));
        if matches!(f.kind, FieldKind::Money) {
            sql.select(d, &currency(d, f)?, Cast::Plain, Col::Text);
        }
    }
    Ok(())
}

fn key_field(fields: &[FieldMeta]) -> Result<(usize, &FieldMeta)> {
    fields.iter().enumerate().find(|(_, f)| f.key).ok_or_else(|| Error::Internal("entity has no key field".into()))
}

/// `LIKE` pattern with `\`, `%` and `_` escaped.
fn like_escape(s: &str, out: &mut String) {
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
}

fn clamp(x: i128) -> i64 {
    x.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

/// Appends `FROM table` and the filters of `q` as a `WHERE` clause. Returns whether a `WHERE` was
/// written. Filters are read exactly as [`DataQuery::apply`] reads them, so memory and SQL agree.
fn from_where<E: Entity>(d: &dyn Dialect, sql: &mut Sql, q: &DataQuery) -> Result<bool> {
    let fields = E::fields();
    sql.push(" FROM ");
    sql.push(&ident(d, E::TABLE)?);
    let mut conds = Vec::new();
    for f in q.filters.iter().take(MAX_FILTERS) {
        Cond::read(f, fields, q.time_zone, &mut conds);
    }
    conds.extend(Cond::search(&q.q, fields));
    let mut first = true;
    for c in &conds {
        if let Cond::Search(cols, needle) = c {
            sql.push(if first { " WHERE (" } else { " AND (" });
            first = false;
            if cols.is_empty() {
                sql.push("1 = 0");
            }
            let mut pattern = String::with_capacity(needle.len() + 2);
            pattern.push('%');
            like_escape(needle, &mut pattern);
            pattern.push('%');
            for (n, i) in cols.iter().enumerate() {
                if n > 0 {
                    sql.push(" OR ");
                }
                sql.push("LOWER(TRANSLATE(");
                sql.push(&column(d, &fields[*i])?);
                sql.push(", 'İı', 'ii')) LIKE ");
                sql.bind(d, Arg::Text(pattern.clone()), Cast::Plain);
                sql.push(" ESCAPE '\\'");
            }
            sql.push(")");
            continue;
        }
        let i = match c {
            Cond::Text(i, ..)
            | Cond::Range(i, ..)
            | Cond::AnyOf(i, ..)
            | Cond::Is(i, ..)
            | Cond::Blank(i, ..)
            | Cond::AnyKey(i, ..) => *i,
            Cond::Search(..) => continue,
        };
        let f = &fields[i];
        if matches!(c, Cond::Text(..)) && matches!(f.kind, FieldKind::Money | FieldKind::Bool) {
            continue;
        }
        sql.push(if first { " WHERE " } else { " AND " });
        first = false;
        condition(d, sql, f, c)?;
    }
    Ok(!first)
}

fn condition(d: &dyn Dialect, sql: &mut Sql, f: &FieldMeta, c: &Cond) -> Result<()> {
    let col = column(d, f)?;
    match c {
        Cond::Text(_, op, needle) => {
            let mut expr = String::new();
            match f.kind {
                FieldKind::Id | FieldKind::Integer => d.text(&mut expr, &col),
                FieldKind::Date | FieldKind::Time | FieldKind::DateTime | FieldKind::LocalDateTime => {
                    d.read(&mut expr, &col, Cast::of(&f.kind))
                }
                _ => {
                    expr.push_str("LOWER(");
                    expr.push_str(&col);
                    expr.push(')');
                }
            }
            sql.push(&expr);
            let mut pattern = String::with_capacity(needle.len() + 2);
            match op {
                TextOp::Equals => {
                    sql.push(" = ");
                    sql.bind(d, Arg::Text(needle.clone()), Cast::Plain);
                    return Ok(());
                }
                TextOp::Contains => {
                    pattern.push('%');
                    like_escape(needle, &mut pattern);
                    pattern.push('%');
                }
                TextOp::StartsWith => {
                    like_escape(needle, &mut pattern);
                    pattern.push('%');
                }
            }
            sql.push(" LIKE ");
            sql.bind(d, Arg::Text(pattern), Cast::Plain);
            sql.push(" ESCAPE '\\'");
        }
        Cond::Range(_, lo, hi) => {
            let mut first = true;
            for (bound, upper) in [(lo, false), (hi, true)] {
                let Some(key) = bound else { continue };
                if !first {
                    sql.push(" AND ");
                }
                first = false;
                sql.push(&col);
                sql.push(if upper { " <= " } else { " >= " });
                let (arg, cast) = match *key {
                    Key::Num(h) if matches!(f.kind, FieldKind::Money) => (Arg::Int(clamp(h)), Cast::Plain),
                    Key::Num(h) if upper => (Arg::Int(clamp(h.div_euclid(100))), Cast::Plain),
                    Key::Num(h) => (Arg::Int(clamp(-((-h).div_euclid(100)))), Cast::Plain),
                    Key::Dec(v) => (Arg::Text(v.sql_text()), Cast::Decimal),
                    Key::Date(v) => (Arg::Text(v.to_string()), Cast::Date),
                    Key::Time(v) => (Arg::Text(v.to_string()), Cast::Time),
                    Key::Moment(v) => (Arg::Text(v.to_string()), Cast::Moment),
                    Key::Local(v) => (Arg::Text(v.to_string()), Cast::Local),
                };
                sql.bind(d, arg, cast);
            }
        }
        Cond::AnyOf(_, wanted) => in_list(d, sql, &col, wanted.iter().map(|w| Arg::Text(w.clone())).collect()),
        Cond::AnyKey(_, keys) => {
            let args: Vec<Arg> = if matches!(f.kind, FieldKind::Id | FieldKind::Integer) {
                keys.iter().filter_map(|k| k.trim().parse::<i64>().ok()).map(Arg::Int).collect()
            } else {
                keys.iter().map(|k| Arg::Text(k.clone())).collect()
            };
            in_list(d, sql, &col, args);
        }
        Cond::Is(_, b) => {
            sql.push(&col);
            sql.push(" = ");
            sql.bind(d, Arg::Bool(*b), Cast::Plain);
        }
        Cond::Blank(_, b) => {
            sql.push(&col);
            sql.push(if *b { " IS NULL" } else { " IS NOT NULL" });
        }
        Cond::Search(..) => {}
    }
    Ok(())
}

fn in_list(d: &dyn Dialect, sql: &mut Sql, col: &str, args: Vec<Arg>) {
    if args.is_empty() {
        sql.push("1 = 0");
        return;
    }
    sql.push(col);
    sql.push(" IN (");
    for (i, a) in args.into_iter().enumerate() {
        if i > 0 {
            sql.push(", ");
        }
        sql.bind(d, a, Cast::Plain);
    }
    sql.push(")");
}

/// One sort key: the field, its direction and whether its column can hold NULL.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OrderKey {
    pub index: usize,
    pub desc: bool,
    pub nullable: bool,
}

/// Sort keys of a query: the asked ones (known, sortable fields, each once, at most [`MAX_SORT`]),
/// then the key field, so the order is total and keyset cursors are exact.
pub(crate) fn order_keys(d: &dyn Dialect, fields: &[FieldMeta], q: &DataQuery) -> Result<Vec<OrderKey>> {
    let nullable = |f: &FieldMeta| f.optional || (d.empty_is_null() && matches!(f.kind, FieldKind::Text));
    let mut keys: Vec<OrderKey> = Vec::new();
    for s in q.sort.iter().take(MAX_SORT) {
        let Some(index) = fields.iter().position(|f| f.name == s.field) else { continue };
        if fields[index].kind.is_json() || keys.iter().any(|k| k.index == index) {
            continue;
        }
        keys.push(OrderKey { index, desc: s.desc, nullable: nullable(&fields[index]) });
    }
    let (key, _) = key_field(fields)?;
    if !keys.iter().any(|k| k.index == key) {
        keys.push(OrderKey { index: key, desc: false, nullable: false });
    }
    Ok(keys)
}

/// `ORDER BY`. NULLs come first going up and last going down, as in memory; the clause is written
/// only for columns that can hold NULL, so plain indexes still serve the sort.
fn push_order(d: &dyn Dialect, sql: &mut Sql, fields: &[FieldMeta], keys: &[OrderKey]) -> Result<()> {
    sql.push(" ORDER BY ");
    for (i, k) in keys.iter().enumerate() {
        if i > 0 {
            sql.push(", ");
        }
        sql.push(&column(d, &fields[k.index])?);
        sql.push(match (k.desc, k.nullable) {
            (false, false) => " ASC",
            (true, false) => " DESC",
            (false, true) => " ASC NULLS FIRST",
            (true, true) => " DESC NULLS LAST",
        });
    }
    Ok(())
}

/// Rows after the cursor position in the sort order: `(a > x) OR (a = x AND b > y) OR ...`, with
/// NULLs placed as [`push_order`] places them. When the first key cannot be NULL, a plain
/// `a >= x` goes in front so an index on it narrows the scan.
fn push_seek(
    d: &dyn Dialect,
    sql: &mut Sql,
    fields: &[FieldMeta],
    keys: &[OrderKey],
    values: &[Arg],
    has_where: bool,
) -> Result<()> {
    sql.push(if has_where { " AND " } else { " WHERE " });
    let cast = |k: &OrderKey| match fields[k.index].kind {
        FieldKind::Money => Cast::Plain,
        ref kind => Cast::of(kind),
    };
    if let (Some(k), Some(v)) = (keys.first(), values.first())
        && !k.nullable
        && keys.len() > 1
    {
        sql.push(&column(d, &fields[k.index])?);
        sql.push(if k.desc { " <= " } else { " >= " });
        sql.bind(d, v.clone(), cast(k));
        sql.push(" AND ");
    }
    sql.push("(");
    let mut terms = 0;
    for (n, (k, v)) in keys.iter().zip(values).enumerate() {
        let is_null = matches!(v, Arg::Null(_));
        if is_null && k.desc {
            continue;
        }
        sql.push(if terms == 0 { "(" } else { " OR (" });
        terms += 1;
        for (pk, pv) in keys[..n].iter().zip(values) {
            sql.push(&column(d, &fields[pk.index])?);
            if matches!(pv, Arg::Null(_)) {
                sql.push(" IS NULL AND ");
            } else {
                sql.push(" = ");
                sql.bind(d, pv.clone(), cast(pk));
                sql.push(" AND ");
            }
        }
        let col = column(d, &fields[k.index])?;
        if is_null {
            sql.push(&col);
            sql.push(" IS NOT NULL)");
        } else if k.desc && k.nullable {
            sql.push("(");
            sql.push(&col);
            sql.push(" < ");
            sql.bind(d, v.clone(), cast(k));
            sql.push(" OR ");
            sql.push(&col);
            sql.push(" IS NULL))");
        } else {
            sql.push(&col);
            sql.push(if k.desc { " < " } else { " > " });
            sql.bind(d, v.clone(), cast(k));
            sql.push(")");
        }
    }
    if terms == 0 {
        sql.push("1 = 0");
    }
    sql.push(")");
    Ok(())
}

/// Most characters an encoded cursor may have.
const MAX_CURSOR: usize = 8192;

#[derive(Serialize, Deserialize)]
struct RowCursor {
    o: Vec<(String, bool)>,
    v: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct GroupCursor {
    g: u64,
}

/// Cursor of an order with a masked sort key: the position only, so no value of the row is in it.
#[derive(Serialize, Deserialize)]
struct SkipCursor {
    o: Vec<(String, bool)>,
    s: u64,
}

fn order_of(fields: &[FieldMeta], keys: &[OrderKey]) -> Vec<(String, bool)> {
    keys.iter().map(|k| (fields[k.index].name.to_string(), k.desc)).collect()
}

/// Whether a sort key is a masked field. Such orders page by position, because a keyset cursor
/// would carry the raw value to the client.
fn masked_order(fields: &[FieldMeta], keys: &[OrderKey]) -> bool {
    keys.iter().any(|k| fields[k.index].mask.is_some())
}

/// Next cursor of a row page that starts at row `skip` and holds `limit` rows: a keyset cursor
/// after `last`, or a position when a sort key is masked.
pub(crate) fn next_cursor<M: Model>(d: &dyn Dialect, keys: &[OrderKey], last: &M, skip: u64, limit: usize) -> Cursor {
    let fields = M::fields();
    if masked_order(fields, keys) {
        return encode(&SkipCursor { o: order_of(fields, keys), s: skip.saturating_add(limit as u64) });
    }
    row_cursor(d, keys, last)
}

fn encode<T: Serialize>(body: &T) -> Cursor {
    Cursor(URL_SAFE_NO_PAD.encode(serde_json::to_vec(body).unwrap_or_default()))
}

fn decode<T: for<'de> Deserialize<'de>>(cursor: &Cursor) -> Result<T> {
    if cursor.0.len() > MAX_CURSOR {
        return Err(bad_cursor());
    }
    let bytes = URL_SAFE_NO_PAD.decode(cursor.0.as_bytes()).map_err(|_| bad_cursor())?;
    serde_json::from_slice(&bytes).map_err(|_| bad_cursor())
}

/// A cursor that does not belong to this list and order.
pub(crate) fn bad_cursor() -> Error {
    let mut e = ValidationErrors::default();
    e.add_general(Text::key("zeyn.error.cursor"));
    e.into()
}

/// Cursor after `row`: the sort order and the row's values of every sort key.
pub(crate) fn row_cursor<M: Model>(d: &dyn Dialect, keys: &[OrderKey], row: &M) -> Cursor {
    let fields = M::fields();
    let v = keys
        .iter()
        .map(|k| match row.value_at(k.index).unwrap_or(Value::Null) {
            Value::Null => serde_json::Value::Null,
            Value::Int(n) => n.into(),
            Value::Money(m) => m.minor.into(),
            Value::Bool(b) => b.into(),
            Value::Text(s) if s.is_empty() && d.empty_is_null() => serde_json::Value::Null,
            Value::Text(s) => s.into(),
            Value::Choice(c) => c.into(),
            Value::Date(x) => x.to_string().into(),
            Value::Time(x) => x.to_string().into(),
            Value::DateTime(x) => x.to_string().into(),
            Value::LocalDateTime(x) => x.to_string().into(),
            Value::Decimal(_) => serde_json::Value::Null,
            Value::Number(x) => x.to_string().into(),
        })
        .collect();
    encode(&RowCursor { o: order_of(fields, keys), v })
}

/// Reads a row cursor back into one bound value per sort key. A cursor made for another order, or
/// with a value its column cannot hold, is refused.
pub(crate) fn read_cursor(fields: &[FieldMeta], keys: &[OrderKey], cursor: &Cursor) -> Result<Vec<Arg>> {
    let body: RowCursor = decode(cursor)?;
    let same = body.o.len() == keys.len()
        && body.v.len() == keys.len()
        && body.o.iter().zip(keys).all(|((name, desc), k)| name == fields[k.index].name && *desc == k.desc);
    if !same {
        return Err(bad_cursor());
    }
    body.v
        .into_iter()
        .zip(keys)
        .map(|(v, k)| {
            let f = &fields[k.index];
            let text = |v: &serde_json::Value| v.as_str().filter(|s| s.chars().count() <= 4000).map(str::to_string);
            let arg = match (&f.kind, &v) {
                (kind, serde_json::Value::Null) if k.nullable => Some(Arg::Null(Col::of(kind))),
                (FieldKind::Id | FieldKind::Integer | FieldKind::Money, _) => v.as_i64().map(Arg::Int),
                (FieldKind::Bool, _) => v.as_bool().map(Arg::Bool),
                (FieldKind::Date, _) => text(&v).and_then(|s| Date::parse(&s)).map(|x| Arg::Text(x.to_string())),
                (FieldKind::Time, _) => text(&v).and_then(|s| Time::parse(&s)).map(|x| Arg::Text(x.to_string())),
                (FieldKind::DateTime, _) => {
                    text(&v).and_then(|s| DateTime::parse(&s)).map(|x| Arg::Text(x.to_string()))
                }
                (FieldKind::LocalDateTime, _) => {
                    text(&v).and_then(|s| LocalDateTime::parse(&s)).map(|x| Arg::Text(x.to_string()))
                }
                (FieldKind::Choice(options), _) => {
                    text(&v).filter(|s| options.iter().any(|o| o.value == s)).map(Arg::Text)
                }
                (FieldKind::Text, _) => text(&v).map(Arg::Text),
                (FieldKind::Decimal, _) => {
                    text(&v).and_then(|s| crate::Decimal::parse(&s)).map(|x| Arg::Text(x.sql_text()))
                }
                (FieldKind::Custom { .. } | FieldKind::Choices(_), _) => None,
            };
            arg.ok_or_else(bad_cursor)
        })
        .collect()
}

/// One column total the database computes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Agg {
    pub field: usize,
    pub op: Aggregate,
}

/// Adds the select expressions of the totals that fit their column (the same ones the grid
/// allows) and returns them. Money adds the lowest and highest currency, so mixed currencies are
/// detected and not summed.
fn push_aggs(d: &dyn Dialect, sql: &mut Sql, fields: &[FieldMeta], q: &DataQuery) -> Result<Vec<Agg>> {
    let mut out = Vec::new();
    for a in &q.aggregates {
        let Some(field) = fields.iter().position(|f| f.name == a.field) else { continue };
        let f = &fields[field];
        let col = column(d, f)?;
        let numeric = matches!(f.kind, FieldKind::Integer | FieldKind::Money | FieldKind::Decimal);
        let ordered = numeric
            || matches!(f.kind, FieldKind::Date | FieldKind::Time | FieldKind::DateTime | FieldKind::LocalDateTime);
        let fits = match a.op {
            Aggregate::Count => true,
            Aggregate::Sum | Aggregate::Avg => numeric,
            Aggregate::Min | Aggregate::Max => ordered,
        };
        if !fits {
            continue;
        }
        let money = matches!(f.kind, FieldKind::Money);
        match a.op {
            Aggregate::Count => sql.select(d, &format!("COUNT({col})"), Cast::Int, Col::Int),
            Aggregate::Sum if f.kind == FieldKind::Decimal => {
                sql.select(d, &format!("COALESCE(SUM({col}), 0)"), Cast::Decimal, Col::Text)
            }
            Aggregate::Avg if f.kind == FieldKind::Decimal => {
                let scale = f.decimal_limits().1;
                sql.select(d, &format!("ROUND(AVG({col}), {scale})"), Cast::Decimal, Col::Text)
            }
            Aggregate::Sum => sql.select(d, &format!("COALESCE(SUM({col}), 0)"), Cast::Int, Col::Int),
            Aggregate::Avg if money => sql.select(d, &format!("ROUND(AVG({col}))"), Cast::Int, Col::Int),
            Aggregate::Avg => {
                let mut e = String::new();
                d.float(&mut e, &format!("AVG({col})"));
                sql.select(d, &e, Cast::Plain, Col::Float);
            }
            Aggregate::Min | Aggregate::Max => {
                let f_name = if a.op == Aggregate::Min { "MIN" } else { "MAX" };
                let cast = Cast::of(&f.kind);
                let cast = if money { Cast::Int } else { cast };
                sql.select(d, &format!("{f_name}({col})"), cast, Col::of(&f.kind));
            }
        }
        if money && a.op != Aggregate::Count {
            let cur = currency(d, f)?;
            sql.select(d, &format!("MIN({cur})"), Cast::Plain, Col::Text);
            sql.select(d, &format!("MAX({cur})"), Cast::Plain, Col::Text);
        }
        out.push(Agg { field, op: a.op });
    }
    Ok(out)
}

/// Turns the cells of the totals back into [`crate::Total`]s, matching [`crate::total`] for
/// memory data: an empty sum is 0, an empty average is empty, an empty minimum is left out, and
/// money in more than one currency has no total.
pub(crate) fn read_aggs(
    fields: &[FieldMeta],
    aggs: &[Agg],
    cells: &mut impl Iterator<Item = super::decode::Cell>,
) -> Vec<crate::action::Total> {
    use super::decode::{Cell, value_of};
    let mut out = Vec::with_capacity(aggs.len());
    for a in aggs {
        let f = &fields[a.field];
        let first = cells.next().unwrap_or(Cell::Null);
        let money = matches!(f.kind, FieldKind::Money) && a.op != Aggregate::Count;
        let value = if money {
            let (lo, hi) = (cells.next().unwrap_or(Cell::Null), cells.next().unwrap_or(Cell::Null));
            match (&first, &lo) {
                (_, Cell::Text(_)) if lo != hi => None,
                (Cell::Int(n), Cell::Null) if a.op == Aggregate::Sum => Some(Value::Int(*n)),
                (Cell::Null, _) if a.op == Aggregate::Avg => Some(Value::Null),
                (Cell::Null, _) => None,
                _ => Some(value_of(&f.kind, first, Some(lo))),
            }
        } else {
            match (first, a.op) {
                (Cell::Null, Aggregate::Avg) => Some(Value::Null),
                (Cell::Null, _) => None,
                (cell @ Cell::Int(_), Aggregate::Count | Aggregate::Sum) => {
                    Some(value_of(&FieldKind::Integer, cell, None))
                }
                (cell, _) => Some(value_of(&f.kind, cell, None)),
            }
        };
        if let Some(value) = value {
            out.push(crate::action::Total { field: f.name.to_string(), op: a.op, value });
        }
    }
    out
}

/// Everything one `find` runs: the page of rows or groups, the capped count and the totals.
pub(crate) struct FindPlan {
    pub main: Sql,
    pub limit: usize,
    /// Position of the first row of the page, for cursors that page by position.
    pub skip: u64,
    pub shape: Shape,
    pub count: Option<Sql>,
    pub totals: Option<(Sql, Vec<Agg>)>,
}

pub(crate) enum Shape {
    /// Rows; the keys make the next cursor.
    Rows(Vec<OrderKey>),
    /// Groups by a field, from an offset.
    Groups { field: usize, aggs: Vec<Agg>, offset: u64 },
}

/// Next cursor of a group page.
pub(crate) fn group_cursor(offset: u64) -> Cursor {
    encode(&GroupCursor { g: offset })
}

/// Plans a `find`: filters, sort, keyset or offset paging, a count capped at `count_cap`, totals
/// and groups. One row more than the page is fetched to know whether more follow.
pub(crate) fn plan_find<E: Entity>(d: &dyn Dialect, q: &DataQuery, count_cap: u64) -> Result<FindPlan> {
    let fields = E::fields();
    let limit = q.limit() as usize;
    let fetch = limit as i64 + 1;
    let totals = if q.aggregates.is_empty() {
        None
    } else {
        let mut sql = Sql::new();
        sql.push("SELECT ");
        let aggs = push_aggs(d, &mut sql, fields, q)?;
        from_where::<E>(d, &mut sql, q)?;
        (!aggs.is_empty()).then_some((sql, aggs))
    };
    let cap = i64::try_from(count_cap).unwrap_or(i64::MAX);
    let group = q.group.as_deref().and_then(|g| fields.iter().position(|f| f.name == g));

    if let Some(gi) = group {
        let f = &fields[gi];
        let mut group_by = column(d, f)?;
        if matches!(f.kind, FieldKind::Money) {
            group_by.push_str(", ");
            group_by.push_str(&currency(d, f)?);
        }
        let offset = match &q.page {
            PageRequest::Offset { offset, .. } => *offset,
            PageRequest::After { cursor: None, .. } => 0,
            PageRequest::After { cursor: Some(c), .. } => decode::<GroupCursor>(c)?.g,
        };
        let mut main = Sql::new();
        main.push("SELECT ");
        main.select(d, &column(d, f)?, Cast::of(&f.kind), Col::of(&f.kind));
        if matches!(f.kind, FieldKind::Money) {
            main.select(d, &currency(d, f)?, Cast::Plain, Col::Text);
        }
        main.select(d, "COUNT(*)", Cast::Int, Col::Int);
        let aggs = push_aggs(d, &mut main, fields, q)?;
        from_where::<E>(d, &mut main, q)?;
        let desc = q.sort.iter().any(|s| s.desc && s.field == f.name);
        main.push(" GROUP BY ");
        main.push(&group_by);
        main.push(" ORDER BY ");
        main.push(&column(d, f)?);
        main.push(if desc { " DESC NULLS LAST" } else { " ASC NULLS FIRST" });
        d.page(&mut main, fetch, i64::try_from(offset).unwrap_or(i64::MAX));
        let count = if q.with_total {
            let mut c = Sql::new();
            c.push("SELECT 1 AS c0");
            from_where::<E>(d, &mut c, q)?;
            c.push(" GROUP BY ");
            c.push(&group_by);
            Some(capped(d, c, cap))
        } else {
            None
        };
        return Ok(FindPlan {
            main,
            limit,
            skip: offset,
            shape: Shape::Groups { field: gi, aggs, offset },
            count,
            totals,
        });
    }

    let keys = order_keys(d, fields, q)?;
    let mut main = Sql::new();
    main.push("SELECT ");
    select_fields(d, &mut main, fields)?;
    let has_where = from_where::<E>(d, &mut main, q)?;
    let skip = match &q.page {
        PageRequest::Offset { offset, .. } => *offset,
        PageRequest::After { cursor: Some(c), .. } if masked_order(fields, &keys) => {
            let body: SkipCursor = decode(c)?;
            if body.o != order_of(fields, &keys) {
                return Err(bad_cursor());
            }
            body.s
        }
        PageRequest::After { cursor, .. } => {
            if let Some(c) = cursor {
                let values = read_cursor(fields, &keys, c)?;
                push_seek(d, &mut main, fields, &keys, &values, has_where)?;
            }
            0
        }
    };
    let offset = i64::try_from(skip).unwrap_or(i64::MAX);
    push_order(d, &mut main, fields, &keys)?;
    d.page(&mut main, fetch, offset);
    let count = if q.with_total {
        let mut c = Sql::new();
        c.push("SELECT 1 AS c0");
        from_where::<E>(d, &mut c, q)?;
        Some(capped(d, c, cap))
    } else {
        None
    };
    Ok(FindPlan { main, limit, skip, shape: Shape::Rows(keys), count, totals })
}

/// `SELECT COUNT(*)` over at most `cap` rows of `inner`, so counting a huge table stays cheap.
fn capped(d: &dyn Dialect, mut inner: Sql, cap: i64) -> Sql {
    d.page(&mut inner, cap, 0);
    inner.wrap("SELECT COUNT(*) AS c0 FROM (", ") zc");
    inner.cols = vec![Col::Int];
    inner
}

/// Every row matching the filters in the query's order, with no paging: for exports and streams.
pub(crate) fn plan_stream<E: Entity>(d: &dyn Dialect, q: &DataQuery) -> Result<Sql> {
    let fields = E::fields();
    let keys = order_keys(d, fields, q)?;
    let mut sql = Sql::new();
    sql.push("SELECT ");
    select_fields(d, &mut sql, fields)?;
    from_where::<E>(d, &mut sql, q)?;
    push_order(d, &mut sql, fields, &keys)?;
    Ok(sql)
}

fn push_key(d: &dyn Dialect, sql: &mut Sql, fields: &[FieldMeta], key: Value) -> Result<()> {
    let (_, kf) = key_field(fields)?;
    sql.push(" WHERE ");
    sql.push(&column(d, kf)?);
    sql.push(" = ");
    let mut args = Vec::with_capacity(1);
    args_of(&kf.kind, key, &mut args);
    for a in args.into_iter().take(1) {
        sql.bind(d, a, Cast::of(&kf.kind));
    }
    Ok(())
}

/// One row by key.
pub(crate) fn plan_get<E: Entity>(d: &dyn Dialect, key: Value) -> Result<Sql> {
    let mut sql = Sql::new();
    sql.push("SELECT ");
    select_fields(d, &mut sql, E::fields())?;
    sql.push(" FROM ");
    sql.push(&ident(d, E::TABLE)?);
    push_key(d, &mut sql, E::fields(), key)?;
    Ok(sql)
}

/// Whether a key value means "not saved yet": 0 or empty.
pub(crate) fn is_new_key(v: &Value) -> bool {
    matches!(v, Value::Int(0) | Value::Null) || matches!(v, Value::Text(s) if s.is_empty())
}

/// Values of custom fields, which are stored as JSON text. Serialized only when the entity has
/// such fields.
fn custom_json<M: Model>(row: &M) -> Option<serde_json::Map<String, serde_json::Value>> {
    if !M::fields().iter().any(|f| f.kind.is_json()) {
        return None;
    }
    match serde_json::to_value(row) {
        Ok(serde_json::Value::Object(map)) => Some(map),
        _ => None,
    }
}

fn field_args<M: Model>(
    row: &M,
    i: usize,
    f: &FieldMeta,
    json: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Vec<Arg> {
    let mut args = Vec::with_capacity(2);
    if f.kind.is_json() {
        let v = json.as_ref().and_then(|m| m.get(f.name)).filter(|v| !v.is_null());
        args.push(v.map_or(Arg::Null(Col::Text), |v| Arg::Text(v.to_string())));
    } else {
        args_of(&f.kind, row.value_at(i).unwrap_or(Value::Null), &mut args);
    }
    args
}

/// `INSERT`. The key is left to the database when it is new (unless `key` gives one), the row
/// version starts at 1, and `#[readonly]` fields are not written: they keep their column default
/// until server code changes them with its own SQL. `returning` appends the dialect neutral
/// `RETURNING` select list (PostgreSQL).
pub(crate) fn plan_insert<E: Entity>(d: &dyn Dialect, row: &E, key: Option<Arg>, returning: bool) -> Result<Sql> {
    let fields = E::fields();
    let json = custom_json(row);
    let mut sql = Sql::new();
    let mut names = String::new();
    let mut values = Sql::new();
    let mut add = |name: String, arg: Arg, cast: Cast, values: &mut Sql| {
        if !names.is_empty() {
            names.push_str(", ");
            values.push(", ");
        }
        names.push_str(&name);
        values.bind(d, arg, cast);
    };
    for (i, f) in fields.iter().enumerate() {
        if f.readonly && !f.key {
            continue;
        }
        if f.version {
            add(column(d, f)?, Arg::Int(1), Cast::Plain, &mut values);
            continue;
        }
        if f.key {
            let v = row.value_at(i).unwrap_or(Value::Null);
            match (&key, is_new_key(&v)) {
                (Some(k), _) => add(column(d, f)?, k.clone(), Cast::Plain, &mut values),
                (None, true) => {}
                (None, false) => {
                    for a in field_args(row, i, f, &json) {
                        add(column(d, f)?, a, Cast::of(&f.kind), &mut values);
                    }
                }
            }
            continue;
        }
        let mut args = field_args(row, i, f, &json).into_iter();
        if let Some(a) = args.next() {
            add(column(d, f)?, a, Cast::of(&f.kind), &mut values);
        }
        if let Some(a) = args.next() {
            add(currency(d, f)?, a, Cast::Plain, &mut values);
        }
    }
    sql.push("INSERT INTO ");
    sql.push(&ident(d, E::TABLE)?);
    sql.push(" (");
    sql.push(&names);
    sql.push(") VALUES (");
    sql.push(&values.text);
    sql.push(")");
    sql.args = values.args;
    if returning {
        sql.push(" RETURNING ");
        select_fields(d, &mut sql, fields)?;
    }
    Ok(sql)
}

/// `UPDATE ... WHERE key = $k AND version = $v`, raising the version by one. `#[readonly]` fields
/// are not written. Without a `#[version]` field the row is matched by key only.
pub(crate) fn plan_update<E: Entity>(d: &dyn Dialect, row: &E, returning: bool) -> Result<(Sql, bool)> {
    let fields = E::fields();
    let json = custom_json(row);
    let mut sql = Sql::new();
    sql.push("UPDATE ");
    sql.push(&ident(d, E::TABLE)?);
    sql.push(" SET ");
    let mut first = true;
    let mut sep = |sql: &mut Sql| {
        if !first {
            sql.push(", ");
        }
        first = false;
    };
    let mut version = None;
    for (i, f) in fields.iter().enumerate() {
        if f.key || f.readonly {
            continue;
        }
        let col = column(d, f)?;
        if f.version {
            version = Some((col.clone(), row.value_at(i).unwrap_or(Value::Null)));
            sep(&mut sql);
            sql.push(&col);
            sql.push(" = ");
            sql.push(&col);
            sql.push(" + 1");
            continue;
        }
        let mut args = field_args(row, i, f, &json).into_iter();
        if let Some(a) = args.next() {
            sep(&mut sql);
            sql.push(&col);
            sql.push(" = ");
            sql.bind(d, a, Cast::of(&f.kind));
        }
        if let Some(a) = args.next() {
            sep(&mut sql);
            sql.push(&currency(d, f)?);
            sql.push(" = ");
            sql.bind(d, a, Cast::Plain);
        }
    }
    if first {
        return Err(Error::Internal(format!("`{}` has no field that an update can write", E::NAME)));
    }
    let (ki, _) = key_field(fields)?;
    push_key(d, &mut sql, fields, row.value_at(ki).unwrap_or(Value::Null))?;
    let versioned = version.is_some();
    if let Some((col, v)) = version {
        sql.push(" AND ");
        sql.push(&col);
        sql.push(" = ");
        let Value::Int(n) = v else { return Err(Error::Internal("row version is not a whole number".into())) };
        sql.bind(d, Arg::Int(n), Cast::Plain);
    }
    if returning {
        sql.push(" RETURNING ");
        select_fields(d, &mut sql, fields)?;
    }
    Ok((sql, versioned))
}

/// `DELETE` by key.
pub(crate) fn plan_delete<E: Entity>(d: &dyn Dialect, key: Value) -> Result<Sql> {
    let mut sql = Sql::new();
    sql.push("DELETE FROM ");
    sql.push(&ident(d, E::TABLE)?);
    push_key(d, &mut sql, E::fields(), key)?;
    Ok(sql)
}

/// Turns a unique constraint violation into a validation error on the field the constraint
/// guards. The field is the `#[rule(unique)]` field whose name is in the constraint's name (name
/// constraints `<table>_<column>_key`, as PostgreSQL does), or the only unique field. Otherwise
/// the error belongs to the whole record.
pub(crate) fn unique_error<M: Model>(constraint: Option<&str>) -> Error {
    let constraint = constraint.unwrap_or_default().to_ascii_lowercase();
    let unique: Vec<&FieldMeta> = M::fields().iter().filter(|f| f.rules.contains(&Rule::Unique)).collect();
    let field = unique
        .iter()
        .find(|f| !constraint.is_empty() && constraint.contains(&f.name.to_ascii_lowercase()))
        .or_else(|| (unique.len() == 1).then(|| &unique[0]));
    let mut errors = ValidationErrors::default();
    match field {
        Some(f) => errors.errors.push(FieldError {
            field: f.name,
            rule: Rule::Unique,
            message: Text::key("zeyn.validation.unique").arg("label", f.label_text()),
        }),
        None => errors.add_general(Text::key("zeyn.error.duplicate")),
    }
    errors.into()
}

/// A unique violation that no field explains.
pub(crate) fn duplicate_error() -> Error {
    let mut errors = ValidationErrors::default();
    errors.add_general(Text::key("zeyn.error.duplicate"));
    errors.into()
}

/// `CREATE TABLE` for an entity, for scaffolding and tests; production schemas belong in
/// migrations. Whole number keys are generated, `#[rule(unique)]` adds a named unique constraint,
/// `#[version]` starts at 1, and `#[readonly]` columns get a default because inserts skip them.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::sql::{Postgres, create_table};
///
/// #[entity(table = "products")]
/// pub struct Product {
///     #[key]
///     pub id: ProductId,
///     #[rule(required, max_len = 20, unique)]
///     pub code: String,
///     pub price: Money,
/// }
///
/// let sql = create_table::<Product>(&Postgres).unwrap();
/// assert!(sql[0].starts_with("CREATE TABLE \"products\""));
/// assert!(sql[0].contains("\"code\" VARCHAR(20) NOT NULL"));
/// assert!(sql[0].contains("\"price_currency\" VARCHAR(3) NOT NULL"));
/// ```
pub fn create_table<E: Entity>(d: &dyn Dialect) -> Result<Vec<String>> {
    let table = ident(d, E::TABLE)?;
    let bare = E::TABLE.rsplit('.').next().unwrap_or(E::TABLE);
    let mut cols = Vec::new();
    for f in E::fields() {
        let name = column(d, f)?;
        let text_null = d.empty_is_null() && (matches!(f.kind, FieldKind::Text) || f.kind.is_json());
        let not_null = if f.optional || text_null { "" } else { " NOT NULL" };
        if f.key && matches!(f.kind, FieldKind::Id | FieldKind::Integer) {
            cols.push(format!("{name} {} PRIMARY KEY", d.generated_key()));
            continue;
        }
        if f.key {
            cols.push(format!("{name} {} PRIMARY KEY", d.column_type(&f.kind, f.max_len())));
            continue;
        }
        let default = match (&f.kind, f.version, f.readonly && !f.optional) {
            (_, true, _) => " DEFAULT 1",
            (FieldKind::Integer | FieldKind::Money | FieldKind::Decimal, _, true) => " DEFAULT 0",
            (FieldKind::Bool, _, true) => match d.false_literal() {
                "0" => " DEFAULT 0",
                _ => " DEFAULT FALSE",
            },
            (FieldKind::Text, _, true) if !text_null => " DEFAULT ''",
            _ => "",
        };
        let ty = match f.kind {
            FieldKind::Decimal => {
                let (precision, scale) = f.decimal_limits();
                d.decimal_type(precision, scale)
            }
            _ => d.column_type(&f.kind, f.max_len()),
        };
        cols.push(format!("{name} {ty}{default}{not_null}"));
        if matches!(f.kind, FieldKind::Money) {
            let default = if f.readonly && !f.optional { " DEFAULT 'TRY'" } else { "" };
            cols.push(format!("{} {}{default}{not_null}", currency(d, f)?, d.column_type(&FieldKind::Text, Some(3))));
        }
        if f.rules.contains(&Rule::Unique) {
            cols.push(format!("CONSTRAINT {} UNIQUE ({name})", ident(d, &format!("{bare}_{}_key", f.name))?));
        }
    }
    let mut out = vec![format!("CREATE TABLE {table} (\n    {}\n)", cols.join(",\n    "))];
    out.extend(d.after_create(E::TABLE)?);
    Ok(out)
}
