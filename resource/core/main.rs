pub mod api;
pub mod config;
pub mod datasource;
pub mod error;

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
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .nest("/api/v1/datasources", api::datasources::datasource_routes())
        .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
        .nest("/api/v1", api::panels::panel_routes_nested())
        .nest("/api/v1/explore", api::explore::explore_routes())
        .nest("/api/v1/alerts", api::alerts::alert_routes())
        .nest("/api/v1/templates", api::templates::template_routes())
        .with_state(state)
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

    let state = AppState {
        db,
        config: config.clone(),
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
        AppState {
            db,
            config: AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
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
}
