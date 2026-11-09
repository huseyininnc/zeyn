//! Fractional index keys for user ordered rows: sortable lists, kanban columns and tree moves.
//!
//! A [`Rank`] is a short base62 string. Rows sort by comparing their ranks byte by byte, and a
//! new key always fits between two neighbors, so a move writes one row instead of renumbering
//! the list.
//!
//! ```
//! use zeyn::rank;
//! let first = rank::between(None, None).unwrap();
//! let last = rank::between(Some(&first), None).unwrap();
//! let middle = rank::between(Some(&first), Some(&last)).unwrap();
//! assert!(first < middle && middle < last);
//!
//! let dropped = rank::n_between(Some(&first), Some(&middle), 3).unwrap();
//! assert!(first < dropped[0] && dropped[0] < dropped[1] && dropped[1] < dropped[2]);
//! assert!(dropped[2] < middle);
//! ```
//!
//! # Concurrent inserts
//!
//! Every key ends with [`JITTER`] random characters, so two requests that insert between the
//! same neighbors at the same moment get different keys. The jitter makes a clash unlikely, not
//! impossible: give the rank column a unique index (per list, such as `(board_id, rank)`) and
//! retry the insert once with a fresh key when the index refuses it. [`between`] returns `None`
//! rather than a key without room for its jitter; rebalance the list then.
//!
//! # Databases
//!
//! Ranks compare by bytes. The alphabet `0-9 A-Z a-z` is in ASCII order, so byte order is key
//! order, but only when the database compares bytes too: PostgreSQL columns need `COLLATE "C"`
//! and Oracle sorts with `NLSSORT(col, 'NLS_SORT=BINARY')`. `zeyn::sql` does both for `Rank`
//! fields (see `docs/database.md`).

use crate::value::{FieldKind, FieldType, Value};
use serde::{Deserialize, Deserializer, Serialize};
use std::cell::Cell;
use std::fmt;
use std::hash::{BuildHasher, Hasher};

/// The digits in value order, which is also their byte order.
const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Random characters at the end of every key from [`between`].
pub const JITTER: usize = 3;

/// A position in a user ordered list: 1 to [`Rank::MAX_LEN`] base62 characters (`0-9 A-Z a-z`),
/// not ending in `0`. Ranks compare byte by byte; store them in a column that does too.
///
/// Parsing ([`Rank::parse`], `FromStr`, `Deserialize`) is the trust boundary for values from a
/// client or a database: anything outside the alphabet, empty, longer than [`Rank::MAX_LEN`] or
/// ending in `0` is refused.
///
/// ```
/// use zeyn::Rank;
/// assert!(Rank::parse("V").is_some());
/// assert!(Rank::parse("a-b").is_none());
/// assert!(Rank::parse("V0").is_none());
/// assert!(Rank::parse(&"V".repeat(65)).is_none());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Rank(String);

impl Rank {
    /// Longest rank in bytes. The SQL column is `VARCHAR(64)`.
    pub const MAX_LEN: usize = 64;

    /// Past this length [`Rank::needs_rebalance`] is true. The 16 characters left before
    /// [`Rank::MAX_LEN`] hold many more inserts at the same spot.
    pub const REBALANCE_LEN: usize = 48;

