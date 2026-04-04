# Test Coverage Implementation Plan (0% → 100%)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add comprehensive tests to the entire Strata codebase, achieving 100% test coverage for both Rust backend and Vue frontend.

**Architecture:** Hybrid approach — integration tests with real PostgreSQL (via `sqlx::test`) for API handlers, unit tests with `wiremock` for HTTP clients, unit tests for pure logic modules. Frontend uses Vitest + Vue Test Utils with mocked `fetch`. All tests use `#[cfg(test)] mod tests` inline pattern for Rust. Use `bun` instead of `npm`.

**Tech Stack:** Rust: sqlx::test, wiremock, tower::ServiceExt, http-body-util. Frontend: vitest, @vue/test-utils, jsdom, @vitest/coverage-v8.

---

## PART 1: BACKEND (RUST)

### Task 1: Add Rust dev-dependencies

**Files:**
- Modify: `resource/Cargo.toml`

**Step 1: Add dev-dependencies**

Add to `resource/Cargo.toml`:
```toml
[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
wiremock = "0.6"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "migrate"] }
```

Note: `sqlx` in dev-deps ensures `sqlx::test` macro is available. `tower` provides `ServiceExt::oneshot`. `http-body-util` provides `BodyExt::collect` for reading response bodies.

**Step 2: Verify it compiles**

