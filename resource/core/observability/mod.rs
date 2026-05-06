//! External observability integrations.
//!
//! Today: Sentry error/panic capture. Future: OpenTelemetry export, Datadog
//! traces. Each integration is a sibling module so the surface area stays
//! contained.

pub mod sentry;
