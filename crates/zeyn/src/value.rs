use crate::i18n::NumberFormat;
use crate::time::{Date, DateTime, LocalDateTime, Time};
use serde::{Deserialize, Serialize};
use std::fmt;

/// How a field is shown on screen and described in the schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "options", rename_all = "snake_case")]
pub enum FieldKind {
    Id,
    Text,
    Integer,
    Money,
    /// An exact decimal number ([`crate::Decimal`]) with the `scale` and `precision` rules.
    Decimal,
    Bool,
    Date,
    Time,
    /// An exact moment, shown in the user's time zone.
    DateTime,
    /// A wall clock date and time with no time zone.
    LocalDateTime,
    Choice(&'static [ChoiceOption]),
    /// Several options of a `#[choice]` enum: a `Vec<T>` field, edited with a multi select.
    Choices(&'static [ChoiceOption]),
    /// Field type defined outside Zeyn. `editor` names the client side editor that enhances the input.
    Custom {
        editor: &'static str,
    },
}

impl FieldKind {
    /// Whether values of this kind are stored as JSON text in a database column: custom types,
    /// line lists, id lists and several choices.
    pub fn is_json(&self) -> bool {
        matches!(self, FieldKind::Custom { .. } | FieldKind::Choices(_))
    }
}

/// One option of a `#[choice]` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ChoiceOption {
    pub value: &'static str,
    pub label: &'static str,
    /// Catalog key for the label, `Enum.Variant`.
    pub key: &'static str,
}

/// Type independent form of a field value. Rendering, schema and grid use this.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    Money(Money),
    Date(Date),
    Time(Time),
    DateTime(DateTime),
    LocalDateTime(LocalDateTime),
    Choice(&'static str),
    /// A computed number with a fraction, such as the average of a whole number column.
    /// Shown with two decimals in the user's locale. Never a model field.
    Decimal(f64),
    /// An exact decimal number, the value of a [`crate::Decimal`] field.
    Number(crate::Decimal),
}

/// Currency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    #[default]
    Try,
    Usd,
    Eur,
}

impl Currency {
    pub const fn symbol(self) -> &'static str {
        match self {
            Currency::Try => "₺",
            Currency::Usd => "$",
            Currency::Eur => "€",
        }
    }
}

/// Amount of money, stored as an integer count of the smallest unit to avoid floating point errors.
///
/// ```
/// use zeyn::Money;
/// assert_eq!(Money::tl(123456).format("tr"), "1.234,56 ₺");
/// assert_eq!(Money::tl(123456).format("en"), "₺1,234.56");
/// assert_eq!(Money::parse("1.234,56", "tr", zeyn::Currency::Try), Ok(Money::tl(123456)));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    pub minor: i64,
    pub currency: Currency,
}

impl Money {
    pub const fn new(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }

    /// Turkish lira amount in kuruş.
    pub const fn tl(kurus: i64) -> Self {
        Self::new(kurus, Currency::Try)
    }

    /// Amount without the symbol in the locale's format: `-1.234,56` in `tr`, `-1,234.56` in `en`.
    pub fn format_plain(&self, locale: &str) -> String {
        let f = NumberFormat::for_locale(locale);
        let abs = self.minor.unsigned_abs();
        let major = (abs / 100).to_string();
        let mut grouped = String::with_capacity(major.len() + major.len() / 3);
        for (i, c) in major.chars().enumerate() {
            if i > 0 && (major.len() - i).is_multiple_of(3) {
                grouped.push(f.group);
            }
            grouped.push(c);
        }
        let sign = if self.minor < 0 { "-" } else { "" };
        format!("{sign}{grouped}{}{:02}", f.decimal, abs % 100)
    }

    /// Amount with the currency symbol in the locale's format.
    pub fn format(&self, locale: &str) -> String {
        let plain = self.format_plain(locale);
        let symbol = self.currency.symbol();
        if NumberFormat::for_locale(locale).symbol_first {
            match plain.strip_prefix('-') {
                Some(rest) => format!("-{symbol}{rest}"),
                None => format!("{symbol}{plain}"),
            }
        } else {
            format!("{plain} {symbol}")
        }
    }