Run: `cd resource && cargo check`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add resource/Cargo.toml
git commit -m "test: add Rust dev-dependencies for testing"
```

---

### Task 2: Extract `build_router` for testability + make modules public

**Files:**
- Modify: `resource/core/main.rs`

The `main()` currently builds the Router inline. Extract a `pub fn build_router(state: AppState) -> Router` so tests can construct the full app. Also make modules `pub` so integration tests within submodules can access sibling modules.

**Step 1: Refactor main.rs**

```rust
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
```

**Step 2: Verify it compiles**

Run: `cd resource && cargo check`
Expected: compiles without errors

**Step 3: Commit**

```bash
git add resource/core/main.rs
git commit -m "refactor: extract build_router and make modules pub for testability"
```

---

### Task 3: Unit tests for `error/mod.rs`

**Files:**
- Modify: `resource/core/error/mod.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/error/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;

    async fn error_to_parts(error: AppError) -> (StatusCode, ErrorResponse) {
        let response = error.into_response();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let err_body: ErrorResponse = serde_json::from_slice(&body).unwrap();
        (status, err_body)
    }

    #[derive(serde::Deserialize)]
    struct ErrorResponse {
        code: u16,
        status: String,
        message: String,
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let (status, body) = error_to_parts(AppError::NotFound("missing".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.code, 404);
        assert_eq!(body.status, "Not Found");
        assert_eq!(body.message, "missing");
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let (status, body) = error_to_parts(AppError::BadRequest("invalid".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.code, 400);
        assert_eq!(body.status, "Bad Request");
        assert_eq!(body.message, "invalid");
    }

    #[tokio::test]
    async fn database_error_returns_500() {
        let db_err = sqlx::Error::RowNotFound;
        let (status, body) = error_to_parts(AppError::Database(db_err)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, 500);
    }

    #[tokio::test]
    async fn internal_error_returns_500() {
        let (status, body) = error_to_parts(AppError::Internal("oops".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.code, 500);
        assert_eq!(body.message, "oops");
    }

    #[tokio::test]
    async fn request_error_returns_502() {
        let req_err = reqwest::get("http://127.0.0.1:1/nonexistent")
            .await
            .unwrap_err();
        let (status, body) = error_to_parts(AppError::Request(req_err)).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body.code, 502);
    }

    #[test]
    fn from_serde_json_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let app_err = AppError::from(json_err);
        assert!(matches!(app_err, AppError::Internal(_)));
    }

    #[test]
    fn display_messages() {
        assert_eq!(AppError::NotFound("x".into()).to_string(), "Not found: x");
        assert_eq!(AppError::BadRequest("y".into()).to_string(), "Bad request: y");
        assert_eq!(AppError::Internal("z".into()).to_string(), "Internal error: z");
    }

    #[test]
    fn app_result_type_alias_works() {
        let ok: AppResult<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);
        let err: AppResult<i32> = Err(AppError::NotFound("nope".into()));
        assert!(err.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test error::tests -- --nocapture`
Expected: all 8 tests pass

**Step 3: Commit**

```bash
git add resource/core/error/mod.rs
git commit -m "test: add unit tests for AppError (all variants + conversions)"
```

---

### Task 4: Unit tests for `config/mod.rs`

**Files:**
- Modify: `resource/core/config/mod.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/config/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_no_env() {
        // Clear relevant env vars to test defaults
        env::remove_var("DATABASE_URL");
        env::remove_var("HOST");
        env::remove_var("PORT");
        let config = AppConfig::from_env();
        assert_eq!(config.database_url, "postgres://strata:secret@localhost:5432/strata");
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
    }

    #[test]
    fn reads_custom_env() {
        env::set_var("DATABASE_URL", "postgres://test:test@db:5432/testdb");
        env::set_var("HOST", "127.0.0.1");
        env::set_var("PORT", "8080");
        let config = AppConfig::from_env();
        assert_eq!(config.database_url, "postgres://test:test@db:5432/testdb");
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        // Cleanup
        env::remove_var("DATABASE_URL");
        env::remove_var("HOST");
        env::remove_var("PORT");
    }

    #[test]
    fn invalid_port_falls_back_to_default() {
        env::set_var("PORT", "not_a_number");
        let config = AppConfig::from_env();
        assert_eq!(config.port, 3000);
        env::remove_var("PORT");
    }

    #[test]
    fn clone_works() {
        let config = AppConfig {
            database_url: "url".into(),
            host: "host".into(),
            port: 1234,
        };
        let cloned = config.clone();
        assert_eq!(cloned.database_url, "url");
        assert_eq!(cloned.host, "host");
        assert_eq!(cloned.port, 1234);
    }
}
```

Note: these tests must run with `--test-threads=1` because they mutate shared env vars.

**Step 2: Run tests**

Run: `cd resource && cargo test config::tests -- --test-threads=1 --nocapture`
Expected: all 4 tests pass

**Step 3: Commit**

```bash
git add resource/core/config/mod.rs
git commit -m "test: add unit tests for AppConfig (defaults, custom env, invalid port)"
```

---

### Task 5: Unit tests for `datasource/prometheus.rs` (wiremock)

**Files:**
- Modify: `resource/core/datasource/prometheus.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/datasource/prometheus.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn new_trims_trailing_slash() {
        let client = PrometheusClient::new("http://localhost:9090/");
        assert_eq!(client.base_url, "http://localhost:9090");
    }

    #[tokio::test]
    async fn query_instant_minimal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "vector", "result": []}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client.query("up", None, None).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_with_time_and_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .and(query_param("time", "1234"))
            .and(query_param("timeout", "30s"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client.query("up", Some("1234"), Some("30s")).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_sends_all_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .and(query_param("query", "rate(http_total[5m])"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .and(query_param("step", "15"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client.query_range("rate(http_total[5m])", "1000", "2000", "15").await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_connection_error() {
        let client = PrometheusClient::new("http://127.0.0.1:1");
        let result = client.query("up", None, None).await;
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test datasource::prometheus::tests -- --nocapture`
Expected: all 5 tests pass

**Step 3: Commit**

```bash
git add resource/core/datasource/prometheus.rs
git commit -m "test: add unit tests for PrometheusClient (wiremock)"
```

---

### Task 6: Unit tests for `datasource/loki.rs` (wiremock)

**Files:**
- Modify: `resource/core/datasource/loki.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/datasource/loki.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn new_trims_trailing_slash() {
        let client = LokiClient::new("http://localhost:3100/");
        assert_eq!(client.base_url, "http://localhost:3100");
    }

    #[tokio::test]
    async fn query_without_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .and(query_param("query", "{job=\"app\"}"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "streams", "result": []}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query("{job=\"app\"}", None).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_with_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query("{job=\"app\"}", Some(50)).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_all_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query_range("{job=\"app\"}", "1000", "2000", Some(100)).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_without_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query_range("{job=\"app\"}", "1000", "2000", None).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_connection_error() {
        let client = LokiClient::new("http://127.0.0.1:1");
        let result = client.query("{job=\"app\"}", None).await;
        assert!(result.is_err());
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test datasource::loki::tests -- --nocapture`
Expected: all 6 tests pass

**Step 3: Commit**

```bash
git add resource/core/datasource/loki.rs
git commit -m "test: add unit tests for LokiClient (wiremock)"
```

---

### Task 7: Integration tests for health endpoint

**Files:**
- Modify: `resource/core/main.rs`

**Step 1: Write test at bottom of main.rs**

```rust
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
```

**Step 2: Run test (requires Docker PostgreSQL)**

Run: `cd resource && cargo test tests::health_returns_ok -- --nocapture`
Expected: PASS (sqlx::test auto-creates temp DB)

**Step 3: Commit**

```bash
git add resource/core/main.rs
git commit -m "test: add integration test for health endpoint"
```

---

### Task 8: Integration tests for dashboards API

**Files:**
- Modify: `resource/core/api/dashboards.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/api/dashboards.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        dashboard_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app.oneshot(Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Dashboard> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/", serde_json::json!({
                "title": "Test Dashboard",
                "slug": "test-dash"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Dashboard = body_json(resp).await;
        assert_eq!(created.title, "Test Dashboard");
        assert_eq!(created.slug, "test-dash");
        assert_eq!(created.time_range, Some("1h".into()));
        assert_eq!(created.refresh_interval, Some(0));

        // GET by slug
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/test-dash").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: Dashboard = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/nonexistent").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_dashboard(pool: sqlx::PgPool) {
        // Create first
        let app = test_app(pool.clone());
        app.oneshot(json_request("POST", "/", serde_json::json!({
            "title": "Original", "slug": "update-test"
        }))).await.unwrap();

        // Update
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("PUT", "/update-test", serde_json::json!({
                "title": "Updated"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: Dashboard = body_json(resp).await;
        assert_eq!(updated.title, "Updated");
    }

    #[sqlx::test]
    async fn update_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("PUT", "/nonexistent", serde_json::json!({"title": "x"})))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_dashboard(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        app.oneshot(json_request("POST", "/", serde_json::json!({
            "title": "To Delete", "slug": "delete-me"
        }))).await.unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::delete("/delete-me").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify deleted
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/delete-me").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn toggle_star(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        app.oneshot(json_request("POST", "/", serde_json::json!({
            "title": "Star Test", "slug": "star-test"
        }))).await.unwrap();

        // Star
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::post("/star-test/star").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let starred: Dashboard = body_json(resp).await;
        assert_eq!(starred.is_starred, Some(true));

        // Unstar
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post("/star-test/star").body(Body::empty()).unwrap())
            .await.unwrap();
        let unstarred: Dashboard = body_json(resp).await;
        assert_eq!(unstarred.is_starred, Some(false));
    }

    #[sqlx::test]
    async fn toggle_star_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post("/nonexistent/star").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn create_with_all_optional_fields(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/", serde_json::json!({
                "title": "Full",
                "slug": "full-dash",
                "description": "A full dashboard",
                "time_range": "24h",
                "refresh_interval": 30,
                "variables": [{"name": "host", "value": "localhost"}]
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Dashboard = body_json(resp).await;
        assert_eq!(created.description, Some("A full dashboard".into()));
        assert_eq!(created.time_range, Some("24h".into()));
        assert_eq!(created.refresh_interval, Some(30));
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test api::dashboards::tests -- --nocapture`
Expected: all 9 tests pass

**Step 3: Commit**

```bash
git add resource/core/api/dashboards.rs
git commit -m "test: add integration tests for dashboards CRUD API"
```

---

### Task 9: Integration tests for datasources API

**Files:**
- Modify: `resource/core/api/datasources.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/api/datasources.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        // Include query routes since datasource_routes nests them
        datasource_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method_str: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method_str)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn create_ds(pool: &sqlx::PgPool, name: &str, ds_type: &str, url: &str) -> Datasource {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/", serde_json::json!({
                "name": name, "type": ds_type, "url": url
            })))
            .await.unwrap();
        body_json(resp).await
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app.oneshot(Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Datasource> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "Prom", "prometheus", "http://prom:9090").await;
        assert_eq!(created.name, "Prom");
        assert_eq!(created.ds_type, "prometheus");

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: Datasource = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(Request::get(&format!("/{}", fake_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_datasource(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "Old", "loki", "http://loki:3100").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("PUT", &format!("/{}", created.id), serde_json::json!({
                "name": "New Name"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: Datasource = body_json(resp).await;
        assert_eq!(updated.name, "New Name");
    }

    #[sqlx::test]
    async fn update_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(json_request("PUT", &format!("/{}", fake_id), serde_json::json!({"name": "x"})))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_datasource(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "ToDelete", "prometheus", "http://x:9090").await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::delete(&format!("/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn create_with_default_flag(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/", serde_json::json!({
                "name": "Default", "type": "prometheus", "url": "http://prom:9090", "is_default": true
            })))
            .await.unwrap();
        let created: Datasource = body_json(resp).await;
        assert_eq!(created.is_default, Some(true));
    }

    #[sqlx::test]
    async fn test_connection_prometheus(pool: sqlx::PgPool) {
        let mock_prom = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/-/healthy"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_prom)
            .await;

        let ds = create_ds(&pool, "Prom", "prometheus", &mock_prom.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post(&format!("/{}/test", ds.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], true);
    }

    #[sqlx::test]
    async fn test_connection_loki(pool: sqlx::PgPool) {
        let mock_loki = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ready"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_loki)
            .await;

        let ds = create_ds(&pool, "Loki", "loki", &mock_loki.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post(&format!("/{}/test", ds.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], true);
    }

    #[sqlx::test]
    async fn test_connection_unknown_type(pool: sqlx::PgPool) {
        let ds = create_ds(&pool, "Unknown", "redis", "http://redis:6379").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post(&format!("/{}/test", ds.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], false);
    }

    #[sqlx::test]
    async fn test_connection_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(Request::post(&format!("/{}/test", fake_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn test_connection_prometheus_unhealthy(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/-/healthy"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock)
            .await;

        let ds = create_ds(&pool, "Prom", "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::post(&format!("/{}/test", ds.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], false);
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test api::datasources::tests -- --nocapture`
Expected: all 12 tests pass

**Step 3: Commit**

```bash
git add resource/core/api/datasources.rs
git commit -m "test: add integration tests for datasources CRUD + test_connection"
```

---

### Task 10: Integration tests for panels API

**Files:**
- Modify: `resource/core/api/panels.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/api/panels.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        panel_routes_nested().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method_str: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method_str)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_dashboard(pool: &sqlx::PgPool) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO dashboards (title, slug) VALUES ('Test', 'test-dash') RETURNING id"
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/dashboards/test-dash/panels").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Panel> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_list(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/dashboards/test-dash/panels", serde_json::json!({
                "title": "CPU Panel",
                "type": "timeseries",
                "query": "rate(cpu[5m])",
                "position": {"x": 0, "y": 0, "w": 6, "h": 3}
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Panel = body_json(resp).await;
        assert_eq!(created.title, "CPU Panel");
        assert_eq!(created.panel_type, "timeseries");

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/dashboards/test-dash/panels").body(Body::empty()).unwrap())
            .await.unwrap();
        let items: Vec<Panel> = body_json(resp).await;
        assert_eq!(items.len(), 1);
    }

    #[sqlx::test]
    async fn create_dashboard_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/dashboards/nonexistent/panels", serde_json::json!({
                "title": "X", "type": "stat", "query": "up", "position": {"x":0,"y":0,"w":3,"h":2}
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_panel(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/dashboards/test-dash/panels", serde_json::json!({
                "title": "Old", "type": "stat", "query": "up",
                "position": {"x": 0, "y": 0, "w": 3, "h": 2}
            })))
            .await.unwrap();
        let created: Panel = body_json(resp).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("PUT", &format!("/panels/{}", created.id), serde_json::json!({
                "title": "New Title", "query": "down"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: Panel = body_json(resp).await;
        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.query, "down");
    }

    #[sqlx::test]
    async fn update_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(json_request("PUT", &format!("/panels/{}", fake_id), serde_json::json!({"title": "x"})))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_panel(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/dashboards/test-dash/panels", serde_json::json!({
                "title": "ToDelete", "type": "stat", "query": "up",
                "position": {"x": 0, "y": 0, "w": 3, "h": 2}
            })))
            .await.unwrap();
        let created: Panel = body_json(resp).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::delete(&format!("/panels/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify empty
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/dashboards/test-dash/panels").body(Body::empty()).unwrap())
            .await.unwrap();
        let items: Vec<Panel> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_with_optional_config(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/dashboards/test-dash/panels", serde_json::json!({
                "title": "With Config",
                "type": "gauge",
                "query": "mem_usage",
                "config": {"min": 0, "max": 100},
                "position": {"x": 0, "y": 0, "w": 3, "h": 3}
            })))
            .await.unwrap();
        let created: Panel = body_json(resp).await;
        assert_eq!(created.config["min"], 0);
        assert_eq!(created.config["max"], 100);
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test api::panels::tests -- --nocapture`
Expected: all 7 tests pass

**Step 3: Commit**

```bash
git add resource/core/api/panels.rs
git commit -m "test: add integration tests for panels API"
```

---

### Task 11: Integration tests for alerts API

**Files:**
- Modify: `resource/core/api/alerts.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/api/alerts.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        alert_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method_str: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method_str)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_datasource(pool: &sqlx::PgPool) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ('Test', 'prometheus', 'http://prom:9090') RETURNING id"
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_rule(pool: &sqlx::PgPool, ds_id: Uuid, name: &str) -> AlertRule {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/rules", serde_json::json!({
                "name": name,
                "datasource_id": ds_id,
                "query": "up == 0",
                "condition": "gt",
                "threshold": 0.5,
                "notification_channels": ["sms"],
                "notification_recipients": ["+66123456789"]
            })))
            .await.unwrap();
        body_json(resp).await
    }

    #[sqlx::test]
    async fn list_rules_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/rules").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertRule> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule(&pool, ds_id, "CPU Alert").await;
        assert_eq!(created.name, "CPU Alert");
        assert_eq!(created.condition, "gt");
        assert!(created.is_active);
        assert_eq!(created.severity, "warning"); // default
        assert_eq!(created.duration_secs, 60); // default

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/rules/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: AlertRule = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_rule_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(Request::get(&format!("/rules/{}", fake_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule(&pool, ds_id, "Original").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("PUT", &format!("/rules/{}", created.id), serde_json::json!({
                "name": "Updated",
                "severity": "critical",
                "is_active": false
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: AlertRule = body_json(resp).await;
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.severity, "critical");
        assert!(!updated.is_active);
    }

    #[sqlx::test]
    async fn update_rule_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(json_request("PUT", &format!("/rules/{}", fake_id), serde_json::json!({"name": "x"})))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule(&pool, ds_id, "ToDelete").await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::delete(&format!("/rules/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/rules/{}", created.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn list_events_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/events").body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertEvent> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn list_events_with_filter(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let rule = create_rule(&pool, ds_id, "Rule1").await;

        // Insert an event directly
        sqlx::query("INSERT INTO alert_events (rule_id, state, value, message) VALUES ($1, $2, $3, $4)")
            .bind(rule.id)
            .bind("firing")
            .bind(1.5_f64)
            .bind("CPU high")
            .execute(&pool)
            .await
            .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::get(&format!("/events?rule_id={}&limit=10", rule.id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertEvent> = body_json(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].state, "firing");

        // Without filter — should also return 1
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/events").body(Body::empty()).unwrap())
            .await.unwrap();
        let all_items: Vec<AlertEvent> = body_json(resp).await;
        assert_eq!(all_items.len(), 1);
    }

    #[sqlx::test]
    async fn create_rule_with_custom_severity_and_duration(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/rules", serde_json::json!({
                "name": "Custom",
                "datasource_id": ds_id,
                "query": "up",
                "condition": "lt",
                "threshold": 1.0,
                "duration_secs": 300,
                "severity": "critical",
                "notification_channels": ["email"],
                "notification_recipients": ["admin@test.com"]
            })))
            .await.unwrap();
        let created: AlertRule = body_json(resp).await;
        assert_eq!(created.duration_secs, 300);
        assert_eq!(created.severity, "critical");
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test api::alerts::tests -- --nocapture`
Expected: all 10 tests pass

**Step 3: Commit**

```bash
git add resource/core/api/alerts.rs
git commit -m "test: add integration tests for alerts API (rules + events)"
```

---

### Task 12: Integration tests for templates API

**Files:**
- Modify: `resource/core/api/templates.rs`

**Step 1: Write tests**

Add at the bottom of `resource/core/api/templates.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        template_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method_str: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method_str)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_template(pool: &sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO dashboard_templates (slug, name, category, dashboard_json, is_active)
             VALUES ('test-tmpl', 'Test Template', 'test',
             '{\"panels\":[{\"title\":\"CPU\",\"type\":\"timeseries\",\"query\":\"rate(cpu[5m])\",\"position\":{\"x\":0,\"y\":0,\"w\":6,\"h\":3},\"config\":{}}]}',
             true)"
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn seed_inactive_template(pool: &sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO dashboard_templates (slug, name, category, dashboard_json, is_active)
             VALUES ('inactive-tmpl', 'Inactive', 'test', '{\"panels\":[]}', false)"
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app.oneshot(Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<DashboardTemplate> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn list_only_active(pool: sqlx::PgPool) {
        seed_template(&pool).await;
        seed_inactive_template(&pool).await;

        let app = test_app(pool);
        let resp = app.oneshot(Request::get("/").body(Body::empty()).unwrap()).await.unwrap();
        let items: Vec<DashboardTemplate> = body_json(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "test-tmpl");
    }

    #[sqlx::test]
    async fn use_template_creates_dashboard_and_panels(pool: sqlx::PgPool) {
        seed_template(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/test-tmpl/use", serde_json::json!({
                "title": "My Dashboard",
                "slug": "my-dash"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dashboard: super::super::dashboards::Dashboard = body_json(resp).await;
        assert_eq!(dashboard.title, "My Dashboard");
        assert_eq!(dashboard.slug, "my-dash");

        // Verify panels were created
        let panel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM panels WHERE dashboard_id = $1")
            .bind(dashboard.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(panel_count, 1);
    }

    #[sqlx::test]
    async fn use_template_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/nonexistent/use", serde_json::json!({
                "title": "X", "slug": "x"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn use_template_with_datasource_id(pool: sqlx::PgPool) {
        seed_template(&pool).await;
        let ds_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ('Prom', 'prometheus', 'http://prom:9090') RETURNING id"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/test-tmpl/use", serde_json::json!({
                "title": "With DS", "slug": "with-ds", "datasource_id": ds_id
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify panel has datasource_id set
        let dashboard: super::super::dashboards::Dashboard = body_json(resp).await;
        let panel_ds_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT datasource_id FROM panels WHERE dashboard_id = $1 LIMIT 1"
        )
        .bind(dashboard.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(panel_ds_id, Some(ds_id));
    }

    #[sqlx::test]
    async fn use_template_no_panels_key(pool: sqlx::PgPool) {
        sqlx::query(
            "INSERT INTO dashboard_templates (slug, name, category, dashboard_json, is_active)
             VALUES ('empty-tmpl', 'Empty', 'test', '{\"other\": true}', true)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request("POST", "/empty-tmpl/use", serde_json::json!({
                "title": "From Empty", "slug": "from-empty"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let dashboard: super::super::dashboards::Dashboard = body_json(resp).await;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM panels WHERE dashboard_id = $1")
            .bind(dashboard.id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

**Step 2: Run tests**

Run: `cd resource && cargo test api::templates::tests -- --nocapture`
Expected: all 6 tests pass

**Step 3: Commit**

```bash
git add resource/core/api/templates.rs
git commit -m "test: add integration tests for templates API"
```

---

### Task 13: Integration tests for explore + query API

**Files:**
- Modify: `resource/core/api/explore.rs`
- Modify: `resource/core/api/query.rs`

**Step 1: Write explore tests**

Add at the bottom of `resource/core/api/explore.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        explore_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_datasource(pool: &sqlx::PgPool, ds_type: &str, url: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ($1, $2, $3) RETURNING id"
        )
        .bind(format!("Test {}", ds_type))
        .bind(ds_type)
        .bind(url)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn explore_prometheus_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "vector", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": ds_id,
                "query": "up"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_prometheus_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": ds_id,
                "query": "up",
                "start": "1000",
                "end": "2000",
                "step": "15"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_loki_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "streams", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": ds_id,
                "query": "{job=\"app\"}"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_loki_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": ds_id,
                "query": "{job=\"app\"}",
                "start": "1000",
                "end": "2000",
                "limit": 50
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_unsupported_type(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool, "redis", "http://redis:6379").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": ds_id,
                "query": "KEYS *"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn explore_datasource_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("/query", serde_json::json!({
                "datasource_id": Uuid::new_v4(),
                "query": "up"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn explore_saves_history(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool.clone());
        app.oneshot(json_request("/query", serde_json::json!({
            "datasource_id": ds_id, "query": "up"
        }))).await.unwrap();

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/history").body(Body::empty()).unwrap())
            .await.unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].query, "up");
        assert_eq!(items[0].query_type, "prometheus");
    }

    #[sqlx::test]
    async fn history_with_datasource_filter(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool.clone());
        app.oneshot(json_request("/query", serde_json::json!({
            "datasource_id": ds_id, "query": "up"
        }))).await.unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::get(&format!("/history?datasource_id={}&limit=5", ds_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert_eq!(items.len(), 1);

        // Different datasource — empty
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/history?datasource_id={}", Uuid::new_v4())).body(Body::empty()).unwrap())
            .await.unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn label_values_prometheus(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": ["__name__", "job", "instance"]
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/labels/{}", ds_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["data"].as_array().unwrap().len(), 3);
    }

    #[sqlx::test]
    async fn label_values_loki(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": ["job"]
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/labels/{}", ds_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn label_values_unsupported_returns_empty(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool, "postgresql", "postgres://x").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/labels/{}", ds_id)).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["data"].as_array().unwrap().len(), 0);
    }

    #[sqlx::test]
    async fn label_values_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get(&format!("/labels/{}", Uuid::new_v4())).body(Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
```

**Step 2: Write query tests**

Add at the bottom of `resource/core/api/query.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        // Mount via datasource routes since query is nested under /{id}/query
        crate::api::datasources::datasource_routes().with_state(crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        })
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_ds(pool: &sqlx::PgPool, ds_type: &str, url: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ($1, $2, $3) RETURNING id"
        )
        .bind(format!("Test {}", ds_type))
        .bind(ds_type)
        .bind(url)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn proxy_prometheus_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {"resultType": "vector", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "prometheus", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", ds_id), serde_json::json!({
                "query": "up"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_prometheus_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "prometheus", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", ds_id), serde_json::json!({
                "query": "up", "start": "1000", "end": "2000", "step": "15"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_loki_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "loki", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", ds_id), serde_json::json!({
                "query": "{job=\"app\"}"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_loki_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "loki", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", ds_id), serde_json::json!({
                "query": "{job=\"app\"}", "start": "1000", "end": "2000"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_unsupported_type(pool: sqlx::PgPool) {
        let ds_id = seed_ds(&pool, "redis", "http://redis:6379").await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", ds_id), serde_json::json!({
                "query": "KEYS *"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn proxy_datasource_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(&format!("/{}/query", Uuid::new_v4()), serde_json::json!({
                "query": "up"
            })))
            .await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
```

**Step 3: Run tests**

Run: `cd resource && cargo test api::explore::tests -- --nocapture && cargo test api::query::tests -- --nocapture`
Expected: all tests pass (13 explore + 6 query)

**Step 4: Commit**

```bash
git add resource/core/api/explore.rs resource/core/api/query.rs
git commit -m "test: add integration tests for explore and query proxy APIs"
```

---

## PART 2: FRONTEND (VUE/TYPESCRIPT)

### Task 14: Add frontend test infrastructure

**Files:**
- Modify: `dashboard/package.json`
- Create: `dashboard/vitest.config.ts`

**Step 1: Install test dependencies with bun**

Run:
```bash
cd dashboard && bun add -d vitest @vue/test-utils @vitest/coverage-v8 jsdom
```

**Step 2: Create vitest.config.ts**

```typescript
import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    globals: true,
    root: fileURLToPath(new URL('./', import.meta.url)),
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,vue}'],
      exclude: ['src/main.ts', 'src/types/**'],
      thresholds: { statements: 100, branches: 100, functions: 100, lines: 100 },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
```

**Step 3: Add test script to package.json**

Add to scripts:
```json
"test": "vitest run",
"test:watch": "vitest",
"test:coverage": "vitest run --coverage"
```

**Step 4: Verify setup**

Run: `cd dashboard && bunx vitest run`
Expected: "No test files found" (no tests yet, but vitest works)

**Step 5: Commit**

```bash
git add dashboard/package.json dashboard/vitest.config.ts dashboard/bun.lockb
git commit -m "test: add vitest + vue test utils frontend test infrastructure"
```

---

### Task 15: Unit tests for `api/client.ts`

**Files:**
- Create: `dashboard/src/api/__tests__/client.test.ts`

**Step 1: Write tests**

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'

// Must import after fetch mock is set up
const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

// Dynamic import to ensure mock is active
const { api } = await import('../client')

beforeEach(() => {
  mockFetch.mockReset()
})

describe('api client', () => {
  describe('get', () => {
    it('sends GET request with correct headers', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ data: 'test' }),
      })

      const result = await api.get('/dashboards')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
      })
      expect(result).toEqual({ data: 'test' })
    })
  })

  describe('post', () => {
    it('sends POST with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.post('/dashboards', { title: 'New' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: JSON.stringify({ title: 'New' }),
      })
    })

    it('sends POST without body when no data', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({}),
      })

      await api.post('/dashboards/slug/star')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug/star', {
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        body: undefined,
      })
    })
  })

  describe('put', () => {
    it('sends PUT with JSON body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ id: '1' }),
      })

      await api.put('/dashboards/slug', { title: 'Updated' })

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
        body: JSON.stringify({ title: 'Updated' }),
      })
    })
  })

  describe('delete', () => {
    it('sends DELETE request', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(null),
      })

      await api.delete('/dashboards/slug')

      expect(mockFetch).toHaveBeenCalledWith('/api/v1/dashboards/slug', {
        headers: { 'Content-Type': 'application/json' },
        method: 'DELETE',
      })
    })
  })

  describe('error handling', () => {
    it('throws with server error message', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: () => Promise.resolve({ message: 'Not found' }),
      })

      await expect(api.get('/missing')).rejects.toThrow('Not found')
    })

    it('throws with status text when JSON parse fails', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        json: () => Promise.reject(new Error('invalid json')),
      })

      await expect(api.get('/broken')).rejects.toThrow('Internal Server Error')
    })

    it('throws generic message when no message in error body', async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 400,
        json: () => Promise.resolve({}),
      })

      await expect(api.get('/bad')).rejects.toThrow('HTTP 400')
    })
  })
})
```

**Step 2: Run test**

Run: `cd dashboard && bunx vitest run src/api/__tests__/client.test.ts`
Expected: all 8 tests pass

**Step 3: Commit**

```bash
git add dashboard/src/api/__tests__/client.test.ts
git commit -m "test: add unit tests for API client (all methods + error handling)"
```

---

### Task 16: Unit tests for API modules

**Files:**
- Create: `dashboard/src/api/__tests__/dashboards.test.ts`
- Create: `dashboard/src/api/__tests__/datasources.test.ts`
- Create: `dashboard/src/api/__tests__/alerts.test.ts`
- Create: `dashboard/src/api/__tests__/explore.test.ts`
- Create: `dashboard/src/api/__tests__/templates.test.ts`

**Step 1: Write all API module tests**

Each test verifies that the API module calls the client with correct paths and bodies. See design doc for coverage. All files follow the same pattern — mock `../client` and verify calls.

Example for `dashboards.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'

