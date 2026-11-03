//! Shared state in Redis for apps that run on several servers (Cargo feature `redis`).
//!
//! The server keeps two kinds of state that must be the same on every instance: rate limit
//! counters and saved per user layouts (grid views and dock layouts, both in the view store).
//! Everything else per user lives in the browser or in signed cookies, so one [`RedisStore`]
//! covers both:
//!
//! ```ignore
//! let redis = RedisStore::new("redis://cache:6379", "billing")?;
//! App::new("Billing").rate_store(redis.clone()).datagrid_views(redis)
//! ```

use crate::action::{Error, Result};
use crate::ctx::BoxFuture;
use crate::grid::{DataGridView, DataGridViewStore};
use crate::security::{RateLimit, RateStore};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;

/// Token bucket in one atomic step, the same algorithm as the memory store. Returns how many
/// milliseconds to wait, 0 when the request may go ahead. Time comes from Redis, so the clocks of
/// the app servers do not matter.
const TAKE: &str = r"
local capacity = tonumber(ARGV[1])
local per_ms = tonumber(ARGV[2])
local clock = redis.call('TIME')
local now = tonumber(clock[1]) * 1000 + math.floor(tonumber(clock[2]) / 1000)
local bucket = redis.call('HMGET', KEYS[1], 't', 'at')
local tokens = tonumber(bucket[1]) or capacity
local at = tonumber(bucket[2]) or now
tokens = math.min(capacity, tokens + math.max(0, now - at) * per_ms)
local wait = 0
if tokens >= 1 then tokens = tokens - 1 else wait = math.ceil((1 - tokens) / per_ms) end
redis.call('HSET', KEYS[1], 't', tostring(tokens), 'at', now)
redis.call('PEXPIRE', KEYS[1], ARGV[3])
return wait
";

/// How long calls skip Redis after it failed, so an outage costs one timeout per five seconds
/// and not one per request.
const RETRY_AFTER_MS: u64 = 5_000;

/// Rate limit counters and saved views in Redis, shared by every instance of an app.
///
/// - Connects on first use and reconnects by itself. Calls time out after 500 ms.
/// - When Redis cannot be reached, rate limits let requests through and views are neither loaded
///   nor saved; the outage and the recovery are each logged once. After a failure Redis is left
///   alone for five seconds, so requests do not each wait for a timeout.
/// - Keys start with `zeyn:<prefix>:`, so several apps can share one Redis. Rate counters expire
///   one window after their last use; views never expire.
#[derive(Clone)]
pub struct RedisStore(Arc<Inner>);

struct Inner {
    client: redis::Client,
    connection: OnceCell<ConnectionManager>,
    prefix: String,
    script: redis::Script,
    down: AtomicBool,
    /// While down, Redis is not called again before this many milliseconds after `started`.
    retry_at: AtomicU64,
    started: Instant,
}

impl RedisStore {
    /// A store for the Redis at `url` (`redis://host:6379`, `rediss://` for TLS is not built in).
    /// Fails only when the URL cannot be read; the connection is made on first use.
    pub fn new(url: &str, prefix: &str) -> std::result::Result<RedisStore, String> {
        let client = redis::Client::open(url).map_err(|e| format!("redis url: {e}"))?;
        Ok(RedisStore(Arc::new(Inner {
            client,
            connection: OnceCell::new(),
            prefix: format!("zeyn:{prefix}:"),
            script: redis::Script::new(TAKE),
            down: AtomicBool::new(false),
            retry_at: AtomicU64::new(0),
            started: Instant::now(),
        })))
    }

