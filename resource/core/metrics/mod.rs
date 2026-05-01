//! Prometheus metrics for Strata's "Strata monitors Strata" dogfooding.
//!
//! Pattern: install the Prometheus recorder once at startup via `install()`,
//! emit metrics via the `metrics` facade (`counter!()`, `histogram!()`, …)
//! from any module, render exposition text via `render()` for the
//! `/metrics` route.

pub mod middleware;

use std::sync::OnceLock;

use metrics_exporter_prometheus::PrometheusHandle;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Render the Prometheus exposition text. Returns an empty string if
/// `install()` hasn't been called (only possible from a test that bypasses
/// `main()`).
pub fn render() -> String {
    HANDLE.get().map(|h| h.render()).unwrap_or_default()
}
