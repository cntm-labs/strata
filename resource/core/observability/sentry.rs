//! Sentry error and panic capture.
//!
//! Opt-in via `SENTRY_DSN`. When unset, [`init`] returns `None` and Strata
//! behaves identically to the no-Sentry path.

use std::sync::Arc;

use crate::config::AppConfig;

/// Initialize the Sentry client. Returns `None` when `SENTRY_DSN` is unset.
/// Caller must hold the returned guard for the process lifetime — its
/// `Drop` flushes pending events with a 2-second timeout.
pub fn init(config: &AppConfig) -> Option<sentry::ClientInitGuard> {
    let dsn = config.sentry_dsn.as_deref()?;
    let environment = config
        .strata_env
        .clone()
        .unwrap_or_else(|| "development".to_string());

    let guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(environment.into()),
            sample_rate: 1.0,
            traces_sample_rate: 0.0,
            attach_stacktrace: true,
            send_default_pii: false,
            before_send: Some(Arc::new(before_send)),
            ..Default::default()
        },
    ));
    Some(guard)
}

/// Tracing subscriber layer that promotes ERROR-level events to Sentry
/// captures. WARN and below are ignored to keep volume bounded.
///
/// When the Sentry client is uninitialised (no DSN), this layer's writes
/// are no-ops — `sentry::capture_event` checks for an active hub.
pub fn tracing_layer<S>() -> impl tracing_subscriber::Layer<S> + Send + Sync
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    sentry::integrations::tracing::layer().event_filter(|md| {
        if md.level() == &tracing::Level::ERROR {
            sentry::integrations::tracing::EventFilter::Event
        } else {
            sentry::integrations::tracing::EventFilter::Ignore
        }
    })
}

/// Returns true for headers Sentry should never include in events.
fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "x-api-key" | "x-strata-token"
    )
}

/// Strip the value from any query-string pair whose key is in the sensitive
/// list. Preserves order and keys; replaces values with `[redacted]`.
fn scrub_query(qs: &str) -> String {
    qs.split('&')
        .map(|pair| {
            let lower = pair.to_ascii_lowercase();
            if lower.starts_with("password=")
                || lower.starts_with("token=")
                || lower.starts_with("key=")
                || lower.starts_with("secret=")
            {
                pair.split_once('=')
                    .map(|(k, _)| format!("{k}=[redacted]"))
                    .unwrap_or_else(|| pair.to_string())
            } else {
                pair.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Sentry `before_send` hook: scrub PII from outgoing events.
fn before_send(
    mut event: sentry::protocol::Event<'static>,
) -> Option<sentry::protocol::Event<'static>> {
    if let Some(req) = event.request.as_mut() {
        req.headers.retain(|name, _| !is_sensitive_header(name));
        if let Some(query) = req.query_string.take() {
            req.query_string = Some(scrub_query(&query));
        }
        req.data = None;
        req.cookies = None;
    }
    event.user = None;
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dsn: Option<&str>, env: Option<&str>) -> AppConfig {
        AppConfig {
            database_url: String::new(),
            database_url_admin: None,
            strata_app_password: None,
            sentry_dsn: dsn.map(String::from),
            strata_env: env.map(String::from),
            host: "127.0.0.1".into(),
            port: 3000,
            nucleus_api_key: None,
            nucleus_jwks_cache_ttl_secs: None,
            nucleus_base_url: None,
            resend_api_key: None,
            alert_from_email: "test@test.com".into(),
        }
    }

    #[test]
    fn init_returns_none_when_dsn_unset() {
        let cfg = test_config(None, None);
        let guard = init(&cfg);
        assert!(guard.is_none());
    }

    #[test]
    fn init_returns_some_when_dsn_set() {
        // Syntactically valid DSN that points at a non-resolvable host. The
        // client is initialised, but no events are emitted in this test, so
        // `Drop` flushes nothing and finishes immediately.
        let cfg = test_config(Some("https://public@o0.example.invalid/1"), Some("test"));
        let guard = init(&cfg);
        assert!(guard.is_some());
    }

    #[test]
    fn is_sensitive_header_is_case_insensitive() {
        assert!(is_sensitive_header("Authorization"));
        assert!(is_sensitive_header("authorization"));
        assert!(is_sensitive_header("Cookie"));
        assert!(is_sensitive_header("X-API-Key"));
        assert!(!is_sensitive_header("Content-Type"));
        assert!(!is_sensitive_header("Accept"));
    }

    #[test]
    fn scrub_query_redacts_sensitive_values() {
        assert_eq!(
            scrub_query("token=abc123&keep=ok&password=hunter2"),
            "token=[redacted]&keep=ok&password=[redacted]"
        );
    }

    #[test]
    fn scrub_query_preserves_non_sensitive_pairs() {
        assert_eq!(
            scrub_query("query=up&start=1000&end=2000"),
            "query=up&start=1000&end=2000"
        );
    }

    #[test]
    fn scrub_query_handles_secret_and_key_keys() {
        assert_eq!(
            scrub_query("api_key=foo&secret=bar"),
            "api_key=foo&secret=[redacted]"
        );
    }

    #[test]
    fn scrub_query_handles_empty_value() {
        assert_eq!(scrub_query("token="), "token=[redacted]");
    }

    #[test]
    fn before_send_strips_sensitive_headers() {
        use sentry::protocol::{Event, Request};
        let mut headers = std::collections::BTreeMap::new();
        headers.insert("Authorization".to_string(), "Bearer xyz".to_string());
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let event = Event {
            request: Some(Request {
                headers,
                ..Default::default()
            }),
            ..Default::default()
        };
        let scrubbed = before_send(event).unwrap();
        let req = scrubbed.request.as_ref().unwrap();
        assert!(!req.headers.contains_key("Authorization"));
        assert!(req.headers.contains_key("Content-Type"));
    }

    #[test]
    fn before_send_scrubs_query_string() {
        use sentry::protocol::{Event, Request};
        let event = Event {
            request: Some(Request {
                query_string: Some("token=secret&keep=ok".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let scrubbed = before_send(event).unwrap();
        let qs = scrubbed.request.unwrap().query_string.unwrap();
        assert!(qs.contains("token=[redacted]"));
        assert!(qs.contains("keep=ok"));
    }

    #[test]
    fn tracing_layer_constructs() {
        // Smoke test that the function compiles and returns a usable layer.
        // Cannot exercise event routing without a live Sentry hub bound to
        // a real subscriber; the integration test in main.rs::tests covers
        // the end-to-end path indirectly.
        let _layer = tracing_layer::<tracing_subscriber::Registry>();
    }

    #[test]
    fn before_send_drops_user_and_body() {
        use sentry::protocol::{Event, Request, User};
        let event = Event {
            request: Some(Request {
                data: Some(r#"{"secret":"value"}"#.to_string()),
                ..Default::default()
            }),
            user: Some(User {
                ip_address: Some(sentry::protocol::IpAddress::Auto),
                ..Default::default()
            }),
            ..Default::default()
        };
        let scrubbed = before_send(event).unwrap();
        assert!(scrubbed.user.is_none());
        assert!(scrubbed.request.unwrap().data.is_none());
    }
}