    /// Reads an amount typed in the locale's format. Group separators, spaces and the symbol are
    /// ignored; at most two decimals are accepted.
    #[allow(
        clippy::result_unit_err,
        reason = "the only failure is text that is not an amount; callers turn it into their own field error"
    )]
    pub fn parse(input: &str, locale: &str, currency: Currency) -> Result<Money, ()> {
        let f = NumberFormat::for_locale(locale);
        let cleaned: String = input
            .trim()
            .chars()
            .filter(|c| !c.is_whitespace() && *c != f.group && !"₺$€".contains(*c))
            .collect();
        let (negative, digits) = match cleaned.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, cleaned.as_str()),
        };
        let (major, minor) = match digits.split_once(f.decimal) {
            Some((a, b)) => (a, b),
            None => (digits, ""),
        };
        let all_digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
        if major.is_empty() || !all_digits(major) || !all_digits(minor) || minor.len() > 2 {
            return Err(());
        }
        let major: i64 = major.parse().map_err(|_| ())?;
        let minor: i64 = format!("{minor:0<2}")
            .parse()
            .map_err(|_| ())?;
        let value = major
            .checked_mul(100)
            .and_then(|m| m.checked_add(minor))
            .ok_or(())?;
        Ok(Money::new(if negative { -value } else { value }, currency))
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format("en"))
    }
}

/// Types that can be model fields.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a model field",
    label = "unsupported field type",
    note = "supported types: String, i32, i64, bool, Money, Decimal, Date, Time, DateTime, LocalDateTime, Option<T>, entity ids, `#[choice]` enums, Vec of a `#[choice]` enum or a model, and Vec<i64>"
)]
pub trait FieldType {
    const KIND: FieldKind;

    /// Whether the field may be empty (`Option<T>`).
    const OPTIONAL: bool = false;

    fn to_value(&self) -> Value;

    #[doc(hidden)]
    fn __is_empty(&self) -> bool {
        false
    }

    #[doc(hidden)]
    fn __text(&self) -> Option<&str> {
        None
    }

    #[doc(hidden)]
    fn __text_len(&self) -> Option<usize> {
        self.__text().map(|s| s.chars().count())
    }

    #[doc(hidden)]
    fn __compare(&self, _limit: i64) -> Option<std::cmp::Ordering> {
        None
    }

    /// The exact decimal held, for the `scale` and `precision` checks.
    #[doc(hidden)]
    fn __decimal(&self) -> Option<crate::Decimal> {
        None
    }

    /// Validates nested values, such as the lines of an order. Errors name the line and column.
    #[doc(hidden)]
    fn __validate_items(
        &self,
        _meta: &crate::model::FieldMeta,
        _errors: &mut crate::model::ValidationErrors,
    ) {
    }
}

impl FieldType for String {
    const KIND: FieldKind = FieldKind::Text;
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
    fn __is_empty(&self) -> bool {
        self.trim().is_empty()
    }
    fn __text(&self) -> Option<&str> {
        Some(self)
    }
}

impl FieldType for i64 {
    const KIND: FieldKind = FieldKind::Integer;
    fn to_value(&self) -> Value {
        Value::Int(*self)
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&limit))
    }
}

impl FieldType for i32 {
    const KIND: FieldKind = FieldKind::Integer;
    fn to_value(&self) -> Value {
        Value::Int(i64::from(*self))
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        Some(i64::from(*self).cmp(&limit))
    }
}

impl FieldType for Date {
    const KIND: FieldKind = FieldKind::Date;
    fn to_value(&self) -> Value {
        Value::Date(*self)
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        Some(self.compact().cmp(&limit))
    }
}

impl FieldType for Time {
    const KIND: FieldKind = FieldKind::Time;
    fn to_value(&self) -> Value {
        Value::Time(*self)
    }
}

impl FieldType for DateTime {
    const KIND: FieldKind = FieldKind::DateTime;
    fn to_value(&self) -> Value {
        Value::DateTime(*self)
    }
}