vi.mock('../client', () => ({
  api: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn().mockResolvedValue({}),
    put: vi.fn().mockResolvedValue({}),
    delete: vi.fn().mockResolvedValue(null),
  },
}))

import { dashboardsApi } from '../dashboards'
import { api } from '../client'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('dashboardsApi', () => {
  it('list calls GET /dashboards', async () => {
    await dashboardsApi.list()
    expect(api.get).toHaveBeenCalledWith('/dashboards')
  })

  it('get calls GET /dashboards/:slug', async () => {
    await dashboardsApi.get('my-dash')
    expect(api.get).toHaveBeenCalledWith('/dashboards/my-dash')
  })

  it('create calls POST /dashboards', async () => {
    const data = { title: 'New' }
    await dashboardsApi.create(data)
    expect(api.post).toHaveBeenCalledWith('/dashboards', data)
  })

  it('update calls PUT /dashboards/:slug', async () => {
    const data = { title: 'Updated' }
    await dashboardsApi.update('my-dash', data)
    expect(api.put).toHaveBeenCalledWith('/dashboards/my-dash', data)
  })

  it('remove calls DELETE /dashboards/:slug', async () => {
    await dashboardsApi.remove('my-dash')
    expect(api.delete).toHaveBeenCalledWith('/dashboards/my-dash')
  })

  it('toggleStar calls POST /dashboards/:slug/star', async () => {
    await dashboardsApi.toggleStar('my-dash')
    expect(api.post).toHaveBeenCalledWith('/dashboards/my-dash/star')
  })

  it('listPanels calls GET /dashboards/:slug/panels', async () => {
    await dashboardsApi.listPanels('my-dash')
    expect(api.get).toHaveBeenCalledWith('/dashboards/my-dash/panels')
  })

  it('addPanel calls POST /dashboards/:slug/panels', async () => {
    const data = { title: 'Panel', type: 'stat' as const }
    await dashboardsApi.addPanel('my-dash', data)
    expect(api.post).toHaveBeenCalledWith('/dashboards/my-dash/panels', data)
  })

  it('updatePanel calls PUT /panels/:id', async () => {
    const data = { title: 'Updated' }
    await dashboardsApi.updatePanel('abc', data)
    expect(api.put).toHaveBeenCalledWith('/panels/abc', data)
  })

  it('removePanel calls DELETE /panels/:id', async () => {
    await dashboardsApi.removePanel('abc')
    expect(api.delete).toHaveBeenCalledWith('/panels/abc')
  })
})
```

Follow same pattern for `datasources.test.ts`, `alerts.test.ts`, `explore.test.ts`, `templates.test.ts` — each verifying correct method + path + body for all exported functions.

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/api/__tests__/`
Expected: all tests pass

