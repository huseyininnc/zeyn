//! Fractional index keys: order, growth, rebalance and parsing. Property style tests loop over
//! inputs from a seeded generator, so a failure repeats.

use zeyn::Rank;
use zeyn::prelude::*;
use zeyn::rank::{self, between, n_between, rebalance};

/// A small seeded generator (splitmix64) for repeatable random inputs.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_add(0x9E37_79B9_7F4A_7C15);
        let x = (self.0 ^ (self.0 >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn valid(r: &Rank) -> bool {
    Rank::parse(r.as_str()).as_ref() == Some(r)
}

#[test]
fn alphabet_order_is_byte_order() {
    let digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let bytes = digits.as_bytes();
    assert!(bytes.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn between_is_strictly_between_random_neighbors() {
    let mut rng = Seeded(7);
    let mut list = vec![between(None, None)];
    for _ in 0..5_000 {
        let at = rng.below(list.len() + 1);
        let a = at.checked_sub(1).map(|i| &list[i]);
        let b = list.get(at);
        let key = between(a, b);
        assert!(valid(&key), "{key}");
        assert!(a.is_none_or(|a| *a < key), "{a:?} < {key}");
        assert!(b.is_none_or(|b| key < *b), "{key} < {b:?}");
        list.insert(at, key);
        if list[at].needs_rebalance() {
            list = rebalance(list.len());
        }
    }
    assert!(list.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn ends_of_the_list_always_have_room() {
    let mut first = between(None, None);
    let mut last = first.clone();
    for _ in 0..200 {
        let before = between(None, Some(&first));
        let after = between(Some(&last), None);
        assert!(before < first && last < after);
        assert!(valid(&before) && valid(&after));
        first = before;
        last = after;
    }
    assert!(first.as_str().len() <= Rank::REBALANCE_LEN);
    assert!(last.as_str().len() <= Rank::REBALANCE_LEN);
}

#[test]
fn ten_thousand_inserts_after_the_first_key_rebalance_before_the_limit() {
    let mut list = rebalance(2);
    let mut longest = 0;
    let mut rebalances = 0;
    for _ in 0..10_000 {
        let key = between(Some(&list[0]), Some(&list[1]));
        let len = key.as_str().len();
        longest = longest.max(len);
        assert!(len <= Rank::MAX_LEN, "{len} characters before a rebalance");
        assert_eq!(key.needs_rebalance(), len > Rank::REBALANCE_LEN);
        assert!(list[0] < key && key < list[1]);
        list.insert(1, key);
        if list[1].needs_rebalance() {
            rebalances += 1;
            list = rebalance(list.len());
        }
    }
    assert!(rebalances > 0, "needs_rebalance never turned true");
    assert!(longest < Rank::MAX_LEN, "longest key {longest}");
    assert!(list.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn concurrent_inserts_between_the_same_neighbors_differ() {
    let a = Rank::parse("V").unwrap();
    let b = Rank::parse("W").unwrap();
    let keys: std::collections::HashSet<Rank> = (0..1_000)
        .map(|_| between(Some(&a), Some(&b)))
        .collect();
    assert!(keys.len() > 990, "{} distinct of 1000", keys.len());
}

#[test]
fn neighbors_in_the_wrong_order_or_equal_still_give_a_valid_key() {
    let a = Rank::parse("V").unwrap();
    let b = Rank::parse("k").unwrap();
    let swapped = between(Some(&b), Some(&a));
    assert!(a < swapped && swapped < b);
    let after = between(Some(&a), Some(&a));
    assert!(a < after);
}

#[test]
fn n_between_is_sorted_unique_and_inside() {
    let mut rng = Seeded(11);
    for _ in 0..200 {
        let ends = n_between(None, None, 2);
        let n = rng.below(300);
        let keys = n_between(Some(&ends[0]), Some(&ends[1]), n);
        assert_eq!(keys.len(), n);
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        assert!(
            keys.iter()
                .all(|k| ends[0] < *k && *k < ends[1] && valid(k))
        );
    }
    let open = n_between(None, None, 1_000);
    assert!(open.windows(2).all(|w| w[0] < w[1]));
    assert!(
        open.iter()
            .all(|k| !k.needs_rebalance())
    );
}

#[test]
fn rebalance_spreads_sorted_short_keys() {
    for n in [0, 1, 2, 61, 62, 63, 1_000, 3_843, 3_844, 100_000] {
        let keys = rebalance(n);
        assert_eq!(keys.len(), n);
        assert!(keys.windows(2).all(|w| w[0] < w[1]), "n = {n}");
        assert!(keys.iter().all(valid), "n = {n}");
        let longest = keys
            .iter()
            .map(|k| k.as_str().len())
            .max()
            .unwrap_or(0);
        assert!(longest <= 3, "n = {n}: {longest}");
    }
    assert_eq!(rebalance(1)[0].as_str(), "V");
}

#[test]
fn parse_rejects_bad_input() {
    for bad in [
        "",
        "0",
        "V0",
        "a b",
        "a-b",
        "ç",
        "Ä",
        "V\0",
        "z;DROP",
        "\u{1F600}",
    ] {
        assert!(Rank::parse(bad).is_none(), "{bad:?}");
    }
    assert!(Rank::parse(&"V".repeat(64)).is_some());
    assert!(Rank::parse(&"V".repeat(65)).is_none());
    assert!("Vz".parse::<Rank>().is_ok());
    assert!(serde_json::from_str::<Rank>("\"V\"").is_ok());
    assert!(serde_json::from_str::<Rank>("\"V-\"").is_err());
    assert!(serde_json::from_str::<Rank>(&format!("\"{}\"", "a".repeat(65))).is_err());
}

#[entity(table = "cards")]
pub struct Card {
    #[key]
    pub id: CardId,
    pub rank: Rank,
}

#[test]
fn memory_grids_sort_ranks_by_bytes() {
    let ranks = ["b", "B", "a", "Z", "1"];
    let rows: Vec<Card> = ranks
        .iter()
        .enumerate()
        .map(|(i, r)| Card {
            id: CardId(i as i64 + 1),
            rank: Rank::parse(r).unwrap(),
        })
        .collect();
    let q = DataQuery {
        sort: vec![zeyn::Sort {
            field: "rank".into(),
            desc: false,
        }],
        ..DataQuery::default()
    };
    let page = q.apply(rows);
    let order: Vec<&str> = page
        .items
        .iter()
        .map(|c| c.rank.as_str())
        .collect();
    assert_eq!(order, ["1", "B", "Z", "a", "b"]);
}

#[test]
fn between_timing() {
    let a = rank::between(None, None);
    let b = rank::between(Some(&a), None);
    let start = std::time::Instant::now();
    let mut sink = 0;
    for _ in 0..10_000 {
        sink += between(Some(&a), Some(&b))
            .as_str()
            .len();
    }
    let took = start.elapsed();
    eprintln!("10 000 between() calls: {took:?}");
    assert!(sink > 0);
}