    async fn connection(&self) -> redis::RedisResult<ConnectionManager> {
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(1)))
            .set_response_timeout(Some(Duration::from_millis(500)))
            .set_number_of_retries(1);
        let inner = &self.0;
        inner
            .connection
            .get_or_try_init(|| {
                inner
                    .client
                    .get_connection_manager_with_config(config)
            })
            .await
            .cloned()
    }

    /// Notes whether the last call reached Redis, logging only when that changes.
    fn health<T>(&self, result: redis::RedisResult<T>) -> Option<T> {
        match result {
            Ok(v) => {
                if self
                    .0
                    .down
                    .swap(false, Ordering::Relaxed)
                {
                    tracing::info!("redis is reachable again");
                }
                Some(v)
            }
            Err(e) => {
                self.0
                    .retry_at
                    .store(self.elapsed_ms() + RETRY_AFTER_MS, Ordering::Relaxed);
                if !self
                    .0
                    .down
                    .swap(true, Ordering::Relaxed)
                {
                    tracing::warn!(
                        "redis is unreachable, rate limits are off and views are not saved: {e}"
                    );
                }
                None
            }
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.0.started.elapsed().as_millis() as u64
    }

    /// Whether Redis failed a moment ago, so calls skip it instead of waiting for a timeout.
    fn resting(&self) -> bool {
        self.0.down.load(Ordering::Relaxed)
            && self.elapsed_ms() < self.0.retry_at.load(Ordering::Relaxed)
    }

    /// Key of a saved view. The user part is length prefixed so no user and grid pair can
    /// produce the key of another.
    fn view_key(&self, user: &str, grid: &str) -> String {
        format!("{}view:{}:{user}:{grid}", self.0.prefix, user.len())
    }
}

impl RateStore for RedisStore {
    fn take(&self, key: &str, limit: &RateLimit) -> BoxFuture<std::result::Result<(), Duration>> {
        let store = self.clone();
        let key = format!("{}rate:{key}", self.0.prefix);
        let window_ms = limit
            .window
            .as_millis()
            .clamp(1, u128::from(u32::MAX)) as u64;
        let per_ms = f64::from(limit.requests) / window_ms as f64;
        let capacity = limit.requests;
        Box::pin(async move {
            if store.resting() {
                return Ok(());
            }
            let result = match store.connection().await {
                Ok(mut con) => {
                    store
                        .0
                        .script
                        .key(key)
                        .arg(capacity)
                        .arg(per_ms)
                        .arg(window_ms)
                        .invoke_async::<u64>(&mut con)
                        .await
                }
                Err(e) => Err(e),
            };
            match store.health(result) {
                Some(0) | None => Ok(()),
                Some(wait) => Err(Duration::from_millis(wait)),
            }
        })
    }
}

impl DataGridViewStore for RedisStore {
    fn load(&self, user: &str, grid: &str) -> BoxFuture<Result<Option<DataGridView>>> {
        let (store, key) = (self.clone(), self.view_key(user, grid));
        Box::pin(async move {
            if store.resting() {
                return Err(unavailable());
            }
            let mut con = store
                .health(store.connection().await)
                .ok_or_else(unavailable)?;
            let json: Option<String> = store
                .health(
                    redis::cmd("GET")
                        .arg(&key)
                        .query_async(&mut con)
                        .await,
                )
                .ok_or_else(unavailable)?;
            Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
        })
    }

    fn save(&self, user: &str, grid: &str, view: Option<DataGridView>) -> BoxFuture<Result<()>> {
        let (store, key) = (self.clone(), self.view_key(user, grid));
        Box::pin(async move {
            if store.resting() {
                return Err(unavailable());
            }
            let mut con = store
                .health(store.connection().await)
                .ok_or_else(unavailable)?;
            let cmd = match view {
                Some(v) => redis::cmd("SET")
                    .arg(&key)
                    .arg(serde_json::to_string(&v).map_err(|e| Error::Internal(e.to_string()))?)
                    .to_owned(),
                None => redis::cmd("DEL").arg(&key).to_owned(),
            };
            store
                .health(cmd.query_async::<()>(&mut con).await)
                .ok_or_else(unavailable)
        })
    }
}

fn unavailable() -> Error {
    Error::Internal("view store is unavailable".into())
}
