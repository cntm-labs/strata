//! Sentry error and panic capture.
//!
//! Opt-in via `SENTRY_DSN`. When unset, [`init`] returns `None` and Strata
//! behaves identically to the no-Sentry path.
//!
//! Filled in by Tasks 3–4.

use crate::config::AppConfig;

/// Initialize the Sentry client. Returns `None` when `SENTRY_DSN` is unset.
/// The caller must keep the returned guard alive for the duration of the
/// process — its `Drop` flushes pending events with a 2-second timeout.
pub fn init(_config: &AppConfig) -> Option<sentry::ClientInitGuard> {
    None
}
