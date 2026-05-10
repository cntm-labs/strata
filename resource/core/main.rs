pub mod api;
pub mod config;
pub mod datasource;
pub mod db;
pub mod error;
pub mod metrics;
pub mod middleware;
pub mod notifier;
pub mod observability;

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use config::AppConfig;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: AppConfig,
    pub notifier: Arc<notifier::Notifier>,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn metrics_handler() -> ([(&'static str, &'static str); 1], String) {
    (
        [("content-type", "text/plain; version=0.0.4")],
        metrics::render(),
    )
}

pub fn build_router(state: AppState) -> Router {
    let public = Router::new()
        .route("/api/v1/health", get(health))
        .route("/metrics", get(metrics_handler));

    let protected = Router::new()
        .nest("/api/v1/datasources", api::datasources::datasource_routes())
        .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
        .nest("/api/v1", api::panels::panel_routes_nested())
        .nest("/api/v1/explore", api::explore::explore_routes())
        .nest("/api/v1/alerts", api::alerts::alert_routes())
        .nest("/api/v1/templates", api::templates::template_routes())
        .layer(axum::middleware::from_fn(middleware::tenant::inject_tenant));

    let protected = if let Some(api_key) = state.config.nucleus_api_key.as_deref() {
        let client = Arc::new(cntm_nucleus::NucleusClient::new(
            cntm_nucleus::NucleusConfig {
                secret_key: api_key.to_string(),
                base_url: state.config.nucleus_base_url.clone(),
                jwks_cache_ttl_secs: state.config.nucleus_jwks_cache_ttl_secs,
            },
        ));
        protected
            .layer(axum::middleware::from_fn(middleware::auth::require_auth))
            .layer(axum::Extension(client))
    } else {
        tracing::warn!("NUCLEUS_API_KEY not set — running without authentication");
        protected
    };

    public
        .merge(protected.with_state(state))
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(axum::middleware::from_fn(metrics::middleware::record_http))
        .layer(sentry::integrations::tower::SentryHttpLayer::with_transaction())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    let config = AppConfig::from_env();

    // Sentry guard MUST be the first binding so it drops last (Rust drop
    // order is reverse of construction). That guarantees panics during
    // early startup get flushed before the process exits.
    let _sentry_guard = observability::sentry::init(&config);

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "strata=debug,tower_http=debug".into()),
        )
        .with(observability::sentry::tracing_layer())
        .init();

    let pool = db::bootstrap_db(&config)
        .await
        .expect("Failed to bootstrap database");

    metrics::install(&pool);

    let notifier = Arc::new(notifier::Notifier::new(
        config.resend_api_key.as_deref(),
        &config.alert_from_email,
    ));

    let state = AppState {
        pool,
        config: config.clone(),
        notifier,
    };

    let app = build_router(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Strata listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state(pool: sqlx::PgPool) -> AppState {
        test_state_with_auth(pool, None)
    }

    fn test_state_with_auth(pool: sqlx::PgPool, secret_key: Option<String>) -> AppState {
        AppState {
            pool,
            config: AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: secret_key,
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: Arc::new(notifier::Notifier::new(None, "test@test.com")),
        }
    }

    #[sqlx::test]
    async fn metrics_endpoint_returns_prometheus_format(pool: sqlx::PgPool) {
        crate::metrics::install(&pool.clone());
        let app = build_router(test_state(pool));

        // Drive a request through the recording middleware first so the
        // exporter has at least one observed sample to emit HELP/TYPE for.
        let _ = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await;

        let resp = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let ct = resp
            .headers()
            .get("content-type")
            .expect("content-type header")
            .to_str()
            .unwrap()
            .to_string();
        assert!(ct.starts_with("text/plain"), "content-type was {ct}");

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            body.contains("# HELP") || body.contains("# TYPE"),
            "body did not contain Prometheus exposition headers; got:\n{body}"
        );
    }

    #[sqlx::test]
    async fn unknown_route_returns_fallback(pool: sqlx::PgPool) {
        let app = build_router(test_state(pool));
        let resp = app
            .oneshot(
                Request::get("/api/v1/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Fallback serves static files; non-existent returns 404 or static fallback
        assert!(resp.status() == StatusCode::OK || resp.status() == StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn health_returns_ok(pool: sqlx::PgPool) {
        let app = build_router(test_state(pool));
        let resp = app
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, serde_json::json!({"status": "ok"}));
    }

    #[sqlx::test]
    async fn protected_route_requires_auth_when_configured(pool: sqlx::PgPool) {
        let state = test_state_with_auth(pool, Some("sk_test_fake".into()));
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn protected_route_rejects_invalid_token(pool: sqlx::PgPool) {
        let state = test_state_with_auth(pool, Some("sk_test_fake".into()));
        let app = build_router(state);
        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .header("Authorization", "Bearer invalid.jwt.token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn health_accessible_even_with_auth_configured(pool: sqlx::PgPool) {
        let state = test_state_with_auth(pool, Some("sk_test_fake".into()));
        let app = build_router(state);
        let resp = app
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
