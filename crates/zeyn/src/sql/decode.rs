//! Values between Zeyn models and database rows.
//!
//! Every driver reads a row into [`Cell`]s of a few plain types (whole number, float, text, yes/no)
//! and binds [`Arg`]s of the same types. Dates travel as ISO text that the SQL converts, so drivers
//! need no date library. Rows become models through a serde deserializer that moves the cells into
//! the model's fields without building an intermediate map.

use crate::action::{Error, Result};
use crate::model::{FieldMeta, Model};
use crate::time::{Date, DateTime, LocalDateTime, Time};
use crate::value::{Currency, FieldKind, Money, Value};
use serde::de::value::{BorrowedStrDeserializer, Error as DeError, I64Deserializer, StrDeserializer};
use serde::de::{self, DeserializeSeed, Deserializer, IntoDeserializer, MapAccess, Visitor};

/// Type of one column as a driver reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    Int,
    Float,
    Text,
    Bool,
}

impl Col {
    /// Column type that holds a field of `kind` (for money, its amount in the smallest unit).
    pub fn of(kind: &FieldKind) -> Col {
        match kind {
            FieldKind::Id | FieldKind::Integer | FieldKind::Money => Col::Int,
            FieldKind::Bool => Col::Bool,
            _ => Col::Text,
        }
    }
}

/// One value read from a row.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
}

/// One bound parameter. Values never become part of the SQL text.
///
/// ```
/// use zeyn::sql::Arg;
/// assert_eq!(Arg::from(5), Arg::Int(5));
/// assert_eq!(Arg::from("tea"), Arg::Text("tea".into()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// A NULL of the given column type.
    Null(Col),
    Int(i64),
    Text(String),
    Bool(bool),
}

impl From<i64> for Arg {
    fn from(v: i64) -> Self {
        Arg::Int(v)
    }
}

impl From<i32> for Arg {
    fn from(v: i32) -> Self {
        Arg::Int(i64::from(v))
    }
}

impl From<bool> for Arg {
    fn from(v: bool) -> Self {
        Arg::Bool(v)
    }
}

impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Text(v)
    }
}

impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Text(v.to_string())
    }
}

/// ISO code stored in the currency column of a money field.
pub(crate) fn currency_code(c: Currency) -> &'static str {
    match c {
        Currency::Try => "TRY",
        Currency::Usd => "USD",
        Currency::Eur => "EUR",
    }
}

fn currency_from_code(code: &str) -> Option<Currency> {
    match code.trim() {
        "TRY" => Some(Currency::Try),
        "USD" => Some(Currency::Usd),
        "EUR" => Some(Currency::Eur),
        _ => None,
    }
}

/// Serde variant name of a currency, as the derived `Deserialize` expects it.
fn currency_variant(code: &str) -> Option<&'static str> {
    currency_from_code(code).map(|c| match c {
        Currency::Try => "Try",
        Currency::Usd => "Usd",
        Currency::Eur => "Eur",
    })
}

/// Parameters for one field value: one, or two for money (amount and currency code).
pub(crate) fn args_of(kind: &FieldKind, value: Value, out: &mut Vec<Arg>) {
    let null = Arg::Null(Col::of(kind));
    match value {
        Value::Null => {
            out.push(null);
            if matches!(kind, FieldKind::Money) {
                out.push(Arg::Null(Col::Text));
            }
        }
        Value::Money(m) => {
            out.push(Arg::Int(m.minor));
            out.push(Arg::Text(currency_code(m.currency).into()));
        }
        Value::Int(n) => out.push(Arg::Int(n)),
        Value::Bool(b) => out.push(Arg::Bool(b)),
        Value::Text(s) => out.push(Arg::Text(s)),
        Value::Choice(c) => out.push(Arg::Text(c.into())),
        Value::Date(d) => out.push(Arg::Text(d.to_string())),
        Value::Time(t) => out.push(Arg::Text(t.to_string())),
        Value::DateTime(d) => out.push(Arg::Text(d.to_string())),
        Value::LocalDateTime(l) => out.push(Arg::Text(l.to_string())),
        Value::Decimal(x) => out.push(Arg::Text(x.to_string())),
    }
}

/// A field value from its cells: money takes the amount and the currency cell.
pub(crate) fn value_of(kind: &FieldKind, first: Cell, currency: Option<Cell>) -> Value {
    match first {
        Cell::Null => Value::Null,
        Cell::Bool(b) => Value::Bool(b),
        Cell::Float(x) => Value::Decimal(x),
        Cell::Int(n) if matches!(kind, FieldKind::Money) => {
            let c = match currency {
                Some(Cell::Text(code)) => currency_from_code(&code).unwrap_or_default(),
                _ => Currency::default(),
            };
            Value::Money(Money::new(n, c))
        }
        Cell::Int(n) => Value::Int(n),
        Cell::Text(s) => text_value(kind, s),
    }
}

