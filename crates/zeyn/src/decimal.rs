//! Exact decimal numbers for model fields: quantities with fractions, unit prices with more than
//! two decimals, rates and percentages. No floating point is involved anywhere: a value is a whole
//! number of units of `10^-scale`, like a database `NUMERIC(p, s)`.

use crate::i18n::NumberFormat;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Most decimals a [`Decimal`] holds.
pub const MAX_SCALE: u8 = 28;

/// Most significant digits a [`Decimal`] holds, the precision of `NUMERIC(38, s)` and Oracle
/// `NUMBER`.
pub const MAX_PRECISION: u8 = 38;

/// Decimals of a `Decimal` field without a `scale` rule.
pub const DEFAULT_SCALE: u8 = 2;

/// Digits of a `Decimal` field without a `precision` rule.
pub const DEFAULT_PRECISION: u8 = 18;

const TEN_28: i128 = 10_000_000_000_000_000_000_000_000_000;

fn pow10(n: u8) -> i128 {
    10i128.pow(u32::from(n))
}

/// An exact decimal number: `mantissa / 10^scale`. Equal values compare equal whatever their
/// scale (`1.5 == 1.50`); the scale is kept so `1.50` is shown as typed.
///
/// ```
/// use zeyn::Decimal;
/// let d = Decimal::parse("1234.50").unwrap();
/// assert_eq!(d.to_string(), "1234.50");
/// assert_eq!(d.format("tr"), "1.234,50");
/// assert_eq!(d.format("en"), "1,234.50");
/// assert_eq!(Decimal::parse_input("1.234,5", "tr"), Decimal::parse("1234.5"));
/// assert_eq!(Decimal::parse("0.1").unwrap().checked_add(Decimal::parse("0.2").unwrap()), Decimal::parse("0.3"));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct Decimal {
    mantissa: i128,
    scale: u8,
}

impl Decimal {
    pub const ZERO: Decimal = Decimal {
        mantissa: 0,
        scale: 0,
    };

    /// `mantissa / 10^scale`, or `None` when the scale is over [`MAX_SCALE`] or the number has
    /// more than [`MAX_PRECISION`] digits.
    pub fn new(mantissa: i128, scale: u8) -> Option<Decimal> {
        (scale <= MAX_SCALE && digits(mantissa) <= MAX_PRECISION)
            .then_some(Decimal { mantissa, scale })
    }

    /// A whole number.
    pub const fn from_int(n: i64) -> Decimal {
        Decimal {
            mantissa: n as i128,
            scale: 0,
        }
    }

    pub fn mantissa(&self) -> i128 {
        self.mantissa
    }

    pub fn scale(&self) -> u8 {
        self.scale
    }

