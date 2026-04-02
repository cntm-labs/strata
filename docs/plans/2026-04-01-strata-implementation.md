# Strata Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a fully functional Grafana alternative with Rust backend and Vue 3 frontend — supporting Prometheus, Loki, and PostgreSQL data sources with dashboards, panels, explore mode, alerts, and templates.

**Architecture:** Rust/Axum backend serves as API + datasource proxy. Vue 3 SPA for frontend with PrimeVue components, TailwindCSS/DaisyUI styling, and specialized charting libraries (uPlot for time series, ECharts for general charts, AG Grid for tables, xterm.js for logs). PostgreSQL stores all application state.

**Tech Stack:** Rust (Axum, sqlx, reqwest, serde), Vue 3 (TypeScript, Vite, Pinia, Vue Router), PrimeVue, TailwindCSS + DaisyUI, uPlot, ECharts (vue-echarts), AG Grid, xterm.js, vue-grid-layout, PostgreSQL 16

**Design Document:** `docs/plans/2026-04-01-strata-design.md`
**Sitemap:** `SITEMAP.md`

---

## Phase 1: Project Scaffolding & Infrastructure

### Task 1: Rust Backend Scaffolding

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `resource/Cargo.toml`
- Create: `resource/rustfmt.toml`
- Create: `resource/core/main.rs`
- Create: `resource/core/config/mod.rs`
- Create: `resource/core/error/mod.rs`

**Step 1: Create workspace Cargo.toml**

```toml
[workspace]
members = ["resource"]
resolver = "2"
```

**Step 2: Create resource/Cargo.toml with dependencies**

```toml
[package]
name = "strata-resource"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "strata"
path = "core/main.rs"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
reqwest = { version = "0.12", features = ["json"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
thiserror = "2"
```

**Step 3: Create rustfmt.toml**

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
```

**Step 4: Create unified error type at `resource/core/error/mod.rs`**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    status: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::Request(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = ErrorResponse {
            code: status.as_u16(),
            status: status.canonical_reason().unwrap_or("Error").to_string(),
            message,
        };

        (status, axum::Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
```

**Step 5: Create config module at `resource/core/config/mod.rs`**

```rust
use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://strata:secret@localhost:5432/strata".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
        }
    }
}
```

**Step 6: Create main.rs with health check**

```rust
mod config;
mod error;

use axum::{routing::get, Json, Router};
use config::AppConfig;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
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
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Strata listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

**Step 7: Verify backend compiles**

Run: `cd resource && cargo check`
Expected: Compilation succeeds

**Step 8: Commit**

```bash
git add Cargo.toml resource/
git commit -m "feat: scaffold Rust backend with Axum, health check, error types, config"
```

---

### Task 2: Vue 3 Frontend Scaffolding

**Files:**
- Create: `dashboard/` (entire Vue project via `npm create vue@latest`)
- Modify: `dashboard/package.json` (add dependencies)
- Modify: `dashboard/src/main.ts`
- Create: `dashboard/tailwind.config.ts`

**Step 1: Scaffold Vue project**

Run: `npm create vue@latest dashboard -- --typescript --router --pinia`
Expected: Vue project created at `dashboard/`

**Step 2: Install dependencies**

Run:
```bash
cd dashboard
npm install primevue @primevue/themes primeicons
npm install tailwindcss @tailwindcss/vite daisyui
npm install vue-echarts echarts
npm install uplot
npm install ag-grid-vue3 ag-grid-community
npm install @xterm/xterm
npm install vue-grid-layout
npm install axios
npm install -D @types/uplot
```

**Step 3: Configure Tailwind with DaisyUI**

Create `dashboard/src/assets/main.css`:
```css
@import "tailwindcss";
@plugin "daisyui";
```

**Step 4: Configure Vite with Tailwind plugin**

Modify `dashboard/vite.config.ts`:
```typescript
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
})
```

**Step 5: Configure PrimeVue in main.ts**

```typescript
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import PrimeVue from 'primevue/config'
import Aura from '@primevue/themes/aura'
import 'primeicons/primeicons.css'
import './assets/main.css'

import App from './App.vue'
import router from './router'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(PrimeVue, {
  theme: {
    preset: Aura,
  },
})
app.mount('#app')
```

**Step 6: Verify frontend builds**

Run: `cd dashboard && npm run build`
Expected: Build succeeds

**Step 7: Commit**

```bash
git add dashboard/
git commit -m "feat: scaffold Vue 3 frontend with PrimeVue, TailwindCSS, DaisyUI"
```

---

### Task 3: Database Schema & Migrations

**Files:**
- Create: `resource/migrations/001_initial.sql`

**Step 1: Install sqlx-cli**

Run: `cargo install sqlx-cli --no-default-features --features postgres`

**Step 2: Create migration file at `resource/migrations/001_initial.sql`**

```sql
-- Datasources
CREATE TABLE datasources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    type VARCHAR(50) NOT NULL,
    url TEXT NOT NULL,
    credentials_enc TEXT,
    is_default BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Dashboards
CREATE TABLE dashboards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    layout JSONB NOT NULL DEFAULT '[]',
    time_range VARCHAR(50) DEFAULT '1h',
    refresh_interval INT DEFAULT 0,
    variables JSONB DEFAULT '[]',
    is_starred BOOLEAN DEFAULT false,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Panels
CREATE TABLE panels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id UUID NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    type VARCHAR(50) NOT NULL,
    datasource_id UUID REFERENCES datasources(id),
    query TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    position JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Alert Rules
CREATE TABLE alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    datasource_id UUID NOT NULL REFERENCES datasources(id),
    query TEXT NOT NULL,
    condition VARCHAR(20) NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    duration_secs INT NOT NULL DEFAULT 60,
    severity VARCHAR(20) NOT NULL DEFAULT 'warning',
    notification_channels JSONB NOT NULL,
    notification_recipients JSONB NOT NULL,
    chorus_api_key_enc TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_evaluated_at TIMESTAMPTZ,
    current_state VARCHAR(20) DEFAULT 'ok',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Alert Events
CREATE TABLE alert_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id UUID NOT NULL REFERENCES alert_rules(id),
    state VARCHAR(20) NOT NULL,
    value DOUBLE PRECISION,
    message TEXT,
    notified_via JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- User Preferences
CREATE TABLE user_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    nucleus_user_id VARCHAR(255) NOT NULL UNIQUE,
    default_dashboard_id UUID REFERENCES dashboards(id),
    theme VARCHAR(20) DEFAULT 'system',
    timezone VARCHAR(50) DEFAULT 'Asia/Bangkok',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Dashboard Templates
CREATE TABLE dashboard_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(50) NOT NULL,
    thumbnail_url TEXT,
    dashboard_json JSONB NOT NULL,
    required_datasource_type VARCHAR(50),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Explore Query History
CREATE TABLE explore_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    datasource_id UUID NOT NULL REFERENCES datasources(id),
    query TEXT NOT NULL,
    query_type VARCHAR(20) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

**Step 3: Run migration**

Run: `cd resource && sqlx migrate run --source migrations`
Expected: Migration applied successfully

**Step 4: Commit**

```bash
git add resource/migrations/
git commit -m "feat: add initial database schema migration"
```

---

### Task 4: Docker Compose for Development

**Files:**
- Create: `docker-compose.yml`
- Create: `.env.example`

**Step 1: Create docker-compose.yml**

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_USER: strata
      POSTGRES_PASSWORD: secret
      POSTGRES_DB: strata
    ports:
      - "5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./dev/prometheus.yml:/etc/prometheus/prometheus.yml

  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"

volumes:
  pgdata:
```

**Step 2: Create .env.example**

```
DATABASE_URL=postgres://strata:secret@localhost:5432/strata
HOST=0.0.0.0
PORT=3000
```

**Step 3: Create dev/prometheus.yml**

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'strata'
    static_targets:
      - targets: ['host.docker.internal:3000']
```

**Step 4: Verify compose starts**

Run: `docker compose up -d`
Expected: postgres, prometheus, loki all healthy

**Step 5: Commit**

```bash
git add docker-compose.yml .env.example dev/
git commit -m "feat: add docker-compose for local dev (PostgreSQL, Prometheus, Loki)"
```

---

## Phase 2: Data Sources API & Proxy

### Task 5: Datasource CRUD API

**Files:**
- Create: `resource/core/api/mod.rs`
- Create: `resource/core/api/datasources.rs`
- Modify: `resource/core/main.rs` (add routes)

**Step 1: Write failing test for list datasources**

Add to `resource/core/api/datasources.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    // Helper to create test app (uses test database)
    async fn test_app() -> Router {
        let db = sqlx::PgPool::connect("postgres://strata:secret@localhost:5432/strata_test")
            .await
            .unwrap();
        let state = AppState { db, config: AppConfig::from_env() };
        datasource_routes().with_state(state)
    }

    #[tokio::test]
    async fn test_list_datasources_empty() {
        let app = test_app().await;
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd resource && cargo test test_list_datasources_empty`
Expected: FAIL — functions not defined

**Step 3: Implement datasource CRUD handlers**

```rust
use axum::{
    extract::{Path, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Datasource {
    pub id: Uuid,
    pub name: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub ds_type: String,
    pub url: String,
    pub credentials_enc: Option<String>,
    pub is_default: Option<bool>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDatasource {
    pub name: String,
    #[serde(rename = "type")]
    pub ds_type: String,
    pub url: String,
    pub credentials: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDatasource {
    pub name: Option<String>,
    pub url: Option<String>,
    pub credentials: Option<String>,
    pub is_default: Option<bool>,
}

pub fn datasource_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(get_one).put(update).delete(remove))
        .route("/{id}/test", post(test_connection))
}

async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Datasource>>> {
    let rows = sqlx::query_as::<_, Datasource>("SELECT * FROM datasources ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateDatasource>,
) -> AppResult<Json<Datasource>> {
    let row = sqlx::query_as::<_, Datasource>(
        "INSERT INTO datasources (name, type, url, credentials_enc, is_default)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(&input.name)
    .bind(&input.ds_type)
    .bind(&input.url)
    .bind(&input.credentials)
    .bind(input.is_default.unwrap_or(false))
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

async fn get_one(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Datasource>> {
    let row = sqlx::query_as::<_, Datasource>("SELECT * FROM datasources WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;
    Ok(Json(row))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateDatasource>,
) -> AppResult<Json<Datasource>> {
    let row = sqlx::query_as::<_, Datasource>(
        "UPDATE datasources SET
            name = COALESCE($2, name),
            url = COALESCE($3, url),
            credentials_enc = COALESCE($4, credentials_enc),
            is_default = COALESCE($5, is_default)
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.url)
    .bind(&input.credentials)
    .bind(input.is_default)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;
    Ok(Json(row))
}

async fn remove(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM datasources WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(()))
}

async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, Datasource>("SELECT * FROM datasources WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;

    let ok = match ds.ds_type.as_str() {
        "prometheus" => {
            reqwest::get(format!("{}/-/healthy", ds.url))
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        "loki" => {
            reqwest::get(format!("{}/ready", ds.url))
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        "postgresql" => {
            sqlx::PgPool::connect(&ds.url)
                .await
                .map(|_| true)
                .unwrap_or(false)
        }
        _ => false,
    };

    Ok(Json(serde_json::json!({ "success": ok })))
}
```

