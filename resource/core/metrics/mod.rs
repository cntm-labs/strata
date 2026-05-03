//! Prometheus metrics for Strata's "Strata monitors Strata" dogfooding.
//!
//! Pattern: install the Prometheus recorder once at startup via `install()`,
//! emit metrics via the `metrics` facade (`counter!()`, `histogram!()`, …)
//! from any module, render exposition text via `render()` for the
//! `/metrics` route.

pub mod middleware;

use std::sync::OnceLock;
use std::time::Duration;

use metrics::{describe_counter, describe_gauge, describe_histogram, gauge, Unit};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use sqlx::PgPool;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Histogram bucket boundaries (seconds) for both
/// `strata_http_request_duration_seconds` and
/// `strata_query_proxy_duration_seconds`. Covers fast in-process DB requests
/// (a few ms) up to slow datasource proxy calls (a few seconds).
const DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Install the global Prometheus recorder and start the DB-pool sampler task.
/// Idempotent: a second call from tests or a hot-reload is a no-op.
pub fn install(pool: &PgPool) {
    if install_recorder_only().is_err() {
        // Already installed (test ordering or hot reload).
        return;
    }

    let pool = pool.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            let total = pool.size();
            let idle = pool.num_idle() as u32;
            let active = total.saturating_sub(idle);
            gauge!("strata_active_connections").set(active as f64);
        }
    });
}

/// Test-only convenience that installs the recorder without spawning the
/// DB-pool sampler. Idempotent. Used by unit tests that have no `PgPool`
/// (e.g. middleware tests, notifier tests).
#[cfg(test)]
pub fn install_for_tests() {
    let _ = install_recorder_only();
}

/// Returns Ok(()) if this call performed the installation, Err(()) if a
/// recorder was already installed (either by an earlier call here or by
/// some other test fixture racing us).
fn install_recorder_only() -> Result<(), ()> {
    if HANDLE.get().is_some() {
        return Err(());
    }
    // `install_recorder` itself fails if a global recorder is already set —
    // can happen under parallel tests where two threads pass the OnceLock
    // check above before either reaches install_recorder. Tolerate it.
    let handle = match PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Suffix("_duration_seconds".to_string()),
            DURATION_BUCKETS,
        )
        .expect("set_buckets_for_metric must succeed")
        .install_recorder()
    {
        Ok(h) => h,
        Err(_) => return Err(()),
    };
    register_descriptors();
    HANDLE.set(handle).map_err(|_| ())?;
    Ok(())
}

/// Render the Prometheus exposition text. Returns an empty string if
/// `install()` hasn't been called (only possible from a test that bypasses
/// `main()`).
pub fn render() -> String {
    HANDLE.get().map(|h| h.render()).unwrap_or_default()
}

fn register_descriptors() {
    describe_counter!(
        "strata_http_requests_total",
        "Total HTTP requests by method, route template, and status"
    );
    describe_histogram!(
        "strata_http_request_duration_seconds",
        Unit::Seconds,
        "HTTP request duration by method and route template"
    );
    describe_gauge!(
        "strata_active_connections",
        "Active (non-idle) connections in the runtime PgPool"
    );
    describe_counter!(
        "strata_query_proxy_total",
        "Total proxied datasource queries by datasource type and outcome"
    );
    describe_histogram!(
        "strata_query_proxy_duration_seconds",
        Unit::Seconds,
        "Proxied datasource query duration by datasource type"
    );
    describe_counter!(
        "strata_alerts_fired_total",
        "Total alert rule evaluations that resulted in firing, by severity"
    );
    describe_counter!(
        "strata_email_sent_total",
        "Total alert email send attempts, by outcome"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn install_is_idempotent(pool: PgPool) {
        install(&pool);
        install(&pool); // must not panic
        assert!(HANDLE.get().is_some());
    }

    #[sqlx::test]
    async fn render_emits_help_and_type_for_observed_metric(pool: PgPool) {
        use metrics::counter;
        install(&pool);
        // The Prometheus exporter only emits HELP/TYPE lines for metrics that
        // have at least one observed sample — describe_*! alone is not enough.
        // Touch one counter to force it into the rendered output.
        counter!("strata_http_requests_total", "method" => "GET", "path" => "/probe", "status" => "200")
            .increment(0);
        let body = render();
        assert!(
            body.contains("# HELP strata_http_requests_total"),
            "expected HELP line; got:\n{body}"
        );
        assert!(
            body.contains("# TYPE strata_http_requests_total counter"),
            "expected TYPE line; got:\n{body}"
        );
    }
}