fn text_value(kind: &FieldKind, s: String) -> Value {
    let parsed = match kind {
        FieldKind::Date => Date::parse(&s).map(Value::Date),
        FieldKind::Time => Time::parse(&s).map(Value::Time),
        FieldKind::DateTime => DateTime::parse(&s).map(Value::DateTime),
        FieldKind::LocalDateTime => LocalDateTime::parse(&s).map(Value::LocalDateTime),
        FieldKind::Choice(options) => options.iter().find(|o| o.value == s).map(|o| Value::Choice(o.value)),
        _ => None,
    };
    parsed.unwrap_or(Value::Text(s))
}

/// Builds a model from the cells of one row, moving the text out of them. The buffer is left empty
/// so the driver can reuse it for the next row.
pub(crate) fn decode_row<M: Model>(cells: &mut Vec<Cell>) -> Result<M> {
    let de = RowDe { fields: M::fields(), cells: cells.drain(..), index: 0 };
    M::deserialize(de).map_err(|e| Error::Internal(format!("reading a `{}` row: {e}", M::NAME)))
}

struct RowDe<'a> {
    fields: &'static [FieldMeta],
    cells: std::vec::Drain<'a, Cell>,
    index: usize,
}

impl<'de> Deserializer<'de> for RowDe<'_> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_map(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf option unit
        unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de> MapAccess<'de> for RowDe<'_> {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, DeError> {
        match self.fields.get(self.index) {
            Some(f) => seed.deserialize(BorrowedStrDeserializer::<DeError>::new(f.name)).map(Some),
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let meta = &self.fields[self.index];
        self.index += 1;
        let first = self.cells.next().unwrap_or(Cell::Null);
        match (&meta.kind, first) {
            (FieldKind::Money, minor) => {
                let currency = self.cells.next().unwrap_or(Cell::Null);
                seed.deserialize(MoneyDe { minor, currency })
            }
            (FieldKind::Custom { .. }, Cell::Text(json)) => {
                let value: serde_json::Value = serde_json::from_str(&json).map_err(de::Error::custom)?;
                seed.deserialize(value).map_err(de::Error::custom)
            }
            (_, cell) => seed.deserialize(CellDe(cell)),
        }
    }
}

struct CellDe(Cell);

impl<'de> Deserializer<'de> for CellDe {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Cell::Null => visitor.visit_unit(),
            Cell::Int(n) => visitor.visit_i64(n),
            Cell::Float(x) => visitor.visit_f64(x),
            Cell::Text(s) => visitor.visit_string(s),
            Cell::Bool(b) => visitor.visit_bool(b),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Cell::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    /// A NULL read into a required text field is the empty string (Oracle stores `''` as NULL).
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            Cell::Null => visitor.visit_str(""),
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        self.deserialize_string(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value, DeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match self.0 {
            Cell::Text(s) => visitor.visit_enum(s.into_deserializer()),
            _ => Err(de::Error::custom("expected the text of a choice")),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char bytes byte_buf unit
        unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

struct MoneyDe {
    minor: Cell,
    currency: Cell,
}

impl<'de> Deserializer<'de> for MoneyDe {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        let Cell::Int(minor) = self.minor else { return Err(de::Error::custom("money amount is not a whole number")) };
        let code = match &self.currency {
            Cell::Text(s) => s.as_str(),
            _ => "",
        };
        let currency = currency_variant(code).ok_or_else(|| de::Error::custom(format!("unknown currency `{code}`")))?;
        visitor.visit_map(MoneyMap { minor, currency, step: 0 })
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.minor {
            Cell::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf unit
        unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

struct MoneyMap {
    minor: i64,
    currency: &'static str,
    step: u8,
}

impl<'de> MapAccess<'de> for MoneyMap {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, DeError> {
        let key = match self.step {
            0 => "minor",
            1 => "currency",
            _ => return Ok(None),
        };
        seed.deserialize(BorrowedStrDeserializer::<DeError>::new(key)).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        self.step += 1;
        if self.step == 1 {
            seed.deserialize(I64Deserializer::<DeError>::new(self.minor))
        } else {
            seed.deserialize(StrDeserializer::<DeError>::new(self.currency))
        }
    }
}
