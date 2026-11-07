//! Inbox and NotificationCenter examples: notifications by day with bulk actions, and the bell
//! with the unread count. The sample notifications are shared by every visitor and come back
//! when the last one is archived.

use crate::{Entry, ex};
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;
use zeyn::ui::ListItem;

/// A notification of the sample inbox.
#[entity(table = "notifications")]
pub struct Notification {
    #[key]
    pub id: NotificationId,
    pub title: String,
    pub detail: String,
    pub created_at: DateTime,
    pub read: bool,
}

/// The sample notifications, relative to when the catalog started.
fn sample() -> Vec<Notification> {
    let now = DateTime::now();
    let hour: i64 = 3_600_000;
    let rows = [
        (
            "Order SO-1042 was paid",
            "Payment of $129.90 received",
            1,
            false,
        ),
        (
            "Stock of Green tea is low",
            "12 units left in Warehouse A",
            3,
            false,
        ),
        (
            "Weekly report is ready",
            "Sales rose 8% against last week",
            26,
            true,
        ),
        ("New customer signed up", "Ada Lovelace, London", 30, false),
        ("Import finished", "1,200 products updated", 75, true),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (title, detail, hours, read))| Notification {
            id: NotificationId(i as i64 + 1),
            title: title.to_string(),
            detail: detail.to_string(),
            created_at: now.add_ms(-(*hours as i64) * hour),
            read: *read,
        })
        .collect()
}

/// Most sample notifications the shared store holds. Visitors only mark and archive them, so it
/// never grows past the samples; the cap holds it there even so.
pub const MAX_SAMPLE_NOTES: usize = 5;

static NOTES: LazyLock<Mutex<Vec<Notification>>> = LazyLock::new(|| Mutex::new(sample()));

/// How many sample notifications the shared store holds now.
pub fn stored() -> usize {
    notes().len()
}

fn notes() -> std::sync::MutexGuard<'static, Vec<Notification>> {
    NOTES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// The notifications of the sample inbox.
#[action(public)]
pub async fn sample_notifications(q: DataQuery) -> Result<Page<Notification>> {
    Ok(q.apply(notes().clone()))
}

/// Marks the chosen sample notifications as read.
#[action(public)]
pub async fn mark_sample_read(s: InboxSelection) -> Result<()> {
    for n in notes().iter_mut() {
        if s.ids.contains(&n.id.0.to_string()) {
            n.read = true;
        }
    }
    Ok(())
}

/// Archives the chosen sample notifications; the samples come back when none is left.
#[action(public)]
pub async fn archive_sample(s: InboxSelection) -> Result<()> {
    let mut all = notes();
    all.retain(|n| !s.ids.contains(&n.id.0.to_string()));
    if all.is_empty() {
        *all = sample();
    }
    all.truncate(MAX_SAMPLE_NOTES);
    Ok(())
}

/// Unread sample notifications, counted up to the limit the server sets.
#[action(public)]
pub async fn count_sample_unread(q: UnreadCount) -> Result<u32> {
    let n = notes()
        .iter()
        .filter(|n| !n.read)
        .take(q.limit as usize)
        .count();
    Ok(n as u32)
}

pub fn overview(_ctx: &Ctx) -> Node {
    Inbox::of(sample_notifications)
        .title("Notifications")
        .when(Notification::created_at)
        .read(Notification::read)
        .item(|n: &Notification| ListItem::new(n.title.clone()).subtitle(n.detail.clone()))
        .mark_read(mark_sample_read)
        .archive(archive_sample)
        .into()
}

pub fn bell(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(NotificationCenter::of(
            count_sample_unread,
            crate::inbox,
            (),
        ))
        .into()
}

/// What the catalog shows of `Inbox`.
pub const ENTRY: Entry = Entry {
    name: "Inbox",
    docs: "components/admin#inbox-and-notificationcenter",
    api: &["Inbox", "NotificationCenter"],
    overview: &[ex!(overview)],
    examples: &[ex!(bell)],
};