    /// The rank if `s` is a valid one, else `None`.
    pub fn parse(s: &str) -> Option<Rank> {
        let ok = !s.is_empty()
            && s.len() <= Self::MAX_LEN
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric())
            && !s.ends_with('0');
        ok.then(|| Rank(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the key grew so long that the list should get fresh keys from [`rebalance`].
    /// Check it after an insert and rebalance the list in the background or in the same
    /// transaction.
    pub fn needs_rebalance(&self) -> bool {
        self.0.len() > Self::REBALANCE_LEN
    }
}

/// The value of a digit byte.
fn digit(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'A'..=b'Z' => b - b'A' + 10,
        _ => b - b'a' + 36,
    }
}

/// Writes a key strictly between `a` and `b` (`None` is the end of the list) to `out`. The key
/// is never a prefix of `b`, so characters appended to it keep it below `b`.
fn midpoint(a: &[u8], b: Option<&[u8]>, out: &mut Vec<u8>) {
    let mut a = a;
    if let Some(b) = b {
        let pad = a
            .iter()
            .copied()
            .chain(std::iter::repeat(b'0'));
        let common = b
            .iter()
            .zip(pad)
            .take_while(|(x, y)| **x == *y)
            .count();
        out.extend_from_slice(&b[..common]);
        a = a.get(common..).unwrap_or(&[]);
        let high = b.get(common).map_or(62, |&c| digit(c));
        let low = a.first().map_or(0, |&c| digit(c));
        if high - low > 1 {
            out.push(DIGITS[usize::from((low + high) / 2)]);
            return;
        }
        out.push(DIGITS[usize::from(low)]);
        a = a.get(1..).unwrap_or(&[]);
    }
    loop {
        let low = a.first().map_or(0, |&c| digit(c));
        if low < 61 {
            out.push(DIGITS[usize::from((low + 62) / 2)]);
            return;
        }
        out.push(DIGITS[usize::from(low)]);
        a = &a[1..];
    }
}

thread_local! {
    /// State of the jitter generator, seeded once per thread from the standard library's random
    /// hash keys.
    static SEED: Cell<u64> = Cell::new(
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
            | 1,
    );
}

/// The next number of a splitmix64 sequence. Jitter only has to differ between requests, it is
/// not a secret.
fn random() -> u64 {
    SEED.with(|s| {
        let x = s
            .get()
            .wrapping_add(0x9E37_79B9_7F4A_7C15);
        s.set(x);
        let x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    })
}

/// A new rank between two neighbors: `between(None, None)` for the first row of a list,
/// `between(Some(&last), None)` to append, `between(None, Some(&first))` to prepend.
///
/// The key is at most one character longer than the longer neighbor, plus [`JITTER`] random
/// characters. Neighbors given in the wrong order are swapped, and equal neighbors place the key
/// after them. `None` means the key would not fit in [`Rank::MAX_LEN`] with its jitter: give the
/// list fresh keys with [`rebalance`] and try again. Rebalancing when [`Rank::needs_rebalance`]
/// turns true keeps that from happening.
///
/// ```
/// use zeyn::Rank;
/// let a = Rank::parse(&"V".repeat(64)).unwrap();
/// let b = Rank::parse(&format!("{}W", "V".repeat(63))).unwrap();
/// assert!(zeyn::rank::between(Some(&a), Some(&b)).is_none());
/// ```
#[must_use = "None means the list needs a rebalance first"]
pub fn between(a: Option<&Rank>, b: Option<&Rank>) -> Option<Rank> {
    let (a, b) = match (a, b) {
        (Some(x), Some(y)) if x > y => (Some(y), Some(x)),
        (Some(x), Some(y)) if x == y => (Some(x), None),
        pair => pair,
    };
    let mut out = Vec::with_capacity(Rank::MAX_LEN + 1);
    midpoint(
        a.map_or(&[][..], |r| r.0.as_bytes()),
        b.map(|r| r.0.as_bytes()),
        &mut out,
    );
    if out.len() + JITTER > Rank::MAX_LEN {
        return None;
    }
    let mut bits = random();
    for i in 0..JITTER {
        let n = if i + 1 == JITTER {
            1 + bits % 61
        } else {
            bits % 62
        };
        out.push(DIGITS[n as usize]);
        bits /= 62;
    }
    Some(Rank(ascii(out)))
}

/// Text of digit bytes. Every byte comes from [`DIGITS`], which is ASCII, so the conversion
/// cannot fail.
fn ascii(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("rank digits are ASCII")
}

/// `n` sorted, distinct ranks between two neighbors, for a drop of several rows at once. The
/// keys split the gap evenly, so they grow by about one character per 62 rows. `None` when they
/// do not fit, as for [`between`].
#[must_use = "None means the list needs a rebalance first"]
pub fn n_between(a: Option<&Rank>, b: Option<&Rank>, n: usize) -> Option<Vec<Rank>> {
    let mut out = Vec::with_capacity(n);
    fill(a.cloned(), b, n, &mut out)?;
    Some(out)
}

/// Pushes `n` sorted ranks between `a` and `b` by splitting the gap in the middle.
fn fill(a: Option<Rank>, b: Option<&Rank>, n: usize, out: &mut Vec<Rank>) -> Option<()> {
    if n == 0 {
        return Some(());
    }
    let mid = between(a.as_ref(), b)?;
    fill(a, Some(&mid), n / 2, out)?;
    out.push(mid.clone());
    fill(Some(mid), b, n - n / 2 - 1, out)
}

/// `n` evenly spaced fresh ranks, as short as they can be, for renumbering a whole list in one
/// transaction when [`Rank::needs_rebalance`] turns true.
///
/// ```
/// let keys = zeyn::rank::rebalance(3);
/// let keys: Vec<&str> = keys.iter().map(|r| r.as_str()).collect();
/// assert_eq!(keys, ["F", "V", "k"]);
/// ```
pub fn rebalance(n: usize) -> Vec<Rank> {
    let gaps = n as u128 + 1;
    let mut len = 1;
    let mut span: u128 = 62;
    while span < gaps {
        len += 1;
        span *= 62;
    }
    (1..=n as u128)
        .map(|i| {
            let mut v = i * span / gaps;
            let mut key = vec![b'0'; len];
            for slot in key.iter_mut().rev() {
                *slot = DIGITS[(v % 62) as usize];
                v /= 62;
            }
            while key.last() == Some(&b'0') {
                key.pop();
            }
            Rank(ascii(key))
        })
        .collect()
}

/// `V`, the middle of the key space, so a new row has a valid rank before it gets a real one.
impl Default for Rank {
    fn default() -> Self {
        Rank("V".into())
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Rank {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for Rank {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Rank::parse(s).ok_or("a rank is 1 to 64 characters of 0-9, A-Z and a-z, not ending in 0")
    }
}

/// Accepts only valid ranks, so a value from a client or a database row is checked on the way in.
impl<'de> Deserialize<'de> for Rank {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = std::borrow::Cow::<str>::deserialize(d)?;
        s.parse()
            .map_err(serde::de::Error::custom)
    }
}

impl FieldType for Rank {
    const KIND: FieldKind = FieldKind::Rank;
    fn to_value(&self) -> Value {
        Value::Text(self.0.clone())
    }
    fn __text(&self) -> Option<&str> {
        Some(&self.0)
    }
}

impl crate::value::SupportsRequired for Rank {}
