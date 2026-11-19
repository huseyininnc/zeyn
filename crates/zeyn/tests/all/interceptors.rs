//! Apps can write their own interceptors and audit stores with the exported types.

use std::sync::{Arc, Mutex};
use zeyn::{AuditEntry, AuditLog, AuditSink, Call, CallOutcome, Ctx, Interceptor};

/// Logs slow calls, as the architecture guide shows.
struct SlowCalls(Arc<Mutex<Vec<&'static str>>>);

impl Interceptor for SlowCalls {
    fn after(&self, _ctx: &Ctx, call: &Call, outcome: &CallOutcome) {
        if outcome.duration.as_millis() > 500 {
            tracing::warn!(screen = call.screen, "slow call");
        }
        if let Ok(mut seen) = self.0.lock() {
            seen.push(call.screen);
        }
    }
}

/// An app's own audit store.
struct Collect;

impl AuditSink for Collect {
    fn record(&self, entry: AuditEntry) {
        let _ = entry.user;
    }
}

#[test]
fn custom_interceptors_and_audit_sinks_can_be_written_by_apps() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let _app = zeyn::App::new("Test")
        .interceptor(SlowCalls(seen))
        .interceptor(AuditLog(Collect));
}