**Step 4: Wire routes in main.rs**

Add `mod api;` and nest datasource routes:
```rust
.nest("/api/v1/datasources", api::datasources::datasource_routes())
```

**Step 5: Run test to verify it passes**

Run: `cd resource && cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add resource/core/api/
git commit -m "feat: add datasource CRUD API with test connection"
```

---

### Task 6: Datasource Query Proxy

**Files:**
- Create: `resource/core/datasource/mod.rs`
- Create: `resource/core/datasource/prometheus.rs`
- Create: `resource/core/datasource/loki.rs`
- Create: `resource/core/datasource/postgresql.rs`
- Create: `resource/core/api/query.rs`

**Step 1: Write failing test for Prometheus query proxy**

```rust
#[tokio::test]
async fn test_prometheus_query_formats_request() {
    let client = PrometheusClient::new("http://localhost:9090");
    let result = client.query("up", None, None).await;
    assert!(result.is_ok());
}
```

**Step 2: Run test to verify it fails**

Run: `cd resource && cargo test test_prometheus_query`
Expected: FAIL

**Step 3: Implement Prometheus client**

```rust
// resource/core/datasource/prometheus.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::error::AppResult;

pub struct PrometheusClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusResponse {
    pub status: String,
    pub data: serde_json::Value,
}

impl PrometheusClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub async fn query(
        &self,
        query: &str,
        time: Option<&str>,
        timeout: Option<&str>,
    ) -> AppResult<PrometheusResponse> {
        let mut params = vec![("query", query.to_string())];
        if let Some(t) = time {
            params.push(("time", t.to_string()));
        }
        if let Some(t) = timeout {
            params.push(("timeout", t.to_string()));
        }

        let resp = self.client
            .get(format!("{}/api/v1/query", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<PrometheusResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn query_range(
        &self,
        query: &str,
        start: &str,
        end: &str,
        step: &str,
    ) -> AppResult<PrometheusResponse> {
        let params = [
            ("query", query),
            ("start", start),
            ("end", end),
            ("step", step),
        ];

        let resp = self.client
            .get(format!("{}/api/v1/query_range", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<PrometheusResponse>()
            .await?;
        Ok(resp)
    }
}
```

**Step 4: Implement Loki client**

```rust
// resource/core/datasource/loki.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::error::AppResult;

pub struct LokiClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LokiResponse {
    pub status: String,
    pub data: serde_json::Value,
}

impl LokiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub async fn query(&self, query: &str, limit: Option<u32>) -> AppResult<LokiResponse> {
        let mut params = vec![("query", query.to_string())];
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        let resp = self.client
            .get(format!("{}/loki/api/v1/query", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<LokiResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn query_range(
        &self,
        query: &str,
        start: &str,
        end: &str,
        limit: Option<u32>,
    ) -> AppResult<LokiResponse> {
        let mut params = vec![
            ("query", query.to_string()),
            ("start", start.to_string()),
            ("end", end.to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        let resp = self.client
            .get(format!("{}/loki/api/v1/query_range", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<LokiResponse>()
            .await?;
        Ok(resp)
    }
}
```

**Step 5: Implement PostgreSQL query executor**

```rust
// resource/core/datasource/postgresql.rs
use serde_json::Value;
use sqlx::PgPool;
use crate::error::AppResult;

pub async fn execute_query(connection_url: &str, query: &str) -> AppResult<Vec<Value>> {
    let pool = PgPool::connect(connection_url).await?;
    let rows = sqlx::query(query).fetch_all(&pool).await?;

    let results: Vec<Value> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            let columns = row.columns();
            let mut obj = serde_json::Map::new();
            for col in columns {
                let val: Value = match col.type_info().to_string().as_str() {
                    "INT4" | "INT8" => row
                        .try_get::<i64, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    "FLOAT4" | "FLOAT8" => row
                        .try_get::<f64, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    "BOOL" => row
                        .try_get::<bool, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    _ => row
                        .try_get::<String, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                };
                obj.insert(col.name().to_string(), val);
            }
            Value::Object(obj)
        })
        .collect();

    Ok(results)
}
```

**Step 6: Create unified query API handler**

```rust
// resource/core/api/query.rs
use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::datasource::{prometheus::PrometheusClient, loki::LokiClient, postgresql};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub step: Option<String>,
    pub limit: Option<u32>,
}

pub async fn proxy_query(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<QueryRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    let result = match ds.ds_type.as_str() {
        "prometheus" => {
            let client = PrometheusClient::new(&ds.url);
            if let (Some(start), Some(end), Some(step)) =
                (&input.start, &input.end, &input.step)
            {
                let resp = client.query_range(&input.query, start, end, step).await?;
                serde_json::to_value(resp).unwrap()
            } else {
                let resp = client.query(&input.query, None, None).await?;
                serde_json::to_value(resp).unwrap()
            }
        }
        "loki" => {
            let client = LokiClient::new(&ds.url);
            if let (Some(start), Some(end)) = (&input.start, &input.end) {
                let resp = client.query_range(&input.query, start, end, input.limit).await?;
                serde_json::to_value(resp).unwrap()
            } else {
                let resp = client.query(&input.query, input.limit).await?;
                serde_json::to_value(resp).unwrap()
            }
        }
        "postgresql" => {
            let rows = postgresql::execute_query(&ds.url, &input.query).await?;
            serde_json::to_value(rows).unwrap()
        }
        other => return Err(AppError::BadRequest(format!("Unsupported datasource type: {}", other))),
    };

    Ok(Json(result))
}
```

**Step 7: Wire query route in main.rs**

Add to datasource routes:
```rust
.route("/{id}/query", post(super::query::proxy_query))
```

**Step 8: Run all tests**

Run: `cd resource && cargo test`
Expected: PASS

**Step 9: Commit**

```bash
git add resource/core/datasource/ resource/core/api/query.rs
git commit -m "feat: add datasource query proxy for Prometheus, Loki, PostgreSQL"
```

---

## Phase 3: Dashboard & Panel CRUD API

### Task 7: Dashboard CRUD API