**Step 3: Commit**

```bash
git add dashboard/src/api/__tests__/
git commit -m "test: add unit tests for all API modules"
```

---

### Task 17: Unit tests for Pinia stores

**Files:**
- Create: `dashboard/src/stores/__tests__/dashboards.test.ts`
- Create: `dashboard/src/stores/__tests__/datasources.test.ts`

**Step 1: Write store tests**

`dashboards.test.ts`:

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

vi.mock('@/api/dashboards', () => ({
  dashboardsApi: {
    list: vi.fn(),
  },
}))

import { useDashboardStore } from '../dashboards'
import { dashboardsApi } from '@/api/dashboards'

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
})

describe('useDashboardStore', () => {
  it('initializes with empty items and loading false', () => {
    const store = useDashboardStore()
    expect(store.items).toEqual([])
    expect(store.loading).toBe(false)
  })

  it('fetchAll sets items and manages loading state', async () => {
    const mockDashboards = [{ id: '1', title: 'Test', slug: 'test' }]
    vi.mocked(dashboardsApi.list).mockResolvedValueOnce(mockDashboards as any)

    const store = useDashboardStore()
    await store.fetchAll()

    expect(store.items).toEqual(mockDashboards)
    expect(store.loading).toBe(false)
  })

  it('fetchAll resets loading on error', async () => {
    vi.mocked(dashboardsApi.list).mockRejectedValueOnce(new Error('fail'))

    const store = useDashboardStore()
    await expect(store.fetchAll()).rejects.toThrow('fail')
    expect(store.loading).toBe(false)
  })
})
```

Follow same pattern for `datasources.test.ts`.

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/stores/__tests__/`
Expected: all tests pass

