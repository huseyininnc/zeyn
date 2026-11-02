//! Rate limits and IP rules.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// What a rate limit counts by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateKey {
    /// The client's IP address.
    Ip,
    /// The signed in user; anonymous requests fall back to the IP address.
    User,
}

/// At most `requests` per `window` for each key. Bursts up to `requests` are allowed, then the
/// allowance refills evenly over the window (token bucket).
///
/// ```ignore
/// App::new("Shop")
///     .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
///     .write_limit(RateLimit::per_user(30, Duration::from_secs(60)))
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateLimit {
    pub requests: u32,
    pub window: Duration,
    pub key: RateKey,
}

impl RateLimit {
    pub fn per_ip(requests: u32, window: Duration) -> Self {
        RateLimit { requests: requests.max(1), window, key: RateKey::Ip }
    }

    pub fn per_user(requests: u32, window: Duration) -> Self {
        RateLimit { requests: requests.max(1), window, key: RateKey::User }
    }
}

/// Counters behind a rate limit. The built in one keeps them in memory, which is right for a single
/// server; several servers behind a load balancer need a shared store such as Redis.
pub trait RateStore: Send + Sync + 'static {
    /// Takes one request from the key's allowance. `Err` carries how long to wait.
    fn take(&self, key: &str, limit: &RateLimit) -> Result<(), Duration>;

    /// Takes `amount` units at once, such as the tokens of a model answer. The allowance may go
    /// below zero, so a large answer is paid in full and the next call waits. `Err` when less than
    /// one unit is left (also with `amount` 0, which only checks). Stores that only count requests
    /// may keep the default, which takes one unit.
    fn charge(&self, key: &str, limit: &RateLimit, amount: u32) -> Result<(), Duration> {
        let _ = amount;
        self.take(key, limit)
    }
}

/// In memory token buckets with a cap on the number of keys, so a flood of addresses cannot
/// exhaust memory.
pub struct MemoryRateStore {
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
    max_keys: usize,
}

impl Default for MemoryRateStore {
    fn default() -> Self {
        MemoryRateStore { buckets: Mutex::new(HashMap::new()), max_keys: 100_000 }
    }
}

impl MemoryRateStore {
    pub fn with_max_keys(max_keys: usize) -> Self {
        MemoryRateStore { max_keys: max_keys.max(1), ..Self::default() }
    }

    pub fn len(&self) -> usize {
        self.buckets.lock().map(|b| b.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl RateStore for MemoryRateStore {
    fn take(&self, key: &str, limit: &RateLimit) -> Result<(), Duration> {
        self.bucket(key, limit, |tokens| if *tokens >= 1.0 { *tokens -= 1.0; true } else { false })
    }

    fn charge(&self, key: &str, limit: &RateLimit, amount: u32) -> Result<(), Duration> {
        self.bucket(key, limit, |tokens| {
            *tokens -= f64::from(amount);
            *tokens >= 1.0
        })
    }
}

impl MemoryRateStore {
    /// Refills the key's bucket and lets `use_tokens` take from it; `Err` with the wait until one
    /// unit is back when it says no.
    fn bucket(&self, key: &str, limit: &RateLimit, use_tokens: impl FnOnce(&mut f64) -> bool) -> Result<(), Duration> {
        let now = Instant::now();
        let capacity = f64::from(limit.requests);
        let per_sec = capacity / limit.window.as_secs_f64().max(0.001);
        let Ok(mut buckets) = self.buckets.lock() else { return Ok(()) };
        if buckets.len() >= self.max_keys && !buckets.contains_key(key) {
            buckets.retain(|_, (tokens, at)| {
                let refilled = *tokens + now.duration_since(*at).as_secs_f64() * per_sec;
                refilled < capacity
            });
            if buckets.len() >= self.max_keys {
                let oldest = buckets.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| k.clone());
                if let Some(k) = oldest {
                    buckets.remove(&k);
                }
            }
        }
        let (tokens, at) = buckets.entry(key.to_string()).or_insert((capacity, now));
        *tokens = (*tokens + now.duration_since(*at).as_secs_f64() * per_sec).min(capacity);
        *at = now;
        if use_tokens(tokens) { Ok(()) } else { Err(Duration::from_secs_f64((1.0 - *tokens).max(0.0) / per_sec)) }
    }
}

/// An address range such as `10.0.0.0/8`, `192.168.1.10` or `2001:db8::/32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpRange {
    base: IpAddr,
    prefix: u8,
}

impl IpRange {
    pub fn parse(s: &str) -> Option<IpRange> {
        let (ip, prefix) = match s.trim().split_once('/') {
            Some((ip, p)) => (ip.parse::<IpAddr>().ok()?, p.parse::<u8>().ok()?),
            None => {
                let ip: IpAddr = s.trim().parse().ok()?;
                (ip, if ip.is_ipv4() { 32 } else { 128 })
            }
        };
        let max = if ip.is_ipv4() { 32 } else { 128 };
        (prefix <= max).then_some(IpRange { base: ip, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let bits = |ip: IpAddr| -> (u128, u8) {
            match ip {
                IpAddr::V4(v) => (u128::from(u32::from(v)), 32),
                IpAddr::V6(v) => match v.to_ipv4_mapped() {
                    Some(v4) => (u128::from(u32::from(v4)), 32),
                    None => (u128::from(v), 128),
                },
            }
        };
        let (a, wa) = bits(self.base);
        let (b, wb) = bits(ip);
        if wa != wb {
            return false;
        }
        if self.prefix == 0 {
            return true;
        }
        let shift = u32::from(wa - self.prefix);
        (a >> shift) == (b >> shift)
    }
}

/// Parses a list of ranges; invalid entries are reported so a typo never opens a door silently.
pub fn parse_ranges<I, S>(ranges: I) -> Result<Vec<IpRange>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    ranges
        .into_iter()
        .map(|r| IpRange::parse(r.as_ref()).ok_or_else(|| format!("`{}` is not an IP address or range", r.as_ref())))
        .collect()
}

/// The client's address: the connection's peer, or, when the peer is a trusted proxy, the last
/// address in `X-Forwarded-For` that is not a trusted proxy.
pub fn client_ip(peer: Option<IpAddr>, forwarded_for: Option<&str>, trusted: &[IpRange]) -> Option<IpAddr> {
    let peer = peer?;
    if !trusted.iter().any(|r| r.contains(peer)) {
        return Some(peer);
    }
    let Some(chain) = forwarded_for else { return Some(peer) };
    chain
        .rsplit(',')
        .filter_map(|p| p.trim().parse::<IpAddr>().ok())
        .find(|ip| !trusted.iter().any(|r| r.contains(*ip)))
        .or(Some(peer))
}
