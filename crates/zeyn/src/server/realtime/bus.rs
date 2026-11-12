//! Several app instances behind one Redis (features `realtime` and `redis`): every topic
//! notification, event and typing signal is delivered to this instance's connections and
//! published on one Redis channel; every instance delivers what the others publish to its own
//! connections. Sequence numbers stay per connection, so a resume that lands on another instance
//! gets `resync`. When Redis is down, sends stay local and a warning is logged once a minute.

use super::Socket;
use super::socket::To;
use crate::RedisStore;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The Redis side of a socket: where it publishes and who it is.
pub(super) struct Bus {
    store: RedisStore,
    channel: String,
    /// A random id, so an instance skips what it published itself.
    instance: String,
    started: AtomicBool,
    warned: Mutex<Option<Instant>>,
}

/// One delivery as it travels between instances.
#[derive(Serialize, Deserialize)]
pub(super) struct Note {
    i: String,
    tenant: String,
    #[serde(flatten)]
    pub(super) what: What,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub(super) enum What {
    Topic {
        topic: String,
    },
    Event {
        to: To,
        name: String,
        data: String,
    },
    Typing {
        topic: String,
        user: String,
        room: String,
    },
}

impl Bus {
    pub(super) fn new(store: RedisStore) -> Bus {
        let mut bytes = [0u8; 8];
        let _ = getrandom::getrandom(&mut bytes);
        Bus {
            channel: format!("{}socket", store.prefix()),
            store,
            instance: bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            started: AtomicBool::new(false),
            warned: Mutex::new(None),
        }
    }

    /// Logs a Redis problem at most once a minute.
    fn warn(&self, what: &str) {
        if let Ok(mut at) = self.warned.lock()
            && at.is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
        {
            *at = Some(Instant::now());
            tracing::warn!("socket: redis {what}; sends stay on this instance");
        }
    }
}

impl Socket {
    /// Publishes a delivery for the other instances, when the socket has a bus.
    pub(super) fn publish(&self, tenant: &str, what: What) {
        let Some(bus) = self.0.bus.get() else {
            return;
        };
        let note = Note {
            i: bus.instance.clone(),
            tenant: tenant.to_string(),
            what,
        };
        let Ok(payload) = serde_json::to_string(&note) else {
            return;
        };
        let socket = self.clone();
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        tokio::spawn(async move {
            let Some(bus) = socket.0.bus.get() else {
                return;
            };
            let sent = match bus.store.connection().await {
                Ok(mut c) => redis::cmd("PUBLISH")
                    .arg(&bus.channel)
                    .arg(payload)
                    .query_async::<i64>(&mut c)
                    .await
                    .map(|_| ()),
                Err(e) => Err(e),
            };
            if sent.is_err() {
                bus.warn("publish failed");
            }
        });
    }

    /// Starts listening to the other instances, once, when the first connection opens.
    pub(super) fn listen(&self) {
        let Some(bus) = self.0.bus.get() else {
            return;
        };
        if bus.started.swap(true, Ordering::AcqRel) {
            return;
        }
        let socket = self.clone();
        tokio::spawn(async move {
            let mut wait = Duration::from_millis(500);
            loop {
                let Some(bus) = socket.0.bus.get() else {
                    return;
                };
                match bus
                    .store
                    .client()
                    .get_async_pubsub()
                    .await
                {
                    Ok(mut pubsub) => {
                        if pubsub
                            .subscribe(&bus.channel)
                            .await
                            .is_ok()
                        {
                            wait = Duration::from_millis(500);
                            let mut stream = pubsub.on_message();
                            while let Some(msg) = stream.next().await {
                                let Ok(text) = msg.get_payload::<String>() else {
                                    continue;
                                };
                                if let Ok(note) = serde_json::from_str::<Note>(&text)
                                    && note.i != bus.instance
                                {
                                    socket.arrive(note);
                                }
                            }
                        }
                        bus.warn("subscription ended");
                    }
                    Err(_) => bus.warn("cannot be reached"),
                }
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(Duration::from_secs(30));
            }
        });
    }

    /// Delivers what another instance published to this instance's connections.
    fn arrive(&self, note: Note) {
        let tenant = Some(note.tenant.as_str()).filter(|t| !t.is_empty());
        match note.what {
            What::Topic { topic } => self.notify_local(tenant, &topic),
            What::Event { to, name, data } => {
                self.emit_local(&note.tenant, &to, &name, &data);
            }
            What::Typing { topic, user, room } => {
                self.typing_local(&note.tenant, &topic, &user, &room);
            }
        }
    }
}

impl super::ZeynSocket {
    /// Shares topics, events and typing signals with the other instances of the app through
    /// Redis, on the channel `zeyn:<prefix>:socket` of the store. Connections, groups and the
    /// replay buffers stay on the instance that holds the connection.
    pub fn redis(self, store: &RedisStore) -> Self {
        let _ = self
            .socket
            .0
            .bus
            .set(Bus::new(store.clone()));
        self
    }
}