    /// Reads the fixed form `-1234.5`, also `.5`, `1e-3` and `12345E-2` (how databases may write
    /// numbers). No group separators. `None` for anything else.
    pub fn parse(text: &str) -> Option<Decimal> {
        let s = text.trim();
        let (negative, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        let (number, exp) = match s.split_once(['e', 'E']) {
            Some((n, e)) if e.len() <= 4 => (n, e.parse::<i32>().ok()?),
            Some(_) => return None,
            None => (s, 0),
        };
        let (whole, frac) = number
            .split_once('.')
            .unwrap_or((number, ""));
        let digits_only = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
        if (whole.is_empty() && frac.is_empty()) || !digits_only(whole) || !digits_only(frac) {
            return None;
        }
        let all = format!("{whole}{frac}");
        let all = all.trim_start_matches('0');
        if all.len() > usize::from(MAX_PRECISION) {
            return None;
        }
        let mut mantissa: i128 = if all.is_empty() { 0 } else { all.parse().ok()? };
        let mut scale = frac.len() as i32 - exp;
        if scale < 0 {
            mantissa = mantissa.checked_mul(10i128.checked_pow(u32::try_from(-scale).ok()?)?)?;
            scale = 0;
        }
        let scale = u8::try_from(scale)
            .ok()
            .filter(|s| *s <= MAX_SCALE)?;
        Decimal::new(if negative { -mantissa } else { mantissa }, scale)
    }

    /// Reads a number typed in the locale's format: `1.234,56` in `tr`, `1,234.56` in `en`.
    /// Group separators and spaces are ignored; exponents are refused.
    pub fn parse_input(text: &str, locale: &str) -> Option<Decimal> {
        let f = NumberFormat::for_locale(locale);
        let mut plain = String::with_capacity(text.len());
        for c in text.trim().chars() {
            match c {
                c if c == f.group || c.is_whitespace() => {}
                c if c == f.decimal => plain.push('.'),
                '0'..='9' | '-' | '+' => plain.push(c),
                _ => return None,
            }
        }
        Decimal::parse(&plain)
    }

    /// The number in the locale's format, with its own decimals: `-1.234,50` in `tr`.
    pub fn format(&self, locale: &str) -> String {
        let f = NumberFormat::for_locale(locale);
        let plain = self.to_string();
        let (sign, rest) = match plain.strip_prefix('-') {
            Some(r) => ("-", r),
            None => ("", plain.as_str()),
        };
        let (whole, frac) = rest
            .split_once('.')
            .unwrap_or((rest, ""));
        let mut out = String::with_capacity(plain.len() + whole.len() / 3);
        out.push_str(sign);
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                out.push(f.group);
            }
            out.push(c);
        }
        if !frac.is_empty() {
            out.push(f.decimal);
            out.push_str(frac);
        }
        out
    }

    /// The number rounded half away from zero, or padded with zeros, to `scale` decimals.
    ///
    /// ```
    /// use zeyn::Decimal;
    /// let d = Decimal::parse("2.345").unwrap();
    /// assert_eq!(d.round(2).to_string(), "2.35");
    /// assert_eq!(Decimal::parse("-2.345").unwrap().round(2).to_string(), "-2.35");
    /// assert_eq!(Decimal::parse("2").unwrap().round(2).to_string(), "2.00");
    /// ```
    pub fn round(&self, scale: u8) -> Decimal {
        let scale = scale.min(MAX_SCALE);
        if scale >= self.scale {
            let up = pow10(scale - self.scale);
            return match self.mantissa.checked_mul(up) {
                Some(m) if digits(m) <= MAX_PRECISION => Decimal { mantissa: m, scale },
                _ => *self,
            };
        }
        let div = pow10(self.scale - scale);
        let q = self.mantissa / div;
        let r = self.mantissa % div;
        let q = if r.abs() * 2 >= div {
            q + self.mantissa.signum()
        } else {
            q
        };
        Decimal { mantissa: q, scale }
    }

    /// The same number without trailing zeros in its decimals: `1.50` becomes `1.5`.
    pub fn normalized(&self) -> Decimal {
        let mut d = *self;
        while d.scale > 0 && d.mantissa % 10 == 0 {
            d.mantissa /= 10;
            d.scale -= 1;
        }
        d
    }

    /// Decimals that carry a value: `1.50` has one.
    pub fn fraction_digits(&self) -> u8 {
        self.normalized().scale
    }

    /// Digits before the decimal separator, without leading zeros: `0.5` has none.
    pub fn integer_digits(&self) -> u8 {
        digits(self.mantissa / pow10(self.scale))
    }

    /// The sum, with the larger scale, or `None` when it does not fit.
    pub fn checked_add(self, other: Decimal) -> Option<Decimal> {
        let scale = self.scale.max(other.scale);
        let a = self
            .mantissa
            .checked_mul(pow10(scale - self.scale))?;
        let b = other
            .mantissa
            .checked_mul(pow10(scale - other.scale))?;
        Decimal::new(a.checked_add(b)?, scale)
    }

    /// The quotient by a whole number, rounded half away from zero to `scale` decimals; `None`
    /// when `by` is zero or the result does not fit. Used for averages.
    pub fn div_int(self, by: i128, scale: u8) -> Option<Decimal> {
        if by == 0 {
            return None;
        }
        let work = scale.min(MAX_SCALE).max(self.scale);
        let num = self
            .mantissa
            .checked_mul(pow10(work - self.scale))?;
        let (q, r) = (num / by, num % by);
        let q = if r.unsigned_abs() * 2 >= by.unsigned_abs() {
            q + num.signum() * by.signum()
        } else {
            q
        };
        Decimal::new(q, work).map(|d| d.round(scale))
    }

    /// Compares with a whole number, as `min` and `max` rules do.
    pub fn cmp_int(&self, n: i64) -> Ordering {
        self.cmp(&Decimal::from_int(n))
    }

    /// The nearest floating point number. Only for output that cannot hold exact values, such
    /// as spreadsheet cells and chart coordinates.
    pub fn to_f64(&self) -> f64 {
        self.to_string().parse().unwrap_or(0.0)
    }

    /// The text bound for SQL: `12345E-2`. Both PostgreSQL `NUMERIC` and Oracle `TO_NUMBER`
    /// read it exactly and without depending on session language settings.
    #[cfg(any(feature = "postgres", feature = "oracle"))]
    pub(crate) fn sql_text(&self) -> String {
        if self.scale == 0 {
            self.mantissa.to_string()
        } else {
            format!("{}E-{}", self.mantissa, self.scale)
        }
    }

    /// Whole part and the fraction scaled to 28 decimals, both with the number's sign.
    fn parts(&self) -> (i128, i128) {
        let div = pow10(self.scale);
        (self.mantissa / div, (self.mantissa % div) * (TEN_28 / div))
    }
}

/// Digits of a whole number, without the sign; zero has none.
fn digits(n: i128) -> u8 {
    let mut n = n.unsigned_abs();
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Decimal {}

impl Hash for Decimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parts().hash(state);
    }
}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts().cmp(&other.parts())
    }
}

impl fmt::Display for Decimal {
    /// The fixed form with `.` and every decimal of the scale: `-1234.50`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.mantissa.unsigned_abs().to_string();
        let sign = if self.mantissa < 0 { "-" } else { "" };
        let scale = usize::from(self.scale);
        if scale == 0 {
            return write!(f, "{sign}{abs}");
        }
        let padded = format!("{abs:0>width$}", width = scale + 1);
        let (whole, frac) = padded.split_at(padded.len() - scale);
        write!(f, "{sign}{whole}.{frac}")
    }
}

impl std::str::FromStr for Decimal {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Decimal::parse(s).ok_or(())
    }
}

/// Written as a string (`"1234.50"`) so JSON readers never turn it into a float.
impl Serialize for Decimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Read from a string (`"1234.50"`) or a JSON number. Floats are read through their shortest
/// text form, so `0.1` stays `0.1`.
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = Decimal;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a decimal number such as \"12.50\"")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Decimal, E> {
                Decimal::parse(v).ok_or_else(|| E::custom("not a decimal number"))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Decimal, E> {
                Ok(Decimal::from_int(v))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Decimal, E> {
                Decimal::new(i128::from(v), 0).ok_or_else(|| E::custom("number too large"))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Decimal, E> {
                Decimal::parse(&v.to_string()).ok_or_else(|| E::custom("not a decimal number"))
            }
        }
        deserializer.deserialize_any(V)
    }
}
