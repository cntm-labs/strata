use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;

use crate::db::TenantTx;
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
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn use_template(
    State(state): State<AppState>,
    mut tx: TenantTx,
    Path(template_slug): Path<String>,
    Json(input): Json<UseTemplate>,
) -> AppResult<Json<super::dashboards::Dashboard>> {
    // dashboard_templates is global (no RLS), so read it from the raw pool.
    let template =
        sqlx::query_as::<_, DashboardTemplate>("SELECT * FROM dashboard_templates WHERE slug = $1")
            .bind(&template_slug)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Template not found".into()))?;

    let tenant_id = tx.tenant_id();

    // Create dashboard from template
    let dashboard = sqlx::query_as::<_, super::dashboards::Dashboard>(
        "INSERT INTO dashboards (tenant_id, title, slug, description, layout)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(&input.title)
    .bind(&input.slug)
    .bind(&template.description)
    .bind(serde_json::json!([]))
    .fetch_one(&mut *tx)
    .await?;

    // Create panels from template JSON
    let panels = template
        .dashboard_json
        .get("panels")
        .and_then(|p| p.as_array());
    if let Some(panels) = panels {
        for panel_json in panels {
            sqlx::query(
                "INSERT INTO panels (tenant_id, dashboard_id, title, type, datasource_id, query, config, position)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(tenant_id)
            .bind(dashboard.id)
            .bind(
                panel_json
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled"),
            )
            .bind(
                panel_json
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("stat"),
            )
            .bind(input.datasource_id)
            .bind(
                panel_json
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            )
            .bind(
                panel_json
                    .get("config")
                    .unwrap_or(&serde_json::json!({})),
            )
            .bind(
                panel_json
                    .get("position")
                    .unwrap_or(&serde_json::json!({"x":0,"y":0,"w":6,"h":3})),
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(Json(dashboard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            pool: db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: None,
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        template_routes()
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::middleware::tenant::inject_tenant,
            ))
            .with_state(state)
    }

    const MOCK_TENANT: Uuid = Uuid::from_u128(0);

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
             VALUES ('inactive-tmpl', 'Inactive', 'test', '{\"panels\":[]}', false)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test]
    async fn chorus_overview_has_10_panels(pool: sqlx::PgPool) {
        let template = sqlx::query_as::<_, DashboardTemplate>(
            "SELECT * FROM dashboard_templates WHERE slug = 'chorus-overview'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let panels = template
            .dashboard_json
            .get("panels")
            .and_then(|p| p.as_array())
            .expect("chorus-overview should have panels array");
        assert_eq!(panels.len(), 10, "chorus-overview should have 10 panels");

        let types: Vec<&str> = panels
            .iter()
            .filter_map(|p| p.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(types.contains(&"stat"));
        assert!(types.contains(&"gauge"));
        assert!(types.contains(&"timeseries"));
        assert!(types.contains(&"piechart"));
    }

    #[sqlx::test]
    async fn list_returns_seeded_templates(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<DashboardTemplate> = body_json(resp).await;
        // Migration 002 seeds 6 + 004 adds 2 + 006 adds 1 (strata-health) = 9 active templates
        assert_eq!(items.len(), 9);
        assert!(items.iter().all(|t| t.is_active));
    }

    #[sqlx::test]
    async fn list_excludes_inactive(pool: sqlx::PgPool) {
        seed_inactive_template(&pool).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let items: Vec<DashboardTemplate> = body_json(resp).await;
        // 9 seeded active + 0 inactive (inactive-tmpl excluded)
        assert_eq!(items.len(), 9);
        assert!(!items.iter().any(|t| t.slug == "inactive-tmpl"));
    }

    #[sqlx::test]
    async fn strata_health_template_seeded_with_eight_panels(pool: sqlx::PgPool) {
        let template = sqlx::query_as::<_, DashboardTemplate>(
            "SELECT * FROM dashboard_templates WHERE slug = 'strata-health'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            template.required_datasource_type.as_deref(),
            Some("prometheus")
        );
        assert_eq!(template.category, "observability");
        assert!(template.is_active);

        let panels = template
            .dashboard_json
            .get("panels")
            .and_then(|p| p.as_array())
            .expect("strata-health should have panels array");
        assert_eq!(panels.len(), 8);

        // Spot-check that the active-connections gauge panel exists.
        let has_gauge = panels
            .iter()
            .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("gauge"));
        assert!(has_gauge, "strata-health should have a gauge panel");
    }

    #[sqlx::test]
    async fn chorus_logs_template_has_5_panels(pool: sqlx::PgPool) {
        let template = sqlx::query_as::<_, DashboardTemplate>(
            "SELECT * FROM dashboard_templates WHERE slug = 'chorus-logs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(template.required_datasource_type.as_deref(), Some("loki"));
        assert_eq!(template.category, "cpaas");

        let panels = template
            .dashboard_json
            .get("panels")
            .and_then(|p| p.as_array())
            .expect("chorus-logs should have panels array");
        assert_eq!(panels.len(), 5);

        // Verify logs panel type exists
        let has_logs = panels
            .iter()
            .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("logs"));
        assert!(has_logs, "chorus-logs should have a logs panel");
    }

    #[sqlx::test]
    async fn chorus_costs_template_has_6_panels(pool: sqlx::PgPool) {
        let template = sqlx::query_as::<_, DashboardTemplate>(
            "SELECT * FROM dashboard_templates WHERE slug = 'chorus-costs'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            template.required_datasource_type.as_deref(),
            Some("postgresql")
        );
        assert_eq!(template.category, "cpaas");

        let panels = template
            .dashboard_json
            .get("panels")
            .and_then(|p| p.as_array())
            .expect("chorus-costs should have panels array");
        assert_eq!(panels.len(), 6);

        // Verify table panel type exists (top accounts)
        let has_table = panels
            .iter()
            .any(|p| p.get("type").and_then(|t| t.as_str()) == Some("table"));
        assert!(has_table, "chorus-costs should have a table panel");
    }

    #[sqlx::test]
    async fn use_template_creates_dashboard_and_panels(pool: sqlx::PgPool) {
        seed_template(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/test-tmpl/use",
                serde_json::json!({
                    "title": "My Dashboard",
                    "slug": "my-dash"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dashboard: super::super::dashboards::Dashboard = body_json(resp).await;
        assert_eq!(dashboard.title, "My Dashboard");
        assert_eq!(dashboard.slug, "my-dash");

        // Verify panels were created
        let panel_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM panels WHERE dashboard_id = $1")
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
            .oneshot(json_request(
                "POST",
                "/nonexistent/use",
                serde_json::json!({
                    "title": "X", "slug": "x"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn use_template_with_datasource_id(pool: sqlx::PgPool) {
        seed_template(&pool).await;
        let ds_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url, tenant_id) VALUES ('Prom', 'prometheus', 'http://prom:9090', $1) RETURNING id"
        )
        .bind(MOCK_TENANT)
        .fetch_one(&pool)
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/test-tmpl/use",
                serde_json::json!({
                    "title": "With DS", "slug": "with-ds", "datasource_id": ds_id
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dashboard: super::super::dashboards::Dashboard = body_json(resp).await;

        // Verify panel has datasource_id set
        let panel_ds_id: Option<Uuid> =
            sqlx::query_scalar("SELECT datasource_id FROM panels WHERE dashboard_id = $1 LIMIT 1")
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
             VALUES ('empty-tmpl', 'Empty', 'test', '{\"other\": true}', true)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/empty-tmpl/use",
                serde_json::json!({
                    "title": "From Empty", "slug": "from-empty"
                }),
            ))
            .await
            .unwrap();
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