**Step 3: Commit**

```bash
git add dashboard/src/stores/__tests__/
git commit -m "test: add unit tests for Pinia stores"
```

---

### Task 18: Unit tests for `composables/usePanelData.ts`

**Files:**
- Create: `dashboard/src/composables/__tests__/usePanelData.test.ts`

**Step 1: Write tests**

```typescript
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

vi.mock('@/api/datasources', () => ({
  datasourcesApi: {
    query: vi.fn(),
  },
}))

// Mock onUnmounted
vi.mock('vue', async () => {
  const actual = await vi.importActual('vue')
  return {
    ...actual,
    onUnmounted: vi.fn((cb: () => void) => cb), // capture the callback
  }
})

import { usePanelData } from '../usePanelData'
import { datasourcesApi } from '@/api/datasources'
import { onUnmounted } from 'vue'
import type { Panel } from '@/types'

const mockPanel: Panel = {
  id: '1',
  dashboard_id: 'd1',
  title: 'Test',
  type: 'stat',
  datasource_id: 'ds-1',
  query: 'up',
  config: {},
  position: { x: 0, y: 0, w: 3, h: 2, i: '1' },
  created_at: '',
  updated_at: '',
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.mocked(datasourcesApi.query).mockResolvedValue({ data: 'result' })
  vi.mocked(onUnmounted).mockImplementation(() => {})
})

afterEach(() => {
  vi.useRealTimers()
})

describe('usePanelData', () => {
  it('fetches data on creation', async () => {
    const { data, loading } = usePanelData(mockPanel, '1h', 0)

    // Wait for the async fetchData to complete
    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalledTimes(1)
    })

    expect(datasourcesApi.query).toHaveBeenCalledWith('ds-1', expect.objectContaining({
      query: 'up',
    }))
  })

  it('skips fetch when no datasource_id', () => {
    const panel = { ...mockPanel, datasource_id: undefined }
    usePanelData(panel, '1h', 0)
    expect(datasourcesApi.query).not.toHaveBeenCalled()
  })

  it('sets up refresh interval', () => {
    usePanelData(mockPanel, '1h', 30)
    expect(onUnmounted).toHaveBeenCalled()
  })

  it('calculates correct time range for 5m', async () => {
    const now = 1700000000
    vi.setSystemTime(now * 1000)

    usePanelData(mockPanel, '5m', 0)

    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalled()
    })

    const call = vi.mocked(datasourcesApi.query).mock.calls[0]
    expect(call[1].start).toBe((now - 300).toString())
    expect(call[1].end).toBe(now.toString())
  })

  it('falls back to 1h for unknown range', async () => {
    const now = 1700000000
    vi.setSystemTime(now * 1000)

    usePanelData(mockPanel, 'unknown', 0)

    await vi.waitFor(() => {
      expect(datasourcesApi.query).toHaveBeenCalled()
    })

    const call = vi.mocked(datasourcesApi.query).mock.calls[0]
    expect(call[1].start).toBe((now - 3600).toString())
  })

  it('exposes refresh function', () => {
    const { refresh } = usePanelData(mockPanel, '1h', 0)
    expect(typeof refresh).toBe('function')
  })
})
```

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/composables/__tests__/`
Expected: all tests pass

**Step 3: Commit**

```bash
git add dashboard/src/composables/__tests__/
git commit -m "test: add unit tests for usePanelData composable"
```

---

### Task 19: Unit tests for router

**Files:**
- Create: `dashboard/src/router/__tests__/index.test.ts`

**Step 1: Write tests**

```typescript
import { describe, it, expect } from 'vitest'
import router from '../index'