**Files:**
- Create: `resource/core/api/dashboards.rs`
- Modify: `resource/core/main.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_create_and_list_dashboards() {
    let app = test_app().await;
    // POST to create
    let response = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"title":"Test","slug":"test"}"#))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

**Step 2: Run test to verify it fails**

Run: `cd resource && cargo test test_create_and_list`
Expected: FAIL

**Step 3: Implement dashboard handlers**

```rust
// resource/core/api/dashboards.rs
use axum::{extract::{Path, State}, routing::{delete, get, post, put}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Dashboard {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub layout: JsonValue,
    pub time_range: Option<String>,
    pub refresh_interval: Option<i32>,
    pub variables: Option<JsonValue>,
    pub is_starred: Option<bool>,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDashboard {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub time_range: Option<String>,
    pub refresh_interval: Option<i32>,
    pub variables: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDashboard {
    pub title: Option<String>,
    pub description: Option<String>,
    pub layout: Option<JsonValue>,
    pub time_range: Option<String>,
    pub refresh_interval: Option<i32>,
    pub variables: Option<JsonValue>,
}

pub fn dashboard_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{slug}", get(get_one).put(update).delete(remove))
        .route("/{slug}/star", post(toggle_star))
}

async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Dashboard>>> {
    let rows = sqlx::query_as::<_, Dashboard>(
        "SELECT * FROM dashboards ORDER BY is_starred DESC, updated_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(state): State<AppState>,
    Json(input): Json<CreateDashboard>,
) -> AppResult<Json<Dashboard>> {
    let row = sqlx::query_as::<_, Dashboard>(
        "INSERT INTO dashboards (title, slug, description, time_range, refresh_interval, variables)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(&input.title)
    .bind(&input.slug)
    .bind(&input.description)
    .bind(input.time_range.as_deref().unwrap_or("1h"))
    .bind(input.refresh_interval.unwrap_or(0))
    .bind(&input.variables)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

async fn get_one(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Dashboard>> {
    let row = sqlx::query_as::<_, Dashboard>(
        "SELECT * FROM dashboards WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;
    Ok(Json(row))
}

async fn update(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<UpdateDashboard>,
) -> AppResult<Json<Dashboard>> {
    let row = sqlx::query_as::<_, Dashboard>(
        "UPDATE dashboards SET
            title = COALESCE($2, title),
            description = COALESCE($3, description),
            layout = COALESCE($4, layout),
            time_range = COALESCE($5, time_range),
            refresh_interval = COALESCE($6, refresh_interval),
            variables = COALESCE($7, variables),
            updated_at = now()
         WHERE slug = $1
         RETURNING *",
    )
    .bind(&slug)
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.layout)
    .bind(&input.time_range)
    .bind(input.refresh_interval)
    .bind(&input.variables)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;
    Ok(Json(row))
}

async fn remove(State(state): State<AppState>, Path(slug): Path<String>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM dashboards WHERE slug = $1")
        .bind(&slug)
        .execute(&state.db)
        .await?;
    Ok(Json(()))
}

async fn toggle_star(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Dashboard>> {
    let row = sqlx::query_as::<_, Dashboard>(
        "UPDATE dashboards SET is_starred = NOT COALESCE(is_starred, false), updated_at = now()
         WHERE slug = $1 RETURNING *",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;
    Ok(Json(row))
}
```

**Step 4: Wire routes in main.rs**

```rust
.nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
```

**Step 5: Run tests**

Run: `cd resource && cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add resource/core/api/dashboards.rs
git commit -m "feat: add dashboard CRUD API with star toggle"
```

---

### Task 8: Panel CRUD API

**Files:**
- Create: `resource/core/api/panels.rs`
- Modify: `resource/core/main.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_create_panel() {
    // Create dashboard first, then add panel
}
```

**Step 2: Run test to verify it fails**

Run: `cd resource && cargo test test_create_panel`
Expected: FAIL

**Step 3: Implement panel handlers**

```rust
// resource/core/api/panels.rs
use axum::{extract::{Path, State}, routing::{delete, get, post, put}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Panel {
    pub id: Uuid,
    pub dashboard_id: Uuid,
    pub title: String,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub panel_type: String,
    pub datasource_id: Option<Uuid>,
    pub query: String,
    pub config: JsonValue,
    pub position: JsonValue,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePanel {
    pub title: String,
    #[serde(rename = "type")]
    pub panel_type: String,
    pub datasource_id: Option<Uuid>,
    pub query: String,
    pub config: Option<JsonValue>,
    pub position: JsonValue,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePanel {
    pub title: Option<String>,
    pub query: Option<String>,
    pub config: Option<JsonValue>,
    pub position: Option<JsonValue>,
}

pub fn panel_routes_nested() -> Router<AppState> {
    Router::new()
        .route("/dashboards/{slug}/panels", get(list_by_dashboard).post(create_for_dashboard))
        .route("/panels/{id}", put(update).delete(remove))
}

async fn list_by_dashboard(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Vec<Panel>>> {
    let rows = sqlx::query_as::<_, Panel>(
        "SELECT p.* FROM panels p
         JOIN dashboards d ON d.id = p.dashboard_id
         WHERE d.slug = $1
         ORDER BY p.created_at",
    )
    .bind(&slug)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn create_for_dashboard(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(input): Json<CreatePanel>,
) -> AppResult<Json<Panel>> {
    let dashboard = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM dashboards WHERE slug = $1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;

    let row = sqlx::query_as::<_, Panel>(
        "INSERT INTO panels (dashboard_id, title, type, datasource_id, query, config, position)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(dashboard)
    .bind(&input.title)
    .bind(&input.panel_type)
    .bind(input.datasource_id)
    .bind(&input.query)
    .bind(input.config.as_ref().unwrap_or(&serde_json::json!({})))
    .bind(&input.position)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePanel>,
) -> AppResult<Json<Panel>> {
    let row = sqlx::query_as::<_, Panel>(
        "UPDATE panels SET
            title = COALESCE($2, title),
            query = COALESCE($3, query),
            config = COALESCE($4, config),
            position = COALESCE($5, position),
            updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.query)
    .bind(&input.config)
    .bind(&input.position)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Panel not found".into()))?;
    Ok(Json(row))
}

async fn remove(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM panels WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(()))
}
```

**Step 4: Wire routes**

```rust
.merge(api::panels::panel_routes_nested())
```

**Step 5: Run tests**

Run: `cd resource && cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add resource/core/api/panels.rs
git commit -m "feat: add panel CRUD API nested under dashboards"
```

---

## Phase 4: Frontend Shell & Routing

### Task 9: App Layout & Router Setup

**Files:**
- Create: `dashboard/src/layouts/AppLayout.vue`
- Create: `dashboard/src/layouts/Sidebar.vue`
- Modify: `dashboard/src/router/index.ts`
- Create: `dashboard/src/views/DashboardListView.vue`
- Create: `dashboard/src/views/ExploreView.vue`
- Create: `dashboard/src/views/AlertsView.vue`
- Create: `dashboard/src/views/DatasourceListView.vue`
- Create: `dashboard/src/views/TemplatesView.vue`
- Create: `dashboard/src/views/SettingsView.vue`

**Step 1: Create AppLayout with sidebar + main content area**

```vue
<!-- dashboard/src/layouts/AppLayout.vue -->
<template>
  <div class="flex h-screen bg-base-100">
    <Sidebar />
    <main class="flex-1 overflow-auto p-6">
      <RouterView />
    </main>
  </div>
</template>

<script setup lang="ts">
import Sidebar from './Sidebar.vue'
</script>
```

**Step 2: Create Sidebar navigation**

```vue
<!-- dashboard/src/layouts/Sidebar.vue -->
<template>
  <aside class="w-64 bg-base-200 flex flex-col border-r border-base-300">
    <div class="p-4 text-xl font-bold">Strata</div>
    <nav class="flex-1 p-2">
      <ul class="menu">
        <li v-for="item in navItems" :key="item.path">
          <RouterLink :to="item.path" class="flex items-center gap-2">
            <i :class="item.icon" />
            <span>{{ item.label }}</span>
          </RouterLink>
        </li>
      </ul>
    </nav>
  </aside>
</template>

<script setup lang="ts">
const navItems = [
  { path: '/dashboards', label: 'Dashboards', icon: 'pi pi-th-large' },
  { path: '/explore', label: 'Explore', icon: 'pi pi-search' },
  { path: '/alerts', label: 'Alerts', icon: 'pi pi-bell' },
  { path: '/datasources', label: 'Data Sources', icon: 'pi pi-database' },
  { path: '/templates', label: 'Templates', icon: 'pi pi-copy' },
  { path: '/settings', label: 'Settings', icon: 'pi pi-cog' },
]
</script>
```

**Step 3: Configure router with all routes from SITEMAP.md**

```typescript
// dashboard/src/router/index.ts
import { createRouter, createWebHistory } from 'vue-router'
import AppLayout from '@/layouts/AppLayout.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: AppLayout,
      children: [
        { path: '', redirect: '/dashboards' },
        { path: 'dashboards', name: 'dashboards', component: () => import('@/views/DashboardListView.vue') },
        { path: 'dashboards/new', name: 'dashboard-new', component: () => import('@/views/DashboardNewView.vue') },
        { path: 'dashboards/:slug', name: 'dashboard-view', component: () => import('@/views/DashboardView.vue') },
        { path: 'dashboards/:slug/edit', name: 'dashboard-edit', component: () => import('@/views/DashboardEditView.vue') },
        { path: 'explore', name: 'explore', component: () => import('@/views/ExploreView.vue') },
        { path: 'alerts', name: 'alerts', component: () => import('@/views/AlertsView.vue') },
        { path: 'alerts/rules/new', name: 'alert-rule-new', component: () => import('@/views/AlertRuleEditView.vue') },
        { path: 'alerts/rules/:id', name: 'alert-rule-edit', component: () => import('@/views/AlertRuleEditView.vue') },
        { path: 'alerts/events', name: 'alert-events', component: () => import('@/views/AlertEventsView.vue') },
        { path: 'datasources', name: 'datasources', component: () => import('@/views/DatasourceListView.vue') },
        { path: 'datasources/new', name: 'datasource-new', component: () => import('@/views/DatasourceEditView.vue') },
        { path: 'datasources/:id', name: 'datasource-edit', component: () => import('@/views/DatasourceEditView.vue') },
        { path: 'templates', name: 'templates', component: () => import('@/views/TemplatesView.vue') },
        { path: 'settings', name: 'settings', component: () => import('@/views/SettingsView.vue') },
      ],
    },
  ],
})

export default router
```

**Step 4: Create placeholder view components**

Each view file follows this pattern:
```vue
<template>
  <div>
    <h1 class="text-2xl font-bold mb-4">{{ title }}</h1>
    <p class="text-base-content/60">Coming soon...</p>
  </div>
</template>

<script setup lang="ts">
defineProps<{ title?: string }>()
</script>
```

Create placeholder files for: `DashboardListView`, `DashboardView`, `DashboardNewView`, `DashboardEditView`, `ExploreView`, `AlertsView`, `AlertRuleEditView`, `AlertEventsView`, `DatasourceListView`, `DatasourceEditView`, `TemplatesView`, `SettingsView`.

**Step 5: Verify frontend builds and router works**

Run: `cd dashboard && npm run build`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add dashboard/src/
git commit -m "feat: add app layout, sidebar navigation, and all route stubs"
```

---

### Task 10: API Client & Pinia Stores

**Files:**
- Create: `dashboard/src/api/client.ts`
- Create: `dashboard/src/api/datasources.ts`
- Create: `dashboard/src/api/dashboards.ts`
- Create: `dashboard/src/api/panels.ts`
- Create: `dashboard/src/stores/datasources.ts`
- Create: `dashboard/src/stores/dashboards.ts`
- Create: `dashboard/src/types/index.ts`

**Step 1: Create shared API client**

```typescript
// dashboard/src/api/client.ts
import axios from 'axios'

export const api = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
})
```

**Step 2: Create TypeScript types**

```typescript
// dashboard/src/types/index.ts
export interface Datasource {
  id: string
  name: string
  type: 'prometheus' | 'loki' | 'postgresql'
  url: string
  is_default: boolean
  created_at: string
}

export interface Dashboard {
  id: string
  title: string
  slug: string
  description?: string
  layout: PanelPosition[]
  time_range: string
  refresh_interval: number
  variables: TemplateVariable[]
  is_starred: boolean
  created_at: string
  updated_at: string
}

export interface Panel {
  id: string
  dashboard_id: string
  title: string
  type: PanelType
  datasource_id?: string
  query: string
  config: Record<string, unknown>
  position: PanelPosition
  created_at: string
  updated_at: string
}

export type PanelType = 'timeseries' | 'stat' | 'gauge' | 'table' | 'bar' | 'heatmap' | 'logs' | 'piechart'

export interface PanelPosition {
  x: number
  y: number
  w: number
  h: number
  i: string
}

export interface TemplateVariable {
  name: string
  label: string
  query: string
  datasource_id: string
  type: 'query' | 'custom' | 'interval'
  current: string
  options: string[]
}

export interface AlertRule {
  id: string
  name: string
  datasource_id: string
  query: string
  condition: 'gt' | 'lt' | 'eq' | 'gte' | 'lte'
  threshold: number
  duration_secs: number
  severity: 'info' | 'warning' | 'critical'
  notification_channels: string[]
  notification_recipients: string[]
  is_active: boolean
  current_state: 'ok' | 'firing' | 'pending'
  created_at: string
  updated_at: string
}

export interface AlertEvent {
  id: string
  rule_id: string
  state: string
  value?: number
  message?: string
  notified_via: string[]
  created_at: string
}
```

**Step 3: Create API modules**

```typescript
// dashboard/src/api/datasources.ts
import { api } from './client'
import type { Datasource } from '@/types'

export const datasourcesApi = {
  list: () => api.get<Datasource[]>('/datasources'),
  get: (id: string) => api.get<Datasource>(`/datasources/${id}`),
  create: (data: Partial<Datasource>) => api.post<Datasource>('/datasources', data),
  update: (id: string, data: Partial<Datasource>) => api.put<Datasource>(`/datasources/${id}`, data),
  remove: (id: string) => api.delete(`/datasources/${id}`),
  test: (id: string) => api.post<{ success: boolean }>(`/datasources/${id}/test`),
  query: (id: string, data: { query: string; start?: string; end?: string; step?: string }) =>
    api.post(`/datasources/${id}/query`, data),
}
```

```typescript
// dashboard/src/api/dashboards.ts
import { api } from './client'
import type { Dashboard, Panel } from '@/types'

export const dashboardsApi = {
  list: () => api.get<Dashboard[]>('/dashboards'),
  get: (slug: string) => api.get<Dashboard>(`/dashboards/${slug}`),
  create: (data: Partial<Dashboard>) => api.post<Dashboard>('/dashboards', data),
  update: (slug: string, data: Partial<Dashboard>) => api.put<Dashboard>(`/dashboards/${slug}`, data),
  remove: (slug: string) => api.delete(`/dashboards/${slug}`),
  toggleStar: (slug: string) => api.post<Dashboard>(`/dashboards/${slug}/star`),
  listPanels: (slug: string) => api.get<Panel[]>(`/dashboards/${slug}/panels`),
  addPanel: (slug: string, data: Partial<Panel>) => api.post<Panel>(`/dashboards/${slug}/panels`, data),
  updatePanel: (id: string, data: Partial<Panel>) => api.put<Panel>(`/panels/${id}`, data),
  removePanel: (id: string) => api.delete(`/panels/${id}`),
}
```

**Step 4: Create Pinia stores**

```typescript
// dashboard/src/stores/datasources.ts
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { datasourcesApi } from '@/api/datasources'
import type { Datasource } from '@/types'

export const useDatasourceStore = defineStore('datasources', () => {
  const items = ref<Datasource[]>([])
  const loading = ref(false)

  async function fetchAll() {
    loading.value = true
    try {
      const { data } = await datasourcesApi.list()
      items.value = data
    } finally {
      loading.value = false
    }
  }

  return { items, loading, fetchAll }
})
```

```typescript
// dashboard/src/stores/dashboards.ts
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { dashboardsApi } from '@/api/dashboards'
import type { Dashboard } from '@/types'

export const useDashboardStore = defineStore('dashboards', () => {
  const items = ref<Dashboard[]>([])
  const loading = ref(false)

  async function fetchAll() {
    loading.value = true
    try {
      const { data } = await dashboardsApi.list()
      items.value = data
    } finally {
      loading.value = false
    }
  }

  return { items, loading, fetchAll }
})
```

**Step 5: Verify typecheck passes**

Run: `cd dashboard && npm run typecheck`
Expected: No type errors

**Step 6: Commit**

```bash
git add dashboard/src/api/ dashboard/src/stores/ dashboard/src/types/
git commit -m "feat: add API client, TypeScript types, and Pinia stores"
```

---

## Phase 5: Core Frontend Pages

### Task 11: Dashboard List Page

**Files:**
- Modify: `dashboard/src/views/DashboardListView.vue`

**USER INPUT REQUESTED:** This is a design decision — how should the dashboard list display? Options:
1. **Card grid** (like New Relic) — visual thumbnails, good for browsing
2. **Table list** (like Grafana) — compact, good for many dashboards
3. **Hybrid** — starred dashboards as cards, rest as table

Implement the chosen approach with: starred dashboards first, search/filter, "New Dashboard" button.

**Step 1: Implement DashboardListView**

```vue
<!-- dashboard/src/views/DashboardListView.vue -->
<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Dashboards</h1>
      <RouterLink to="/dashboards/new" class="btn btn-primary">
        <i class="pi pi-plus mr-2" /> New Dashboard
      </RouterLink>
    </div>

    <InputText v-model="search" placeholder="Search dashboards..." class="w-full mb-4" />

    <div v-if="loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>

    <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <div
        v-for="dash in filtered"
        :key="dash.id"
        class="card bg-base-200 shadow-sm hover:shadow-md transition cursor-pointer"
        @click="$router.push(`/dashboards/${dash.slug}`)"
      >
        <div class="card-body">
          <div class="flex items-center justify-between">
            <h2 class="card-title text-lg">{{ dash.title }}</h2>
            <button
              class="btn btn-ghost btn-sm"
              @click.stop="toggleStar(dash.slug)"
            >
              <i :class="dash.is_starred ? 'pi pi-star-fill text-warning' : 'pi pi-star'" />
            </button>
          </div>
          <p class="text-sm text-base-content/60">{{ dash.description || 'No description' }}</p>
          <div class="text-xs text-base-content/40 mt-2">
            Updated {{ formatDate(dash.updated_at) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useDashboardStore } from '@/stores/dashboards'
import { dashboardsApi } from '@/api/dashboards'
import InputText from 'primevue/inputtext'
import ProgressSpinner from 'primevue/progressspinner'

const store = useDashboardStore()
const search = ref('')

const loading = computed(() => store.loading)
const filtered = computed(() =>
  store.items.filter((d) =>
    d.title.toLowerCase().includes(search.value.toLowerCase()),
  ),
)

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString()
}

async function toggleStar(slug: string) {
  await dashboardsApi.toggleStar(slug)
  await store.fetchAll()
}

onMounted(() => store.fetchAll())
</script>
```

**Step 2: Verify it builds**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 3: Commit**

```bash
git add dashboard/src/views/DashboardListView.vue
git commit -m "feat: implement dashboard list page with search and star toggle"
```

---

### Task 12: Datasource List & Edit Pages

**Files:**
- Modify: `dashboard/src/views/DatasourceListView.vue`
- Modify: `dashboard/src/views/DatasourceEditView.vue`

**Step 1: Implement DatasourceListView**

A table listing all datasources with type badges, test connection button, and add new button.

```vue
<!-- dashboard/src/views/DatasourceListView.vue -->
<template>
  <div>
    <div class="flex items-center justify-between mb-6">
      <h1 class="text-2xl font-bold">Data Sources</h1>
      <RouterLink to="/datasources/new" class="btn btn-primary">
        <i class="pi pi-plus mr-2" /> Add Data Source
      </RouterLink>
    </div>

    <DataTable :value="store.items" :loading="store.loading" stripedRows>
      <Column field="name" header="Name" />
      <Column field="type" header="Type">
        <template #body="{ data }">
          <span class="badge" :class="typeBadgeClass(data.type)">{{ data.type }}</span>
        </template>
      </Column>
      <Column field="url" header="URL" />
      <Column field="is_default" header="Default">
        <template #body="{ data }">
          <i v-if="data.is_default" class="pi pi-check text-success" />
        </template>
      </Column>
      <Column header="Actions">
        <template #body="{ data }">
          <div class="flex gap-2">
            <button class="btn btn-sm btn-ghost" @click="testDs(data.id)">Test</button>
            <RouterLink :to="`/datasources/${data.id}`" class="btn btn-sm btn-ghost">Edit</RouterLink>
            <button class="btn btn-sm btn-ghost text-error" @click="removeDs(data.id)">Delete</button>
          </div>
        </template>
      </Column>
    </DataTable>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useDatasourceStore } from '@/stores/datasources'
import { datasourcesApi } from '@/api/datasources'
import DataTable from 'primevue/datatable'
import Column from 'primevue/column'

const store = useDatasourceStore()

function typeBadgeClass(type: string) {
  return {
    prometheus: 'badge-primary',
    loki: 'badge-secondary',
    postgresql: 'badge-accent',
  }[type] || 'badge-ghost'
}

async function testDs(id: string) {
  const { data } = await datasourcesApi.test(id)
  alert(data.success ? 'Connection successful!' : 'Connection failed')
}

async function removeDs(id: string) {
  if (confirm('Delete this datasource?')) {
    await datasourcesApi.remove(id)
    await store.fetchAll()
  }
}

onMounted(() => store.fetchAll())
</script>
```

**Step 2: Implement DatasourceEditView**

**USER INPUT REQUESTED:** The datasource edit form needs to handle credentials. How should credentials be input?
- **Option A:** Plain text field (simplest, credentials encrypted at rest by backend)
- **Option B:** Key-value pairs for headers/basic auth
- **Option C:** Different form per datasource type (Prometheus = URL only, PostgreSQL = connection string)

```vue
<!-- dashboard/src/views/DatasourceEditView.vue -->
<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">{{ isNew ? 'Add' : 'Edit' }} Data Source</h1>

    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Name</label>
        <InputText v-model="form.name" class="w-full" required />
      </div>

      <div class="form-control">
        <label class="label">Type</label>
        <Select v-model="form.type" :options="typeOptions" optionLabel="label" optionValue="value" class="w-full" />
      </div>

      <div class="form-control">
        <label class="label">URL</label>
        <InputText v-model="form.url" class="w-full" placeholder="http://prometheus:9090" required />
      </div>

      <div class="form-control">
        <label class="label">Credentials (optional)</label>
        <InputText v-model="form.credentials" class="w-full" type="password" />
      </div>

      <div class="form-control">
        <label class="label cursor-pointer justify-start gap-2">
          <input type="checkbox" v-model="form.is_default" class="checkbox" />
          <span>Set as default</span>
        </label>
      </div>

      <div class="flex gap-2">
        <button type="submit" class="btn btn-primary">Save</button>
        <RouterLink to="/datasources" class="btn btn-ghost">Cancel</RouterLink>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { datasourcesApi } from '@/api/datasources'
import InputText from 'primevue/inputtext'
import Select from 'primevue/select'

const route = useRoute()
const router = useRouter()
const isNew = computed(() => route.name === 'datasource-new')

const typeOptions = [
  { label: 'Prometheus', value: 'prometheus' },
  { label: 'Loki', value: 'loki' },
  { label: 'PostgreSQL', value: 'postgresql' },
]

const form = reactive({
  name: '',
  type: 'prometheus',
  url: '',
  credentials: '',
  is_default: false,
})

async function save() {
  if (isNew.value) {
    await datasourcesApi.create(form)
  } else {
    await datasourcesApi.update(route.params.id as string, form)
  }
  router.push('/datasources')
}

onMounted(async () => {
  if (!isNew.value) {
    const { data } = await datasourcesApi.get(route.params.id as string)
    Object.assign(form, data)
  }
})
</script>
```

**Step 3: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 4: Commit**

```bash
git add dashboard/src/views/DatasourceListView.vue dashboard/src/views/DatasourceEditView.vue
git commit -m "feat: implement datasource list and edit pages"
```

---

## Phase 6: Dashboard View & Panel Rendering

### Task 13: Panel Renderer Components

**Files:**
- Create: `dashboard/src/components/panels/PanelRenderer.vue`
- Create: `dashboard/src/components/panels/TimeseriesPanel.vue`
- Create: `dashboard/src/components/panels/StatPanel.vue`
- Create: `dashboard/src/components/panels/GaugePanel.vue`
- Create: `dashboard/src/components/panels/TablePanel.vue`
- Create: `dashboard/src/components/panels/BarPanel.vue`
- Create: `dashboard/src/components/panels/HeatmapPanel.vue`
- Create: `dashboard/src/components/panels/LogsPanel.vue`
- Create: `dashboard/src/components/panels/PiechartPanel.vue`

**Step 1: Create PanelRenderer (dynamic component loader)**

```vue
<!-- dashboard/src/components/panels/PanelRenderer.vue -->
<template>
  <div class="card bg-base-200 h-full flex flex-col">
    <div class="card-body p-3 flex flex-col">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-semibold truncate">{{ panel.title }}</h3>
        <button v-if="editable" class="btn btn-ghost btn-xs" @click="$emit('edit', panel)">
          <i class="pi pi-pencil" />
        </button>
      </div>
      <div class="flex-1 min-h-0">
        <component :is="panelComponent" :data="data" :config="panel.config" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, type Component } from 'vue'
import type { Panel } from '@/types'
import TimeseriesPanel from './TimeseriesPanel.vue'
import StatPanel from './StatPanel.vue'
import GaugePanel from './GaugePanel.vue'
import TablePanel from './TablePanel.vue'
import BarPanel from './BarPanel.vue'
import HeatmapPanel from './HeatmapPanel.vue'
import LogsPanel from './LogsPanel.vue'
import PiechartPanel from './PiechartPanel.vue'

const props = defineProps<{
  panel: Panel
  data: unknown
  editable?: boolean
}>()

defineEmits<{ edit: [panel: Panel] }>()

const componentMap: Record<string, Component> = {
  timeseries: TimeseriesPanel,
  stat: StatPanel,
  gauge: GaugePanel,
  table: TablePanel,
  bar: BarPanel,
  heatmap: HeatmapPanel,
  logs: LogsPanel,
  piechart: PiechartPanel,
}

const panelComponent = computed(() => componentMap[props.panel.type] || StatPanel)
</script>
```

**Step 2: Create TimeseriesPanel (uPlot)**

```vue
<!-- dashboard/src/components/panels/TimeseriesPanel.vue -->
<template>
  <div ref="chartEl" class="w-full h-full" />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()
const chartEl = ref<HTMLElement>()
let chart: uPlot | null = null

function render() {
  if (!chartEl.value || !props.data) return
  chart?.destroy()

  const { width, height } = chartEl.value.getBoundingClientRect()
  // Transform Prometheus matrix data to uPlot format
  // data.result[].values[][] → [timestamps[], series1[], series2[], ...]
  const result = (props.data as any)?.data?.result || []
  if (result.length === 0) return

  const timestamps = result[0].values.map((v: [number, string]) => v[0])
  const series = result.map((r: any) => r.values.map((v: [number, string]) => parseFloat(v[1])))

  const opts: uPlot.Options = {
    width,
    height,
    series: [
      {},
      ...result.map((r: any, i: number) => ({
        label: Object.values(r.metric).join(' ') || `Series ${i + 1}`,
        stroke: `hsl(${i * 60}, 70%, 50%)`,
      })),
    ],
  }

  chart = new uPlot(opts, [timestamps, ...series], chartEl.value)
}

onMounted(render)
watch(() => props.data, render)
onUnmounted(() => chart?.destroy())
</script>
```

**Step 3: Create StatPanel**

```vue
<!-- dashboard/src/components/panels/StatPanel.vue -->
<template>
  <div class="flex items-center justify-center h-full">
    <div class="text-center">
      <div class="text-4xl font-bold">{{ displayValue }}</div>
      <div v-if="config.unit" class="text-sm text-base-content/60">{{ config.unit }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()

const displayValue = computed(() => {
  const result = (props.data as any)?.data?.result
  if (!result || result.length === 0) return '—'
  const val = result[0]?.value?.[1] || result[0]?.values?.slice(-1)?.[0]?.[1]
  if (val === undefined) return '—'
  const num = parseFloat(val)
  return isNaN(num) ? val : num.toLocaleString(undefined, { maximumFractionDigits: 2 })
})
</script>
```

**Step 4: Create GaugePanel (ECharts)**

```vue
<!-- dashboard/src/components/panels/GaugePanel.vue -->
<template>
  <v-chart :option="chartOption" autoresize class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { GaugeChart } from 'echarts/charts'
import { CanvasRenderer } from 'echarts/renderers'

use([GaugeChart, CanvasRenderer])

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()

const value = computed(() => {
  const result = (props.data as any)?.data?.result
  if (!result || result.length === 0) return 0
  return parseFloat(result[0]?.value?.[1] || '0')
})

const chartOption = computed(() => ({
  series: [
    {
      type: 'gauge',
      data: [{ value: value.value }],
      max: (props.config.max as number) || 100,
    },
  ],
}))
</script>
```

**Step 5: Create TablePanel (AG Grid)**

```vue
<!-- dashboard/src/components/panels/TablePanel.vue -->
<template>
  <AgGridVue
    class="ag-theme-alpine-dark w-full h-full"
    :rowData="rowData"
    :columnDefs="columnDefs"
    :defaultColDef="{ sortable: true, filter: true, resizable: true }"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { AgGridVue } from 'ag-grid-vue3'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()

const rowData = computed(() => {
  const result = (props.data as any)?.data?.result || props.data
  if (Array.isArray(result)) return result
  return []
})

const columnDefs = computed(() => {
  const first = rowData.value[0]
  if (!first) return []
  return Object.keys(first).map((key) => ({ field: key, headerName: key }))
})
</script>
```

**Step 6: Create BarPanel, HeatmapPanel, PiechartPanel (ECharts)**

Each follows the same pattern as GaugePanel but with appropriate chart type. (See ECharts docs for options.)

**Step 7: Create LogsPanel (xterm.js)**

```vue
<!-- dashboard/src/components/panels/LogsPanel.vue -->
<template>
  <div ref="termEl" class="w-full h-full" />
</template>

<script setup lang="ts">
import { ref, onMounted, watch, onUnmounted } from 'vue'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()
const termEl = ref<HTMLElement>()
let term: Terminal | null = null

function render() {
  if (!termEl.value) return
  if (!term) {
    term = new Terminal({ disableStdin: true, convertEol: true })
    term.open(termEl.value)
  }
  term.clear()

  const streams = (props.data as any)?.data?.result || []
  for (const stream of streams) {
    for (const [_ts, line] of stream.values || []) {
      term.writeln(line)
    }
  }
}

onMounted(render)
watch(() => props.data, render)
onUnmounted(() => term?.dispose())
</script>
```

**Step 8: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 9: Commit**

```bash
git add dashboard/src/components/panels/
git commit -m "feat: add all panel renderer components (timeseries, stat, gauge, table, bar, heatmap, logs, piechart)"
```

---

### Task 14: Dashboard View Page

**Files:**
- Modify: `dashboard/src/views/DashboardView.vue`
- Create: `dashboard/src/composables/usePanelData.ts`

**Step 1: Create composable for fetching panel data**

```typescript
// dashboard/src/composables/usePanelData.ts
import { ref, onUnmounted } from 'vue'
import { datasourcesApi } from '@/api/datasources'
import type { Panel } from '@/types'

export function usePanelData(panel: Panel, timeRange: string, refreshInterval: number) {
  const data = ref<unknown>(null)
  const loading = ref(false)
  let timer: ReturnType<typeof setInterval> | null = null

  async function fetch() {
    if (!panel.datasource_id) return
    loading.value = true
    try {
      const now = Math.floor(Date.now() / 1000)
      const rangeMap: Record<string, number> = {
        '5m': 300, '15m': 900, '30m': 1800, '1h': 3600,
        '3h': 10800, '6h': 21600, '12h': 43200, '24h': 86400,
      }
      const duration = rangeMap[timeRange] || 3600
      const { data: result } = await datasourcesApi.query(panel.datasource_id, {
        query: panel.query,
        start: (now - duration).toString(),
        end: now.toString(),
        step: Math.max(Math.floor(duration / 250), 15).toString(),
      })
      data.value = result
    } finally {
      loading.value = false
    }
  }

  fetch()
  if (refreshInterval > 0) {
    timer = setInterval(fetch, refreshInterval * 1000)
  }

  onUnmounted(() => {
    if (timer) clearInterval(timer)
  })

  return { data, loading, refresh: fetch }
}
```

**Step 2: Implement DashboardView with vue-grid-layout**

```vue
<!-- dashboard/src/views/DashboardView.vue -->
<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <div>
        <h1 class="text-2xl font-bold">{{ dashboard?.title }}</h1>
        <p class="text-sm text-base-content/60">{{ dashboard?.description }}</p>
      </div>
      <div class="flex gap-2">
        <Select v-model="timeRange" :options="timeRangeOptions" optionLabel="label" optionValue="value" />
        <RouterLink :to="`/dashboards/${slug}/edit`" class="btn btn-ghost">
          <i class="pi pi-pencil" /> Edit
        </RouterLink>
      </div>
    </div>

    <grid-layout
      v-if="panels.length > 0"
      :layout="gridLayout"
      :col-num="12"
      :row-height="80"
      :is-draggable="false"
      :is-resizable="false"
    >
      <grid-item
        v-for="panel in panels"
        :key="panel.id"
        :x="panel.position.x"
        :y="panel.position.y"
        :w="panel.position.w"
        :h="panel.position.h"
        :i="panel.id"
      >
        <PanelRenderer :panel="panel" :data="panelData[panel.id]" />
      </grid-item>
    </grid-layout>

    <div v-else class="text-center p-12 text-base-content/60">
      No panels yet.
      <RouterLink :to="`/dashboards/${slug}/edit`" class="link">Add one</RouterLink>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive } from 'vue'
import { useRoute } from 'vue-router'
import { dashboardsApi } from '@/api/dashboards'
import { datasourcesApi } from '@/api/datasources'
import PanelRenderer from '@/components/panels/PanelRenderer.vue'
import Select from 'primevue/select'
import type { Dashboard, Panel } from '@/types'

const route = useRoute()
const slug = computed(() => route.params.slug as string)
const dashboard = ref<Dashboard | null>(null)
const panels = ref<Panel[]>([])
const panelData = reactive<Record<string, unknown>>({})

const timeRangeOptions = [
  { label: '5m', value: '5m' },
  { label: '15m', value: '15m' },
  { label: '1h', value: '1h' },
  { label: '3h', value: '3h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
]
const timeRange = ref('1h')

const gridLayout = computed(() =>
  panels.value.map((p) => ({
    ...p.position,
    i: p.id,
  })),
)

async function fetchPanelData(panel: Panel) {
  if (!panel.datasource_id) return
  const now = Math.floor(Date.now() / 1000)
  const rangeMap: Record<string, number> = {
    '5m': 300, '15m': 900, '1h': 3600, '3h': 10800, '6h': 21600, '24h': 86400,
  }
  const duration = rangeMap[timeRange.value] || 3600
  const { data } = await datasourcesApi.query(panel.datasource_id, {
    query: panel.query,
    start: (now - duration).toString(),
    end: now.toString(),
    step: Math.max(Math.floor(duration / 250), 15).toString(),
  })
  panelData[panel.id] = data
}

onMounted(async () => {
  const { data: dash } = await dashboardsApi.get(slug.value)
  dashboard.value = dash
  timeRange.value = dash.time_range || '1h'

  const { data: p } = await dashboardsApi.listPanels(slug.value)
  panels.value = p

  await Promise.all(p.map(fetchPanelData))
})
</script>
```

**Step 3: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 4: Commit**

```bash
git add dashboard/src/views/DashboardView.vue dashboard/src/composables/
git commit -m "feat: implement dashboard view page with panel grid and data fetching"
```

---

### Task 15: Dashboard Edit Page

**Files:**
- Modify: `dashboard/src/views/DashboardEditView.vue`
- Create: `dashboard/src/components/PanelEditor.vue`

**USER INPUT REQUESTED:** The panel editor needs a query editor. How sophisticated should it be?
1. **Plain textarea** — simple, ship fast
2. **CodeMirror with syntax highlighting** — better DX, heavier
3. **Monaco editor** — full IDE experience, heaviest

**Step 1: Create PanelEditor modal**

```vue
<!-- dashboard/src/components/PanelEditor.vue -->
<template>
  <Dialog v-model:visible="visible" header="Edit Panel" :style="{ width: '600px' }" modal>
    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Title</label>
        <InputText v-model="form.title" class="w-full" required />
      </div>

      <div class="form-control">
        <label class="label">Type</label>
        <Select v-model="form.type" :options="panelTypes" optionLabel="label" optionValue="value" class="w-full" />
      </div>

      <div class="form-control">
        <label class="label">Datasource</label>
        <Select v-model="form.datasource_id" :options="datasources" optionLabel="name" optionValue="id" class="w-full" />
      </div>

      <div class="form-control">
        <label class="label">Query</label>
        <Textarea v-model="form.query" rows="4" class="w-full font-mono" />
      </div>

      <div class="grid grid-cols-4 gap-2">
        <div class="form-control">
          <label class="label text-xs">X</label>
          <InputNumber v-model="form.position.x" :min="0" :max="11" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Y</label>
          <InputNumber v-model="form.position.y" :min="0" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Width</label>
          <InputNumber v-model="form.position.w" :min="1" :max="12" />
        </div>
        <div class="form-control">
          <label class="label text-xs">Height</label>
          <InputNumber v-model="form.position.h" :min="1" />
        </div>
      </div>

      <div class="flex gap-2 justify-end">
        <button type="button" class="btn btn-ghost" @click="visible = false">Cancel</button>
        <button type="submit" class="btn btn-primary">Save</button>
      </div>
    </form>
  </Dialog>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted } from 'vue'
import { useDatasourceStore } from '@/stores/datasources'
import Dialog from 'primevue/dialog'
import InputText from 'primevue/inputtext'
import InputNumber from 'primevue/inputnumber'
import Textarea from 'primevue/textarea'
import Select from 'primevue/select'
import type { Panel } from '@/types'

const props = defineProps<{ panel?: Panel }>()
const emit = defineEmits<{ save: [data: Partial<Panel>]; close: [] }>()

const dsStore = useDatasourceStore()
const datasources = ref(dsStore.items)
const visible = ref(true)

const panelTypes = [
  { label: 'Time Series', value: 'timeseries' },
  { label: 'Stat', value: 'stat' },
  { label: 'Gauge', value: 'gauge' },
  { label: 'Table', value: 'table' },
  { label: 'Bar', value: 'bar' },
  { label: 'Heatmap', value: 'heatmap' },
  { label: 'Logs', value: 'logs' },
  { label: 'Pie Chart', value: 'piechart' },
]

const form = reactive({
  title: props.panel?.title || '',
  type: props.panel?.type || 'timeseries',
  datasource_id: props.panel?.datasource_id || '',
  query: props.panel?.query || '',
  position: {
    x: props.panel?.position?.x || 0,
    y: props.panel?.position?.y || 0,
    w: props.panel?.position?.w || 6,
    h: props.panel?.position?.h || 3,
  },
})

function save() {
  emit('save', { ...form, config: {} })
  visible.value = false
}

onMounted(() => dsStore.fetchAll())
</script>
```

**Step 2: Implement DashboardEditView with drag-and-drop layout**

```vue
<!-- dashboard/src/views/DashboardEditView.vue -->
<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <h1 class="text-2xl font-bold">Edit: {{ dashboard?.title }}</h1>
      <div class="flex gap-2">
        <button class="btn btn-ghost" @click="addPanel">
          <i class="pi pi-plus mr-2" /> Add Panel
        </button>
        <button class="btn btn-primary" @click="saveLayout">Save</button>
        <RouterLink :to="`/dashboards/${slug}`" class="btn btn-ghost">Back</RouterLink>
      </div>
    </div>

    <grid-layout
      v-if="panels.length > 0"
      v-model:layout="gridLayout"
      :col-num="12"
      :row-height="80"
      :is-draggable="true"
      :is-resizable="true"
      @layout-updated="onLayoutUpdated"
    >
      <grid-item
        v-for="panel in panels"
        :key="panel.id"
        :x="panel.position.x"
        :y="panel.position.y"
        :w="panel.position.w"
        :h="panel.position.h"
        :i="panel.id"
      >
        <PanelRenderer :panel="panel" :data="{}" :editable="true" @edit="editPanel" />
      </grid-item>
    </grid-layout>

    <PanelEditor
      v-if="showEditor"
      :panel="editingPanel"
      @save="onPanelSave"
      @close="showEditor = false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { dashboardsApi } from '@/api/dashboards'
import PanelRenderer from '@/components/panels/PanelRenderer.vue'
import PanelEditor from '@/components/PanelEditor.vue'
import type { Dashboard, Panel } from '@/types'

const route = useRoute()
const slug = computed(() => route.params.slug as string)
const dashboard = ref<Dashboard | null>(null)
const panels = ref<Panel[]>([])
const showEditor = ref(false)
const editingPanel = ref<Panel | undefined>()

const gridLayout = computed(() =>
  panels.value.map((p) => ({ ...p.position, i: p.id })),
)

function addPanel() {
  editingPanel.value = undefined
  showEditor.value = true
}

function editPanel(panel: Panel) {
  editingPanel.value = panel
  showEditor.value = true
}

async function onPanelSave(data: Partial<Panel>) {
  if (editingPanel.value) {
    await dashboardsApi.updatePanel(editingPanel.value.id, data)
  } else {
    await dashboardsApi.addPanel(slug.value, data)
  }
  const { data: p } = await dashboardsApi.listPanels(slug.value)
  panels.value = p
  showEditor.value = false
}

function onLayoutUpdated(layout: any[]) {
  // Sync grid positions back to panels
  for (const item of layout) {
    const panel = panels.value.find((p) => p.id === item.i)
    if (panel) {
      panel.position = { x: item.x, y: item.y, w: item.w, h: item.h, i: item.i }
    }
  }
}

async function saveLayout() {
  const layout = panels.value.map((p) => p.position)
  await dashboardsApi.update(slug.value, { layout })
  // Update each panel's position
  await Promise.all(
    panels.value.map((p) => dashboardsApi.updatePanel(p.id, { position: p.position })),
  )
}

onMounted(async () => {
  const { data: dash } = await dashboardsApi.get(slug.value)
  dashboard.value = dash
  const { data: p } = await dashboardsApi.listPanels(slug.value)
  panels.value = p
})
</script>
```

**Step 3: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 4: Commit**

```bash
git add dashboard/src/views/DashboardEditView.vue dashboard/src/components/PanelEditor.vue
git commit -m "feat: implement dashboard edit page with drag-and-drop panel layout"
```

---

## Phase 7: Explore Mode

### Task 16: Explore API

**Files:**
- Create: `resource/core/api/explore.rs`

**Step 1: Implement explore query handler**

```rust
// resource/core/api/explore.rs
use axum::{extract::State, Json, Router, routing::{get, post}};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ExploreQuery {
    pub datasource_id: Uuid,
    pub query: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub step: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExploreHistoryEntry {
    pub id: Uuid,
    pub datasource_id: Uuid,
    pub query: String,
    pub query_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub fn explore_routes() -> Router<AppState> {
    Router::new()
        .route("/query", post(execute_query))
        .route("/history", get(get_history))
}

async fn execute_query(
    State(state): State<AppState>,
    Json(input): Json<ExploreQuery>,
) -> AppResult<Json<serde_json::Value>> {
    // Reuse datasource query proxy logic
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(input.datasource_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    // Save to history
    sqlx::query(
        "INSERT INTO explore_history (datasource_id, query, query_type) VALUES ($1, $2, $3)",
    )
    .bind(input.datasource_id)
    .bind(&input.query)
    .bind(&ds.ds_type)
    .execute(&state.db)
    .await?;

    // Execute via proxy
    let query_req = super::query::QueryRequest {
        query: input.query,
        start: input.start,
        end: input.end,
        step: input.step,
        limit: input.limit,
    };

    super::query::proxy_query_internal(&state, input.datasource_id, query_req).await
}

async fn get_history(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<ExploreHistoryEntry>>> {
    let rows = sqlx::query_as::<_, ExploreHistoryEntry>(
        "SELECT * FROM explore_history ORDER BY created_at DESC LIMIT 50",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}
```

**Step 2: Wire routes**

```rust
.nest("/api/v1/explore", api::explore::explore_routes())
```

**Step 3: Run tests**

Run: `cd resource && cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add resource/core/api/explore.rs
git commit -m "feat: add explore query API with history"
```

---

### Task 17: Explore Frontend Page

**Files:**
- Modify: `dashboard/src/views/ExploreView.vue`

**USER INPUT REQUESTED:** The explore view auto-detects result type and renders the appropriate panel. How should the result type detection work for the UI?

**Step 1: Implement ExploreView**

```vue
<!-- dashboard/src/views/ExploreView.vue -->
<template>
  <div class="flex flex-col h-full">
    <h1 class="text-2xl font-bold mb-4">Explore</h1>

    <div class="flex gap-4 mb-4">
      <Select
        v-model="datasourceId"
        :options="datasources"
        optionLabel="name"
        optionValue="id"
        placeholder="Select datasource"
        class="w-64"
      />
      <Select v-model="timeRange" :options="timeRangeOptions" optionLabel="label" optionValue="value" />
    </div>

    <div class="flex gap-2 mb-4">
      <Textarea v-model="query" rows="3" class="flex-1 font-mono" placeholder="Enter query (PromQL, LogQL, or SQL)..." />
      <button class="btn btn-primary self-end" @click="runQuery" :disabled="!query || !datasourceId">
        <i class="pi pi-play mr-2" /> Run
      </button>
    </div>

    <div v-if="loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>

    <div v-else-if="result" class="flex-1 min-h-0">
      <div class="mb-2 text-sm text-base-content/60">
        Result type: <span class="badge badge-sm">{{ detectedType }}</span>
        <button class="btn btn-xs btn-ghost ml-2" @click="pinToDashboard">
          <i class="pi pi-bookmark mr-1" /> Pin to Dashboard
        </button>
      </div>
      <component :is="resultComponent" :data="result" :config="{}" class="h-96" />
    </div>

    <div v-if="history.length > 0" class="mt-4">
      <h2 class="text-lg font-semibold mb-2">Recent Queries</h2>
      <div class="space-y-1">
        <div
          v-for="entry in history"
          :key="entry.id"
          class="flex items-center gap-2 text-sm cursor-pointer hover:bg-base-200 p-1 rounded"
          @click="loadHistory(entry)"
        >
          <span class="badge badge-xs">{{ entry.query_type }}</span>
          <code class="flex-1 truncate">{{ entry.query }}</code>
          <span class="text-xs text-base-content/40">{{ formatDate(entry.created_at) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, type Component } from 'vue'
import { useDatasourceStore } from '@/stores/datasources'
import { api } from '@/api/client'
import Select from 'primevue/select'
import Textarea from 'primevue/textarea'
import ProgressSpinner from 'primevue/progressspinner'
import TimeseriesPanel from '@/components/panels/TimeseriesPanel.vue'
import StatPanel from '@/components/panels/StatPanel.vue'
import TablePanel from '@/components/panels/TablePanel.vue'
import LogsPanel from '@/components/panels/LogsPanel.vue'

const dsStore = useDatasourceStore()
const datasources = computed(() => dsStore.items)
const datasourceId = ref('')
const query = ref('')
const timeRange = ref('1h')
const result = ref<any>(null)
const loading = ref(false)
const history = ref<any[]>([])

const timeRangeOptions = [
  { label: '5m', value: '5m' },
  { label: '15m', value: '15m' },
  { label: '1h', value: '1h' },
  { label: '3h', value: '3h' },
  { label: '24h', value: '24h' },
]

const detectedType = computed(() => {
  if (!result.value) return 'unknown'
  const data = result.value?.data
  if (!data) return 'table' // SQL results
  const resultType = data.resultType
  if (resultType === 'matrix') return 'timeseries'
  if (resultType === 'vector') return 'table'
  if (resultType === 'scalar') return 'stat'
  if (resultType === 'streams') return 'logs'
  return 'table'
})

const resultComponent = computed<Component>(() => {
  const map: Record<string, Component> = {
    timeseries: TimeseriesPanel,
    stat: StatPanel,
    table: TablePanel,
    logs: LogsPanel,
  }
  return map[detectedType.value] || TablePanel
})

async function runQuery() {
  loading.value = true
  try {
    const now = Math.floor(Date.now() / 1000)
    const rangeMap: Record<string, number> = {
      '5m': 300, '15m': 900, '1h': 3600, '3h': 10800, '24h': 86400,
    }
    const duration = rangeMap[timeRange.value] || 3600
    const { data } = await api.post('/explore/query', {
      datasource_id: datasourceId.value,
      query: query.value,
      start: (now - duration).toString(),
      end: now.toString(),
      step: Math.max(Math.floor(duration / 250), 15).toString(),
    })
    result.value = data
    await fetchHistory()
  } finally {
    loading.value = false
  }
}

async function fetchHistory() {
  const { data } = await api.get('/explore/history')
  history.value = data
}

function loadHistory(entry: any) {
  query.value = entry.query
  datasourceId.value = entry.datasource_id
}

function pinToDashboard() {
  // TODO: show dialog to pick target dashboard, create panel
  alert('Pin to Dashboard — coming in next iteration')
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleString()
}

onMounted(() => {
  dsStore.fetchAll()
  fetchHistory()
})
</script>
```

**Step 2: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 3: Commit**

```bash
git add dashboard/src/views/ExploreView.vue
git commit -m "feat: implement explore mode with auto-detect result type rendering"
```

---

## Phase 8: Alerts

### Task 18: Alert Rules CRUD API

**Files:**
- Create: `resource/core/api/alerts.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_list_alert_rules() {
    let app = test_app().await;
    let response = app.oneshot(Request::builder().uri("/rules").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

**Step 2: Run test to verify it fails**

Run: `cd resource && cargo test test_list_alert_rules`
Expected: FAIL

**Step 3: Implement alert CRUD**

```rust
// resource/core/api/alerts.rs
use axum::{extract::{Path, State}, routing::{delete, get, post, put}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlertRule {
    pub id: Uuid,
    pub name: String,
    pub datasource_id: Uuid,
    pub query: String,
    pub condition: String,
    pub threshold: f64,
    pub duration_secs: i32,
    pub severity: String,
    pub notification_channels: JsonValue,
    pub notification_recipients: JsonValue,
    pub chorus_api_key_enc: Option<String>,
    pub is_active: bool,
    pub last_evaluated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub current_state: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AlertEvent {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub state: String,
    pub value: Option<f64>,
    pub message: Option<String>,
    pub notified_via: Option<JsonValue>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAlertRule {
    pub name: String,
    pub datasource_id: Uuid,
    pub query: String,
    pub condition: String,
    pub threshold: f64,
    pub duration_secs: Option<i32>,
    pub severity: Option<String>,
    pub notification_channels: JsonValue,
    pub notification_recipients: JsonValue,
}

pub fn alert_routes() -> Router<AppState> {
    Router::new()
        .route("/rules", get(list_rules).post(create_rule))
        .route("/rules/{id}", get(get_rule).put(update_rule).delete(delete_rule))
        .route("/events", get(list_events))
        .route("/test/{id}", post(test_fire))
}

async fn list_rules(State(state): State<AppState>) -> AppResult<Json<Vec<AlertRule>>> {
    let rows = sqlx::query_as::<_, AlertRule>(
        "SELECT * FROM alert_rules ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn create_rule(
    State(state): State<AppState>,
    Json(input): Json<CreateAlertRule>,
) -> AppResult<Json<AlertRule>> {
    let row = sqlx::query_as::<_, AlertRule>(
        "INSERT INTO alert_rules (name, datasource_id, query, condition, threshold, duration_secs, severity, notification_channels, notification_recipients)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING *",
    )
    .bind(&input.name)
    .bind(input.datasource_id)
    .bind(&input.query)
    .bind(&input.condition)
    .bind(input.threshold)
    .bind(input.duration_secs.unwrap_or(60))
    .bind(input.severity.as_deref().unwrap_or("warning"))
    .bind(&input.notification_channels)
    .bind(&input.notification_recipients)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

async fn get_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AlertRule>> {
    let row = sqlx::query_as::<_, AlertRule>("SELECT * FROM alert_rules WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Alert rule not found".into()))?;
    Ok(Json(row))
}

async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateAlertRule>,
) -> AppResult<Json<AlertRule>> {
    let row = sqlx::query_as::<_, AlertRule>(
        "UPDATE alert_rules SET
            name = $2, query = $3, condition = $4, threshold = $5,
            duration_secs = $6, severity = $7,
            notification_channels = $8, notification_recipients = $9,
            updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.query)
    .bind(&input.condition)
    .bind(input.threshold)
    .bind(input.duration_secs.unwrap_or(60))
    .bind(input.severity.as_deref().unwrap_or("warning"))
    .bind(&input.notification_channels)
    .bind(&input.notification_recipients)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Alert rule not found".into()))?;
    Ok(Json(row))
}

async fn delete_rule(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM alert_rules WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(()))
}

async fn list_events(State(state): State<AppState>) -> AppResult<Json<Vec<AlertEvent>>> {
    let rows = sqlx::query_as::<_, AlertEvent>(
        "SELECT * FROM alert_events ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn test_fire(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    // Execute the rule's query and check against threshold
    let rule = sqlx::query_as::<_, AlertRule>("SELECT * FROM alert_rules WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Alert rule not found".into()))?;

    // TODO: Execute query via datasource proxy and compare to threshold
    // TODO: Send notification via Chorus when integrated

    Ok(Json(serde_json::json!({
        "status": "test_fired",
        "rule_name": rule.name,
        "note": "Chorus integration pending"
    })))
}
```

**Step 4: Wire routes**

```rust
.nest("/api/v1/alerts", api::alerts::alert_routes())
```

**Step 5: Run tests**

Run: `cd resource && cargo test`
Expected: PASS

**Step 6: Commit**

```bash
git add resource/core/api/alerts.rs
git commit -m "feat: add alert rules CRUD API with events and test fire"
```

---

### Task 19: Alert Frontend Pages

**Files:**
- Modify: `dashboard/src/views/AlertsView.vue`
- Modify: `dashboard/src/views/AlertRuleEditView.vue`
- Modify: `dashboard/src/views/AlertEventsView.vue`
- Create: `dashboard/src/api/alerts.ts`

**Step 1: Create alerts API module**

```typescript
// dashboard/src/api/alerts.ts
import { api } from './client'
import type { AlertRule, AlertEvent } from '@/types'

export const alertsApi = {
  listRules: () => api.get<AlertRule[]>('/alerts/rules'),
  getRule: (id: string) => api.get<AlertRule>(`/alerts/rules/${id}`),
  createRule: (data: Partial<AlertRule>) => api.post<AlertRule>('/alerts/rules', data),
  updateRule: (id: string, data: Partial<AlertRule>) => api.put<AlertRule>(`/alerts/rules/${id}`, data),
  deleteRule: (id: string) => api.delete(`/alerts/rules/${id}`),
  listEvents: () => api.get<AlertEvent[]>('/alerts/events'),
  testFire: (id: string) => api.post(`/alerts/test/${id}`),
}
```

**Step 2: Implement AlertsView (overview page)**

Shows active alerts prominently + rule list with status badges.

**Step 3: Implement AlertRuleEditView**

Form for creating/editing alert rules with: name, datasource picker, query, condition (gt/lt/eq), threshold, duration, severity, notification channels.

**Step 4: Implement AlertEventsView**

Table of past alert events with filtering by state and time range.

**Step 5: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 6: Commit**

```bash
git add dashboard/src/views/Alerts* dashboard/src/api/alerts.ts
git commit -m "feat: implement alert overview, rule editor, and events pages"
```

---

## Phase 9: Templates

### Task 20: Template API & Seeding

**Files:**
- Create: `resource/core/api/templates.rs`
- Create: `resource/migrations/002_seed_templates.sql`

**Step 1: Create seed migration with built-in templates**

```sql
-- resource/migrations/002_seed_templates.sql
INSERT INTO dashboard_templates (slug, name, description, category, dashboard_json, required_datasource_type) VALUES
('node-exporter', 'Node Exporter', 'System metrics: CPU, memory, disk, network', 'infrastructure',
 '{"panels":[{"title":"CPU Usage","type":"timeseries","query":"100 - (avg by(instance)(rate(node_cpu_seconds_total{mode=\"idle\"}[5m])) * 100)","position":{"x":0,"y":0,"w":6,"h":3}},{"title":"Memory Usage","type":"gauge","query":"(1 - node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes) * 100","position":{"x":6,"y":0,"w":3,"h":3}},{"title":"Disk Usage","type":"gauge","query":"(1 - node_filesystem_avail_bytes{mountpoint=\"/\"} / node_filesystem_size_bytes{mountpoint=\"/\"}) * 100","position":{"x":9,"y":0,"w":3,"h":3}}]}',
 'prometheus'),
('chorus-overview', 'Chorus Overview', 'CPaaS metrics: messages sent, delivery rates, latency', 'cpaas',
 '{"panels":[{"title":"Messages Sent","type":"stat","query":"sum(chorus_messages_total)","position":{"x":0,"y":0,"w":3,"h":2}},{"title":"Delivery Rate","type":"gauge","query":"sum(rate(chorus_delivered_total[5m])) / sum(rate(chorus_messages_total[5m])) * 100","position":{"x":3,"y":0,"w":3,"h":2}},{"title":"Message Volume","type":"timeseries","query":"sum(rate(chorus_messages_total[5m])) by (channel)","position":{"x":0,"y":2,"w":12,"h":3}}]}',
 'prometheus'),
('nucleus-auth', 'Nucleus Auth', 'Auth metrics: login rate, active sessions, errors', 'auth',
 '{"panels":[{"title":"Active Sessions","type":"stat","query":"nucleus_active_sessions","position":{"x":0,"y":0,"w":3,"h":2}},{"title":"Login Rate","type":"timeseries","query":"rate(nucleus_login_total[5m])","position":{"x":3,"y":0,"w":9,"h":3}}]}',
 'prometheus'),
('postgresql', 'PostgreSQL', 'Database metrics: connections, query time, cache hit ratio', 'database',
 '{"panels":[{"title":"Active Connections","type":"stat","query":"pg_stat_activity_count","position":{"x":0,"y":0,"w":3,"h":2}},{"title":"Cache Hit Ratio","type":"gauge","query":"pg_stat_database_blks_hit / (pg_stat_database_blks_hit + pg_stat_database_blks_read) * 100","position":{"x":3,"y":0,"w":3,"h":2}},{"title":"Query Duration","type":"timeseries","query":"rate(pg_stat_statements_total_time[5m])","position":{"x":0,"y":2,"w":12,"h":3}}]}',
 'prometheus'),
('redis', 'Redis', 'Redis metrics: memory, ops/sec, hit rate', 'database',
 '{"panels":[{"title":"Memory Used","type":"stat","query":"redis_memory_used_bytes","position":{"x":0,"y":0,"w":3,"h":2}},{"title":"Ops/sec","type":"timeseries","query":"rate(redis_commands_processed_total[5m])","position":{"x":3,"y":0,"w":9,"h":3}}]}',
 'prometheus'),
('blank', 'Custom Blank', 'Start from scratch', 'custom',
 '{"panels":[]}',
 NULL);
```

**Step 2: Implement template API**

```rust
// resource/core/api/templates.rs
use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct DashboardTemplate {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub thumbnail_url: Option<String>,
    pub dashboard_json: JsonValue,
    pub required_datasource_type: Option<String>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UseTemplate {
    pub title: String,
    pub slug: String,
    pub datasource_id: Option<Uuid>,
}

pub fn template_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/{slug}/use", post(use_template))
}

async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<DashboardTemplate>>> {
    let rows = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE is_active = true ORDER BY category, name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn use_template(
    State(state): State<AppState>,
    Path(template_slug): Path<String>,
    Json(input): Json<UseTemplate>,
) -> AppResult<Json<super::dashboards::Dashboard>> {
    let template = sqlx::query_as::<_, DashboardTemplate>(
        "SELECT * FROM dashboard_templates WHERE slug = $1",
    )
    .bind(&template_slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Template not found".into()))?;

    // Create dashboard from template
    let dashboard = sqlx::query_as::<_, super::dashboards::Dashboard>(
        "INSERT INTO dashboards (title, slug, description, layout)
         VALUES ($1, $2, $3, $4)
         RETURNING *",
    )
    .bind(&input.title)
    .bind(&input.slug)
    .bind(&template.description)
    .bind(serde_json::json!([]))
    .fetch_one(&state.db)
    .await?;

    // Create panels from template JSON
    let panels = template.dashboard_json.get("panels").and_then(|p| p.as_array());
    if let Some(panels) = panels {
        for panel_json in panels {
            sqlx::query(
                "INSERT INTO panels (dashboard_id, title, type, datasource_id, query, config, position)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(dashboard.id)
            .bind(panel_json.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled"))
            .bind(panel_json.get("type").and_then(|v| v.as_str()).unwrap_or("stat"))
            .bind(input.datasource_id)
            .bind(panel_json.get("query").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(panel_json.get("config").unwrap_or(&serde_json::json!({})))
            .bind(panel_json.get("position").unwrap_or(&serde_json::json!({"x":0,"y":0,"w":6,"h":3})))
            .execute(&state.db)
            .await?;
        }
    }

    Ok(Json(dashboard))
}
```

**Step 3: Wire routes**

```rust
.nest("/api/v1/templates", api::templates::template_routes())
```

**Step 4: Commit**

```bash
git add resource/core/api/templates.rs resource/migrations/002_seed_templates.sql
git commit -m "feat: add template API with built-in dashboard templates"
```

---

### Task 21: Templates & New Dashboard Frontend

**Files:**
- Modify: `dashboard/src/views/TemplatesView.vue`
- Modify: `dashboard/src/views/DashboardNewView.vue`

**Step 1: Implement TemplatesView (gallery)**

Card grid with template previews, categorized by type (infrastructure, cpaas, auth, database).

**Step 2: Implement DashboardNewView**

Choose: "From Template" (shows template gallery) or "Blank Dashboard" (creates empty dashboard with slug input).

**Step 3: Verify build**

Run: `cd dashboard && npm run build`
Expected: PASS

**Step 4: Commit**

```bash
git add dashboard/src/views/TemplatesView.vue dashboard/src/views/DashboardNewView.vue
git commit -m "feat: implement template gallery and new dashboard page"
```

---

## Phase 10: Settings & Polish

### Task 22: User Settings Page

**Files:**
- Modify: `dashboard/src/views/SettingsView.vue`

**Step 1: Implement SettingsView**

```vue
<!-- Simple settings page for theme, timezone, default dashboard -->
<template>
  <div class="max-w-2xl">
    <h1 class="text-2xl font-bold mb-6">Settings</h1>
    <form @submit.prevent="save" class="space-y-4">
      <div class="form-control">
        <label class="label">Theme</label>
        <Select v-model="form.theme" :options="themeOptions" optionLabel="label" optionValue="value" class="w-full" />
      </div>
      <div class="form-control">
        <label class="label">Timezone</label>
        <InputText v-model="form.timezone" class="w-full" />
      </div>
      <div class="form-control">
        <label class="label">Default Dashboard</label>
        <Select v-model="form.default_dashboard_id" :options="dashboards" optionLabel="title" optionValue="id" class="w-full" showClear />
      </div>
      <button type="submit" class="btn btn-primary">Save</button>
    </form>
  </div>
</template>
```

**Step 2: Commit**

```bash
git add dashboard/src/views/SettingsView.vue
git commit -m "feat: implement user settings page"
```

---

## Phase 11: Deployment

### Task 23: Dockerfile & Production Build

**Files:**
- Create: `Dockerfile`
- Modify: `resource/core/main.rs` (serve static files)

**Step 1: Add static file serving to Rust backend**

```rust
// In main.rs, serve Vue dist as static files
use tower_http::services::ServeDir;

let app = Router::new()
    .route("/api/v1/health", get(health))
    .nest("/api/v1/datasources", api::datasources::datasource_routes())
    .nest("/api/v1/dashboards", api::dashboards::dashboard_routes())
    .merge(api::panels::panel_routes_nested())
    .nest("/api/v1/explore", api::explore::explore_routes())
    .nest("/api/v1/alerts", api::alerts::alert_routes())
    .nest("/api/v1/templates", api::templates::template_routes())
    .fallback_service(ServeDir::new("static").fallback(ServeDir::new("static/index.html")))
    .with_state(state)
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http());
```

Add to Cargo.toml:
```toml
tower-http = { version = "0.6", features = ["cors", "trace", "fs"] }
```

**Step 2: Create multi-stage Dockerfile**

```dockerfile
# Stage 1: Build frontend
FROM node:20-alpine AS frontend
WORKDIR /app/dashboard
COPY dashboard/package*.json ./
RUN npm ci
COPY dashboard/ ./
RUN npm run build

# Stage 2: Build backend
FROM rust:1.83 AS backend
WORKDIR /app
COPY Cargo.toml ./
COPY resource/ ./resource/
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/strata .
COPY --from=frontend /app/dashboard/dist ./static/
COPY resource/migrations ./migrations/
EXPOSE 3000
CMD ["./strata"]
```

**Step 3: Verify Docker build**

Run: `docker build -t strata:latest .`
Expected: Build succeeds

**Step 4: Update docker-compose.yml for production**

Add strata service:
```yaml
  strata:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://strata:secret@postgres:5432/strata
    depends_on:
      - postgres
```

**Step 5: Commit**

```bash
git add Dockerfile docker-compose.yml resource/
git commit -m "feat: add Dockerfile and production build with static file serving"
```

---

## Summary

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| 1 | 1-4 | Scaffolding: Rust backend, Vue frontend, DB schema, Docker dev env |
| 2 | 5-6 | Data Sources: CRUD API + Prometheus/Loki/PostgreSQL proxy |
| 3 | 7-8 | Dashboards & Panels: CRUD APIs |
| 4 | 9-10 | Frontend Shell: Layout, router, API client, stores, types |
| 5 | 11-12 | Core Pages: Dashboard list, datasource management |
| 6 | 13-15 | Dashboard Rendering: All 8 panel types + view/edit pages |
| 7 | 16-17 | Explore Mode: Ad-hoc queries with auto-detect rendering |
| 8 | 18-19 | Alerts: Rules CRUD + events + frontend pages |
| 9 | 20-21 | Templates: Built-in templates + new dashboard from template |
| 10 | 22 | Settings: Theme, timezone, default dashboard |
| 11 | 23 | Deployment: Multi-stage Docker build |

**Total: 23 tasks, ~16 frontend pages, ~25 API endpoints**

**Deferred (TODO):**
- Auth via Nucleus (integrate when ready)
- Alerting via Chorus (integrate when ready)
- Credential encryption at rest
- Template variable `{{variable}}` substitution in queries
- Dashboard sharing/permissions
