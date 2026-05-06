//! Axum middleware that records request-level Prometheus metrics.

use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use metrics::{counter, histogram};

/// Records `strata_http_requests_total` and
/// `strata_http_request_duration_seconds` for every request.
///
/// Path label uses the matched route template (e.g.
/// `/api/v1/dashboards/{slug}`) to bound cardinality. Requests that don't
/// hit a route (404 fallback) are labelled `<unmatched>`.
pub async fn record_http(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "<unmatched>".to_string());

    // Tag the per-request Sentry scope (created by SentryHttpLayer earlier
    // in the chain) with tenant_id so any error captured downstream carries
    // it. No-op when Sentry is uninitialised — set_tag writes into the
    // current hub which is a default no-op hub without a client.
    if let Some(tenant) = req.extensions().get::<crate::db::TenantId>() {
        let tid = tenant.0.to_string();
        sentry::configure_scope(|scope| {
            scope.set_tag("tenant_id", tid);
        });
    }

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    counter!(
        "strata_http_requests_total",
        "method" => method.to_string(),
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);

    histogram!(
        "strata_http_request_duration_seconds",
        "method" => method.to_string(),
        "path" => path,
    )
    .record(elapsed);

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn test_app() -> Router {
        Router::new()
            .route("/hello/{name}", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(record_http))
    }

    #[tokio::test]
    async fn records_request_with_template_path() {
        crate::metrics::install_for_tests();
        let app = test_app();

        let resp = app
            .oneshot(
                HttpRequest::get("/hello/alice")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let rendered = crate::metrics::render();
        assert!(
            rendered.contains(r#"path="/hello/{name}""#),
            "path label was not the route template; got:\n{rendered}"
        );
        assert!(
            rendered.contains(r#"method="GET""#),
            "method label missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains(r#"status="200""#),
            "status label missing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("strata_http_request_duration_seconds_bucket"),
            "histogram buckets not present; got:\n{rendered}"
        );
    }

    #[tokio::test]
    async fn unmatched_route_uses_fallback_label() {
        crate::metrics::install_for_tests();
        let app = test_app();

        let resp = app
            .oneshot(
                HttpRequest::get("/this-route-does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let rendered = crate::metrics::render();
        assert!(
            rendered.contains(r#"path="<unmatched>""#),
            "404 path was not labelled <unmatched>; got:\n{rendered}"
        );
    }
}