describe('router', () => {
  it('redirects / to /dashboards', () => {
    const root = router.getRoutes().find((r) => r.path === '')
    expect(root?.redirect).toBe('/dashboards')
  })

  const expectedRoutes = [
    '/dashboards',
    '/dashboards/new',
    '/dashboards/:slug',
    '/dashboards/:slug/edit',
    '/explore',
    '/alerts',
    '/alerts/rules/new',
    '/alerts/rules/:id',
    '/alerts/events',
    '/datasources',
    '/datasources/new',
    '/datasources/:id',
    '/templates',
    '/settings',
  ]

  for (const path of expectedRoutes) {
    it(`has route for ${path}`, () => {
      const route = router.getRoutes().find((r) => r.path === path)
      expect(route, `Route ${path} should exist`).toBeDefined()
    })
  }
})
```

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/router/__tests__/`
Expected: all 15 tests pass

**Step 3: Commit**

```bash
git add dashboard/src/router/__tests__/
git commit -m "test: add unit tests for Vue router configuration"
```

---

### Task 20: Component tests for panel components

**Files:**
- Create: `dashboard/src/components/panels/__tests__/PanelRenderer.test.ts`
- Create: `dashboard/src/components/panels/__tests__/panels.test.ts`

These tests shallow-mount panel components and verify they render without errors and accept correct props. Third-party components (ECharts, uPlot, AG Grid, xterm.js) are stubbed.