impl FieldType for LocalDateTime {
    const KIND: FieldKind = FieldKind::LocalDateTime;
    fn to_value(&self) -> Value {
        Value::LocalDateTime(*self)
    }
}

impl FieldType for bool {
    const KIND: FieldKind = FieldKind::Bool;
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}

impl FieldType for Money {
    const KIND: FieldKind = FieldKind::Money;
    fn to_value(&self) -> Value {
        Value::Money(*self)
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        Some(i128::from(self.minor).cmp(&(i128::from(limit) * 100)))
    }
}

impl FieldType for crate::Decimal {
    const KIND: FieldKind = FieldKind::Decimal;
    fn to_value(&self) -> Value {
        Value::Number(*self)
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        Some(self.cmp_int(limit))
    }
    fn __decimal(&self) -> Option<crate::Decimal> {
        Some(*self)
    }
}

impl<T: FieldType> FieldType for Option<T> {
    const KIND: FieldKind = T::KIND;
    const OPTIONAL: bool = true;
    fn to_value(&self) -> Value {
        self.as_ref()
            .map_or(Value::Null, T::to_value)
    }
    fn __is_empty(&self) -> bool {
        self.as_ref().is_none_or(T::__is_empty)
    }
    fn __text(&self) -> Option<&str> {
        self.as_ref().and_then(T::__text)
    }
    fn __compare(&self, limit: i64) -> Option<std::cmp::Ordering> {
        self.as_ref()
            .and_then(|v| v.__compare(limit))
    }
    fn __decimal(&self) -> Option<crate::Decimal> {
        self.as_ref().and_then(T::__decimal)
    }
}

/// Column types that accept `min()` and `max()` totals in a grid.
#[diagnostic::on_unimplemented(
    message = "`min()` and `max()` cannot be used on a `{Self}` column",
    label = "values of this column have no order that makes a useful total",
    note = "`min()` and `max()` work on i32, i64, Money, Decimal, Date, Time, DateTime, LocalDateTime and their Option forms"
)]
pub trait SupportsMinMaxTotal {}
impl SupportsMinMaxTotal for i32 {}
impl SupportsMinMaxTotal for i64 {}
impl SupportsMinMaxTotal for Money {}
impl SupportsMinMaxTotal for crate::Decimal {}
impl SupportsMinMaxTotal for Date {}
impl SupportsMinMaxTotal for Time {}
impl SupportsMinMaxTotal for DateTime {}
impl SupportsMinMaxTotal for LocalDateTime {}
impl<T: SupportsMinMaxTotal> SupportsMinMaxTotal for Option<T> {}

/// Enums made with `#[choice]`. DataGrid badges work only on them.
#[diagnostic::on_unimplemented(
    message = "`badge()` cannot be used on a `{Self}` column",
    label = "not a `#[choice]` enum",
    note = "badges color the options of a `#[choice]` enum; use `cell(...)` for other columns"
)]
pub trait ChoiceType {}

/// Yes/no column types, for `check_icon()`.
#[diagnostic::on_unimplemented(
    message = "`check_icon()` cannot be used on a `{Self}` column",
    label = "not a yes/no column",
    note = "`check_icon()` works on bool and Option<bool>"
)]
pub trait IsBool {}
impl IsBool for bool {}
impl IsBool for Option<bool> {}

/// Moment column types, for `relative_time()`.
#[diagnostic::on_unimplemented(
    message = "`relative_time()` cannot be used on a `{Self}` column",
    label = "not a moment",
    note = "`relative_time()` works on DateTime and Option<DateTime>, which are exact moments"
)]
pub trait IsMoment {}
impl IsMoment for DateTime {}
impl IsMoment for Option<DateTime> {}

/// Types that accept the `required` rule.
#[diagnostic::on_unimplemented(
    message = "the `required` rule cannot be used on `{Self}`",
    label = "this field always has a value",
    note = "`required` works on String and Option<T>"
)]
pub trait SupportsRequired {}
impl SupportsRequired for String {}
impl<T> SupportsRequired for Option<T> {}

