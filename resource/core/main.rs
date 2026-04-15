pub mod api;
pub mod auth;
pub mod config;
pub mod datasource;
pub mod error;
pub mod notifier;

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use config::AppConfig;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
    pub notifier: Arc<notifier::Notifier>,
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub fn build_router(state: AppState) -> Router {
    let public = Router::new().route("/api/v1/health", get(health));

    let protected = Router::new()
        .nest("/api/v1/datasources", api::datasources::datasource_routes())
        .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
        .nest("/api/v1", api::panels::panel_routes_nested())
        .nest("/api/v1/explore", api::explore::explore_routes())
        .nest("/api/v1/alerts", api::alerts::alert_routes())
        .nest("/api/v1/templates", api::templates::template_routes());

    let protected = if state.config.nucleus_secret_key.is_some() {
        protected.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
    } else {
        tracing::warn!("NUCLEUS_SECRET_KEY not set — running without authentication");
        protected
    };

    public
        .merge(protected.with_state(state))
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "strata=debug,tower_http=debug".into()),
        )
        .init();

    let config = AppConfig::from_env();

    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Failed to run database migrations");

    let notifier = Arc::new(notifier::Notifier::new(
        config.resend_api_key.as_deref(),
        &config.alert_from_email,
    ));

    let state = AppState {
        db,
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

    fn test_state(db: sqlx::PgPool) -> AppState {
        test_state_with_auth(db, None)
    }

    fn test_state_with_auth(db: sqlx::PgPool, secret_key: Option<String>) -> AppState {
        AppState {
            db,
            config: AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_secret_key: secret_key,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: Arc::new(notifier::Notifier::new(None, "test@test.com")),
        }
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