**Step 1: Write PanelRenderer test**

```typescript
import { describe, it, expect } from 'vitest'
import { shallowMount } from '@vue/test-utils'
import PanelRenderer from '../PanelRenderer.vue'
```

Read `PanelRenderer.vue` first to understand its interface, then write tests that verify correct component selection per panel type.

**Step 2: Write panel component tests**

Each panel component: verify it mounts without error with minimal props. Stub heavy third-party deps.

**Step 3: Run tests**

Run: `cd dashboard && bunx vitest run src/components/`
Expected: all pass

**Step 4: Commit**

```bash
git add dashboard/src/components/panels/__tests__/
git commit -m "test: add component tests for panel components"
```

---

### Task 21: Component tests for views

**Files:**
- Create: `dashboard/src/views/__tests__/` (one test file per view or grouped)

Shallow-mount each view, stub router and store dependencies, verify:
1. Component mounts without error
2. Calls expected store/API methods on mount
3. Key elements (headings, buttons) are present

**Step 1: Write view tests**

Read each view file first to understand its dependencies, then write focused tests.

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/views/__tests__/`
Expected: all pass

**Step 3: Commit**

```bash
git add dashboard/src/views/__tests__/
git commit -m "test: add component tests for all views"
```

---

### Task 22: Component tests for layouts

**Files:**
- Create: `dashboard/src/layouts/__tests__/AppLayout.test.ts`
- Create: `dashboard/src/layouts/__tests__/AppSidebar.test.ts`

**Step 1: Write tests**

Verify AppLayout renders router-view + AppSidebar. Verify AppSidebar renders all nav links.

**Step 2: Run tests**

Run: `cd dashboard && bunx vitest run src/layouts/__tests__/`
Expected: all pass

**Step 3: Commit**

```bash
git add dashboard/src/layouts/__tests__/
git commit -m "test: add component tests for layout components"
```

---

### Task 23: Final verification

**Step 1: Run full backend test suite**

Run: `cd resource && cargo test`
Expected: all tests pass

**Step 2: Run full frontend test suite with coverage**

Run: `cd dashboard && bunx vitest run --coverage`
Expected: all tests pass, coverage at/near 100%

**Step 3: Run CI checks**

Run: `cd resource && cargo clippy -- -D warnings && cargo fmt --check`
Run: `cd dashboard && bunx eslint . && bunx prettier --check src/`
Expected: no lint/format errors

**Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "test: achieve 100% test coverage for Strata codebase"
```