/// Types that accept the text rules `min_len`, `max_len` and `email`.
#[diagnostic::on_unimplemented(
    message = "text rules (`min_len`, `max_len`, `email`) cannot be used on `{Self}`",
    label = "not a text field",
    note = "text rules work on String and Option<String>"
)]
pub trait SupportsText {}
impl SupportsText for String {}
impl SupportsText for Option<String> {}

/// Types that accept the `min` and `max` rules.
#[diagnostic::on_unimplemented(
    message = "the `min` and `max` rules cannot be used on `{Self}`",
    label = "not a numeric field",
    note = "`min` and `max` work on i32, i64, Money, Decimal, Date and their Option forms; for Money and Decimal the limit is a whole number, for Date it is written as YYYY_MM_DD"
)]
pub trait SupportsMinMax {}
impl SupportsMinMax for i32 {}
impl SupportsMinMax for i64 {}
impl SupportsMinMax for Money {}
impl SupportsMinMax for crate::Decimal {}
/// Dates take limits written as `YYYY_MM_DD`: `#[rule(min = 2024_01_01)]`.
impl SupportsMinMax for Date {}
impl<T: SupportsMinMax> SupportsMinMax for Option<T> {}

/// Column types that accept `sum()` in a grid.
#[diagnostic::on_unimplemented(
    message = "`sum()` and `avg()` cannot be used on a `{Self}` column",
    label = "not a numeric column",
    note = "`sum()` and `avg()` work on i32, i64, Money, Decimal and their Option forms; use `count()` for other columns"
)]
pub trait SupportsSum {}
impl SupportsSum for crate::Decimal {}
impl SupportsSum for i32 {}
impl SupportsSum for i64 {}
impl SupportsSum for Money {}
impl<T: SupportsSum> SupportsSum for Option<T> {}

/// Types that accept the `scale` and `precision` rules.
#[diagnostic::on_unimplemented(
    message = "the `scale` and `precision` rules cannot be used on `{Self}`",
    label = "not a decimal field",
    note = "`scale` and `precision` work on Decimal and Option<Decimal>; Money always has two decimals"
)]
pub trait SupportsScale {}
impl SupportsScale for crate::Decimal {}
impl SupportsScale for Option<crate::Decimal> {}

/// Item types a `Vec<T>` field can hold: line models (`#[model]`, `#[entity]`), edited as a lines
/// table, and `#[choice]` enums, edited with a multi select. `Vec<i64>` holds ids for
/// `Form::lookup_many`.
#[diagnostic::on_unimplemented(
    message = "`Vec<{Self}>` cannot be used as a model field",
    label = "not a line model or a `#[choice]` enum",
    note = "a Vec field holds lines (`Vec` of a `#[model]`), several options (`Vec` of a `#[choice]` enum) or ids (`Vec<i64>` with `Form::lookup_many`)"
)]
pub trait VecItem: Sized {
    #[doc(hidden)]
    const VEC_KIND: FieldKind;

    #[doc(hidden)]
    fn __vec_value(items: &[Self]) -> Value;

    #[doc(hidden)]
    fn __vec_validate(
        _items: &[Self],
        _meta: &crate::model::FieldMeta,
        _errors: &mut crate::model::ValidationErrors,
    ) {
    }
}

impl<L: VecItem> FieldType for Vec<L> {
    const KIND: FieldKind = L::VEC_KIND;

    fn to_value(&self) -> Value {
        L::__vec_value(self)
    }

    fn __is_empty(&self) -> bool {
        self.is_empty()
    }

    fn __validate_items(
        &self,
        meta: &crate::model::FieldMeta,
        errors: &mut crate::model::ValidationErrors,
    ) {
        L::__vec_validate(self, meta, errors);
    }
}

impl<L: VecItem> SupportsRequired for Vec<L> {}

/// The value of several choices: their values joined with commas, `Small,Large`.
#[doc(hidden)]
pub fn __choices_value<T: FieldType>(items: &[T]) -> Value {
    let values: Vec<String> = items
        .iter()
        .filter_map(|i| match i.to_value() {
            Value::Choice(c) => Some(c.to_string()),
            _ => None,
        })
        .collect();
    Value::Text(values.join(","))
}
