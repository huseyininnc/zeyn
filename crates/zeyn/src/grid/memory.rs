//! In memory filtering, sorting, paging and totals: [`DataQuery::apply`] for small lists, samples
//! and tests, and the filter conditions (`Cond`, `Key`) that the SQL builder shares.

use super::*;

/// A comparable bound or value for range filters. Numbers and money compare in hundredths.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub(crate) enum Key {
    Num(i128),
    /// An exact decimal bound or value.
    Dec(crate::Decimal),
    Date(Date),
    Time(Time),
    Moment(DateTime),
    Local(LocalDateTime),
}

impl Key {
    fn of(value: &Value) -> Option<Key> {
        Some(match value {
            Value::Int(x) => Key::Num(i128::from(*x) * 100),
            Value::Money(m) => Key::Num(i128::from(m.minor)),
            Value::Number(d) => Key::Dec(*d),
            Value::Date(d) => Key::Date(*d),
            Value::Time(t) => Key::Time(*t),
            Value::DateTime(d) => Key::Moment(*d),
            Value::LocalDateTime(l) => Key::Local(*l),
            _ => return None,
        })
    }

    fn same_kind(&self, other: &Key) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Reads `1234.56` (or `1234,56`) as hundredths. At most two decimals.
fn hundredths(s: &str) -> Option<i128> {
    let s = s.trim();
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let (major, minor) = match digits.split_once(['.', ',']) {
        Some((a, b)) => (a, b),
        None => (digits, ""),
    };
    let ok = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    if major.is_empty() || major.len() > 30 || minor.len() > 2 || !ok(major) || !ok(minor) {
        return None;
    }
    let value = major.parse::<i128>().ok()? * 100 + format!("{minor:0<2}").parse::<i128>().ok()?;
    Some(if negative { -value } else { value })
}

/// Midnight starting `date` in `tz`; when the clocks skip midnight, the first hour that exists.
fn day_start(date: Date, tz: TimeZone) -> Option<DateTime> {
    (0..3u8).find_map(|h| DateTime::from_local(LocalDateTime::new(date, Time::new(h, 0, 0)?), tz).ok())
}

/// Reads one range bound for a column kind. `upper` widens a date to the end of that day.
fn bound(kind: &FieldKind, raw: &str, tz: TimeZone, upper: bool) -> Option<Key> {
    let s = raw.trim();
    if s.is_empty() || s.chars().count() > MAX_FILTER_TEXT {
        return None;
    }
    match kind {
        FieldKind::Integer | FieldKind::Id | FieldKind::Money => hundredths(s).map(Key::Num),
        FieldKind::Decimal => crate::Decimal::parse(&s.replace(',', ".")).map(Key::Dec),
        FieldKind::Date => Date::parse(s).map(Key::Date),
        FieldKind::Time => Time::parse(s).map(Key::Time),
        FieldKind::LocalDateTime => match Date::parse(s) {
            Some(d) if upper => Some(Key::Local(LocalDateTime::new(d, Time::new(23, 59, 59)?))),
            Some(d) => Some(Key::Local(LocalDateTime::new(d, Time::default()))),
            None => LocalDateTime::parse(s).map(Key::Local),
        },
        FieldKind::DateTime => match Date::parse(s) {
            Some(d) if upper => day_start(d.add_days(1)?, tz).map(|m| Key::Moment(m.add_ms(-1))),
            Some(d) => day_start(d, tz).map(Key::Moment),
            None => DateTime::parse(s)
                .or_else(|| LocalDateTime::parse(s).and_then(|l| DateTime::from_local(l, tz).ok()))
                .map(Key::Moment),
        },
        _ => None,
    }
}

pub(crate) enum TextOp {
    Contains,
    StartsWith,
    Equals,
}

/// One condition of a filter, read once before the rows are scanned.
pub(crate) enum Cond {
    Text(usize, TextOp, String),
    Range(usize, Option<Key>, Option<Key>),
    AnyOf(usize, Vec<String>),
    Is(usize, bool),
    Blank(usize, bool),
    AnyKey(usize, Vec<String>),
    /// Free text ([`DataQuery::q`]), folded, in any of these text fields.
    Search(Vec<usize>, String),
}

/// Text in the form free text search compares: `İ` and `ı` become `i`, then lower case.
pub(crate) fn fold(s: &str) -> String {
    s.chars().map(|c| if matches!(c, 'İ' | 'ı') { 'i' } else { c }).flat_map(char::to_lowercase).collect()
}

impl Cond {
    /// The free text condition of a query, over the unmasked text fields. `None` when the text is
    /// blank or longer than [`crate::MAX_SEARCH_QUERY`] characters.
    pub(crate) fn search(q: &str, fields: &[crate::model::FieldMeta]) -> Option<Cond> {
        let q = q.trim();
        if q.is_empty() || q.chars().count() > crate::MAX_SEARCH_QUERY {
            return None;
        }
        let cols = fields
            .iter()
            .enumerate()
            .filter(|(_, f)| matches!(f.kind, FieldKind::Text) && f.mask.is_none())
            .map(|(i, _)| i)
            .collect();
        Some(Cond::Search(cols, fold(q)))
    }
}

impl Cond {
    /// Reads the conditions of one filter. Unknown fields, unreadable values and conditions that
    /// do not fit the column kind are dropped.
    pub(crate) fn read(f: &Filter, fields: &[crate::model::FieldMeta], tz: TimeZone, out: &mut Vec<Cond>) {
        let Some(i) = fields.iter().position(|m| m.name == f.field) else {
            return;
        };
        let kind = &fields[i].kind;
        let text = |s: &str| {
            let t = s.trim();
            (!t.is_empty() && t.chars().count() <= MAX_FILTER_TEXT).then(|| t.to_lowercase())
        };
        if let Some(t) = text(&f.contains) {
            out.push(Cond::Text(i, TextOp::Contains, t));
        }
        let textual = matches!(kind, FieldKind::Text | FieldKind::Choice(_)) || kind.is_json();
        if textual {
            if let Some(t) = f.starts_with.as_deref().and_then(text) {
                out.push(Cond::Text(i, TextOp::StartsWith, t));
            }
            if let Some(t) = f.equals.as_deref().and_then(text) {
                out.push(Cond::Text(i, TextOp::Equals, t));
            }
        }
        let lo = f.from.as_deref().and_then(|s| bound(kind, s, tz, false));
        let hi = f.to.as_deref().and_then(|s| bound(kind, s, tz, true));
        if lo.is_some() || hi.is_some() {
            out.push(Cond::Range(i, lo, hi));
        }
        if matches!(kind, FieldKind::Id | FieldKind::Integer | FieldKind::Text | FieldKind::Custom { .. })
            && !f.any_of.is_empty()
        {
            out.push(Cond::AnyKey(i, f.any_of.iter().take(MAX_PAGE_SIZE as usize).cloned().collect()));
        }
        if let FieldKind::Choice(options) = kind {
            let wanted: Vec<String> = f
                .any_of
                .iter()
                .take(MAX_FILTER_CHOICES)
                .filter(|v| options.iter().any(|o| o.value == v.as_str()))
                .cloned()
                .collect();
            if !f.any_of.is_empty() {
                out.push(Cond::AnyOf(i, wanted));
            }
        }
        if let (FieldKind::Bool, Some(b)) = (kind, f.is) {
            out.push(Cond::Is(i, b));
        }
        if let Some(b) = f.blank {
            out.push(Cond::Blank(i, b));
        }
    }

