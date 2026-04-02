mod api;
mod config;
mod datasource;
mod error;

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

    let app = Router::new()
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
        .layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Strata listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
