//! Logs and traces through `tracing`.
//!
//! The framework emits spans and events; it never writes to standard error itself. When an app
//! is served and has not installed a subscriber of its own, [`init`] installs one that prints
//! events to standard error, and with the `otel` Cargo feature also exports spans over OTLP.
//! Standard error gets events only, so without OTLP export no span is ever built: the request,
//! `run` and `interceptor` spans then cost one cached check each, as they do with no subscriber
//! at all (tests, `App::router` mounted in another server).
//!
//! Configuration keys:
//!
//! - `log.level`: a filter such as `info` (the default), `debug` or `warn,zeyn=debug`.
//! - `otel.endpoint`: base address of an OTLP/HTTP collector, such as `http://collector:4318`.
//!   Spans go to `<endpoint>/v1/traces`. Export is off without it.
//! - `otel.service_name`: the service name in traces; the app's name by default.
//!
//! Only names, ids and codes are recorded: action, screen, user id, tenant id, status, error code
//! and durations. Field values, form input, query strings and secrets never are.

use crate::platform::Config;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::{FilterExt, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs the default subscriber unless the app set one. Returns what must be flushed at
/// shutdown, if anything.
pub(crate) fn init(config: &Config, service: &str) -> Option<Flush> {
    if tracing::dispatcher::has_been_set() {
        return None;
    }
    let level = config
        .get("log.level")
        .unwrap_or("info");
    let (filter, bad_level) = match level.parse::<Targets>() {
        Ok(f) => (f, false),
        Err(_) => (Targets::new().with_default(tracing::Level::INFO), true),
    };
    let events_only = filter
        .clone()
        .and(tracing_subscriber::filter::filter_fn(|m| m.is_event()));
    let stderr = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_filter(events_only);
    #[cfg(feature = "otel")]
    let (otel, provider, otel_error) = match otel::exporter(config, service) {
        Ok(Some(provider)) => (
            Some(otel::layer(&provider).with_filter(filter)),
            Some(provider),
            None,
        ),
        Ok(None) => (None, None, None),
        Err(e) => (None, None, Some(e)),
    };
    #[cfg(not(feature = "otel"))]
    let (otel, provider, otel_error): (
        Option<tracing_subscriber::layer::Identity>,
        Option<()>,
        Option<String>,
    ) = {
        let _ = (service, filter);
        (None, None, None)
    };
    if tracing_subscriber::registry()
        .with(stderr)
        .with(otel)
        .try_init()
        .is_err()
    {
        return None;
    }
    if bad_level {
        tracing::warn!(level, "log.level is not a valid filter; using info");
    }
    if let Some(e) = otel_error {
        tracing::warn!("OTLP export is off: {e}");
    }
    provider.map(Flush)
}

/// Spans still waiting to be exported.
#[cfg(feature = "otel")]
pub(crate) struct Flush(opentelemetry_sdk::trace::SdkTracerProvider);

/// Spans still waiting to be exported.
#[cfg(not(feature = "otel"))]
pub(crate) struct Flush(());

impl Flush {
    /// Sends what is left, off the async threads because the exporter blocks. Called when the
    /// server stops.
    pub(crate) async fn shutdown(self) {
        #[cfg(feature = "otel")]
        {
            let provider = self.0;
            if let Ok(Err(e)) = tokio::task::spawn_blocking(move || provider.shutdown()).await {
                tracing::warn!("trace export did not finish: {e}");
            }
        }
    }
}

#[cfg(feature = "otel")]
mod otel {
    use super::Config;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::registry::LookupSpan;

    /// The OTLP exporter when `otel.endpoint` is set.
    pub(super) fn exporter(
        config: &Config,
        service: &str,
    ) -> Result<Option<SdkTracerProvider>, String> {
        let Some(endpoint) = config
            .get("otel.endpoint")
            .map(|e| e.trim_end_matches('/'))
        else {
            return Ok(None);
        };
        let endpoint = if endpoint.ends_with("/v1/traces") {
            endpoint.to_string()
        } else {
            format!("{endpoint}/v1/traces")
        };
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| e.to_string())?;
        let name = config
            .get("otel.service_name")
            .unwrap_or(service)
            .to_string();
        Ok(Some(
            SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(
                    opentelemetry_sdk::Resource::builder()
                        .with_service_name(name)
                        .build(),
                )
                .build(),
        ))
    }

    pub(super) fn layer<S>(provider: &SdkTracerProvider) -> impl tracing_subscriber::Layer<S>
    where
        S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    {
        tracing_opentelemetry::layer().with_tracer(provider.tracer("zeyn"))
    }
}