    fn holds<M: Model>(&self, row: &M) -> bool {
        let value = |i: &usize| row.value_at(*i).unwrap_or(Value::Null);
        match self {
            Cond::Text(i, op, needle) => {
                let v = value(i);
                if v == Value::Null {
                    return false;
                }
                let hay = filter_text(&v);
                match op {
                    TextOp::Contains => hay.contains(needle.as_str()),
                    TextOp::StartsWith => hay.starts_with(needle.as_str()),
                    TextOp::Equals => hay == *needle,
                }
            }
            Cond::Range(i, lo, hi) => {
                let Some(k) = Key::of(&value(i)) else {
                    return false;
                };
                let above = lo.is_none_or(|lo| lo.same_kind(&k) && k >= lo);
                let below = hi.is_none_or(|hi| hi.same_kind(&k) && k <= hi);
                above && below
            }
            Cond::AnyOf(i, wanted) => match value(i) {
                Value::Choice(c) => wanted.iter().any(|w| w == c),
                _ => false,
            },
            Cond::Is(i, b) => value(i) == Value::Bool(*b),
            Cond::Blank(i, b) => (value(i) == Value::Null) == *b,
            Cond::AnyKey(i, keys) => {
                let v = value(i);
                v != Value::Null && keys.contains(&key_text(&v))
            }
            Cond::Search(cols, needle) => cols.iter().any(|i| match value(i) {
                Value::Text(t) => fold(&t).contains(needle.as_str()),
                _ => false,
            }),
        }
    }
}

/// Text used to filter a value: what the user sees, lower cased.
fn filter_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Text(s) => s.to_lowercase(),
        Value::Money(m) => m.format_plain("en"),
        Value::Choice(c) => c.to_lowercase(),
        Value::Date(d) => d.to_string(),
        Value::Time(t) => t.to_string(),
        Value::DateTime(d) => d.to_string(),
        Value::LocalDateTime(l) => l.to_string(),
        Value::Decimal(x) => x.to_string(),
        Value::Number(d) => d.to_string(),
    }
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Money(x), Value::Money(y)) => x.minor.cmp(&y.minor),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Text(x), Value::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()).then_with(|| x.cmp(y)),
        (Value::Choice(x), Value::Choice(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        (Value::Time(x), Value::Time(y)) => x.cmp(y),
        (Value::DateTime(x), Value::DateTime(y)) => x.cmp(y),
        (Value::LocalDateTime(x), Value::LocalDateTime(y)) => x.cmp(y),
        (Value::Decimal(x), Value::Decimal(y)) => x.total_cmp(y),
        (Value::Number(x), Value::Number(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

/// Computes one total over a column's values. Nulls are skipped. `None` when the op does not fit
/// the values (a sum of text, or money in different currencies). Money averages round half away
/// from zero to the smallest unit.
pub fn total(values: impl Iterator<Item = Value>, op: Aggregate) -> Option<Value> {
    let mut values = values.filter(|v| *v != Value::Null).peekable();
    match op {
        Aggregate::Count => Some(Value::Int(values.count() as i64)),
        Aggregate::Min => values.min_by(compare),
        Aggregate::Max => values.max_by(compare),
        Aggregate::Sum | Aggregate::Avg if values.peek().is_some_and(|v| matches!(v, Value::Number(_))) => {
            let (mut sum, mut n, mut scale) = (crate::Decimal::ZERO, 0i128, 0u8);
            for v in values {
                let Value::Number(d) = v else { return None };
                sum = sum.checked_add(d)?;
                scale = scale.max(d.scale());
                n += 1;
            }
            match op {
                Aggregate::Sum => Some(Value::Number(sum)),
                _ => sum.div_int(n, scale).map(Value::Number),
            }
        }
        Aggregate::Sum | Aggregate::Avg => {
            let (mut sum, mut n, mut currency, mut money) = (0i128, 0i128, None, false);
            for v in values {
                match v {
                    Value::Int(x) => sum += i128::from(x),
                    Value::Money(m) => {
                        if currency.is_some_and(|c| c != m.currency) {
                            return None;
                        }
                        currency = Some(m.currency);
                        money = true;
                        sum += i128::from(m.minor);
                    }
                    _ => return None,
                }
                n += 1;
            }
            let clamp = |x: i128| x.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
            let wrap = |x: i64| {
                if money { Value::Money(Money::new(x, currency.unwrap_or_default())) } else { Value::Int(x) }
            };
            match op {
                Aggregate::Sum => Some(wrap(clamp(sum))),
                _ if n == 0 => Some(Value::Null),
                _ if money => Some(wrap(clamp((2 * sum + sum.signum() * n) / (2 * n)))),
                _ => Some(Value::Decimal(sum as f64 / n as f64)),
            }
        }
    }
}

impl DataQuery {
    /// Filters, sorts and pages an in memory list. Meant for small data, samples and tests;
    /// databases do this in SQL. Unknown field names and unreadable filter values are ignored,
    /// and at most [`MAX_FILTERS`] filters and [`MAX_SORT`] sort keys are used. Sorting always ends with the
    /// key field so pages are stable.
    pub fn apply<M: Model>(&self, rows: Vec<M>) -> Page<M> {
        let fields = M::fields();
        let index = |name: &str| fields.iter().position(|f| f.name == name);
        let mut conds = Vec::new();
        for f in self.filters.iter().take(MAX_FILTERS) {
            Cond::read(f, fields, self.time_zone, &mut conds);
        }
        conds.extend(Cond::search(&self.q, fields));
        let mut rows: Vec<M> = rows.into_iter().filter(|row| conds.iter().all(|c| c.holds(row))).collect();
        let mut order: Vec<(usize, bool)> =
            self.sort.iter().take(MAX_SORT).filter_map(|s| index(&s.field).map(|i| (i, s.desc))).collect();
        if let Some(key) = fields.iter().position(|f| f.key) {
            order.push((key, false));
        }
        rows.sort_by(|a, b| {
            for (i, desc) in &order {
                let (x, y) = (a.value_at(*i).unwrap_or(Value::Null), b.value_at(*i).unwrap_or(Value::Null));
                let o = compare(&x, &y);
                let o = if *desc { o.reverse() } else { o };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
        let mut page = Page::from_slice(&rows, self);
        page.totals = self
            .aggregates
            .iter()
            .filter_map(|a| {
                let i = index(&a.field)?;
                let value = total(rows.iter().filter_map(|r| r.value_at(i)), a.op)?;
                Some(Total { field: a.field.clone(), op: a.op, value })
            })
            .collect();
        if let Some(gi) = self.group.as_deref().and_then(index) {
            let desc = self.sort.iter().any(|s| s.desc && Some(gi) == index(&s.field));
            let mut sorted: Vec<&M> = rows.iter().collect();
            sorted.sort_by(|a, b| {
                let o = compare(&a.value_at(gi).unwrap_or(Value::Null), &b.value_at(gi).unwrap_or(Value::Null));
                if desc { o.reverse() } else { o }
            });
            let mut buckets: Vec<(Value, Vec<&M>)> = Vec::new();
            for row in sorted {
                let v = row.value_at(gi).unwrap_or(Value::Null);
                match buckets.last_mut() {
                    Some((g, list)) if *g == v => list.push(row),
                    _ => buckets.push((v, vec![row])),
                }
            }
            let all: Vec<crate::action::Group> = buckets
                .into_iter()
                .map(|(value, list)| crate::action::Group {
                    value,
                    count: list.len() as u64,
                    totals: self
                        .aggregates
                        .iter()
                        .filter_map(|a| {
                            let i = index(&a.field)?;
                            let value = total(list.iter().filter_map(|r| r.value_at(i)), a.op)?;
                            Some(Total { field: a.field.clone(), op: a.op, value })
                        })
                        .collect(),
                })
                .collect();
            let groups = Page::from_slice(&all, self);
            page.items.clear();
            page.total = groups.total;
            page.next = groups.next;
            page.groups = groups.items;
        }
        if !self.with_total && matches!(self.page, PageRequest::After { .. }) {
            page.total = None;
        }
        page
    }
}

/// Cursor for a row position in an in memory list.
pub fn position_cursor(position: usize) -> Cursor {
    Cursor(position.to_string())
}
