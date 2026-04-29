use axum::{extract::Path, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use uuid::Uuid;

use crate::db::TenantTx;
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
        .route(
            "/dashboards/{slug}/panels",
            get(list_by_dashboard).post(create_for_dashboard),
        )
        .route("/panels/{id}", axum::routing::put(update).delete(remove))
}

async fn list_by_dashboard(
    mut tx: TenantTx,
    Path(slug): Path<String>,
) -> AppResult<Json<Vec<Panel>>> {
    let rows = sqlx::query_as::<_, Panel>(
        "SELECT p.* FROM panels p
         JOIN dashboards d ON d.id = p.dashboard_id
         WHERE d.slug = $1
         ORDER BY p.created_at",
    )
    .bind(&slug)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn create_for_dashboard(
    mut tx: TenantTx,
    Path(slug): Path<String>,
    Json(input): Json<CreatePanel>,
) -> AppResult<Json<Panel>> {
    let tenant_id = tx.tenant_id();
    let dashboard_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM dashboards WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;

    let row = sqlx::query_as::<_, Panel>(
        "INSERT INTO panels (tenant_id, dashboard_id, title, type, datasource_id, query, config, position)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(dashboard_id)
    .bind(&input.title)
    .bind(&input.panel_type)
    .bind(input.datasource_id)
    .bind(&input.query)
    .bind(input.config.as_ref().unwrap_or(&serde_json::json!({})))
    .bind(&input.position)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn update(
    mut tx: TenantTx,
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
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Panel not found".into()))?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn remove(mut tx: TenantTx, Path(id): Path<Uuid>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM panels WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// UUID injected by `inject_mock_tenant` middleware in tests.
    const MOCK_TENANT: Uuid = Uuid::from_u128(0);

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            pool: db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_secret_key: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        panel_routes_nested()
            .layer(axum::middleware::from_fn(
                crate::middleware::tenant::inject_mock_tenant,
            ))
            .with_state(state)
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
            "INSERT INTO dashboards (title, slug, tenant_id) \
             VALUES ('Test', 'test-dash', $1) RETURNING id",
        )
        .bind(MOCK_TENANT)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get("/dashboards/test-dash/panels")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Panel> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_list(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/dashboards/test-dash/panels",
                serde_json::json!({
                    "title": "CPU Panel",
                    "type": "timeseries",
                    "query": "rate(cpu[5m])",
                    "position": {"x": 0, "y": 0, "w": 6, "h": 3}
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Panel = body_json(resp).await;
        assert_eq!(created.title, "CPU Panel");
        assert_eq!(created.panel_type, "timeseries");

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get("/dashboards/test-dash/panels")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
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
            .oneshot(json_request(
                "POST",
                "/dashboards/test-dash/panels",
                serde_json::json!({
                    "title": "Old", "type": "stat", "query": "up",
                    "position": {"x": 0, "y": 0, "w": 3, "h": 2}
                }),
            ))
            .await
            .unwrap();
        let created: Panel = body_json(resp).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/panels/{}", created.id),
                serde_json::json!({
                    "title": "New Title", "query": "down"
                }),
            ))
            .await
            .unwrap();
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
            .oneshot(json_request(
                "PUT",
                &format!("/panels/{}", fake_id),
                serde_json::json!({"title": "x"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_panel(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/dashboards/test-dash/panels",
                serde_json::json!({
                    "title": "ToDelete", "type": "stat", "query": "up",
                    "position": {"x": 0, "y": 0, "w": 3, "h": 2}
                }),
            ))
            .await
            .unwrap();
        let created: Panel = body_json(resp).await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::delete(format!("/panels/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get("/dashboards/test-dash/panels")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items: Vec<Panel> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_with_optional_config(pool: sqlx::PgPool) {
        seed_dashboard(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "POST",
                "/dashboards/test-dash/panels",
                serde_json::json!({
                    "title": "With Config",
                    "type": "gauge",
                    "query": "mem_usage",
                    "config": {"min": 0, "max": 100},
                    "position": {"x": 0, "y": 0, "w": 3, "h": 3}
                }),
            ))
            .await
            .unwrap();
        let created: Panel = body_json(resp).await;
        assert_eq!(created.config["min"], 0);
        assert_eq!(created.config["max"], 100);
    }

    #[sqlx::test]
    async fn panels_visible_only_in_owning_tenant(pool: sqlx::PgPool) {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, 'A', $2), ($3, 'B', $4)")
            .bind(a)
            .bind(format!("a-{}", a))
            .bind(b)
            .bind(format!("b-{}", b))
            .execute(&pool)
            .await
            .unwrap();
        let dash_a: Uuid = sqlx::query_scalar(
            "INSERT INTO dashboards (title, slug, layout, tenant_id) \
             VALUES ('A','a','[]'::jsonb,$1) RETURNING id",
        )
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO panels (dashboard_id, title, type, query, position, tenant_id) \
             VALUES ($1, 'P', 'stat', '', '{}'::jsonb, $2)",
        )
        .bind(dash_a)
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL ROLE strata_app")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(b.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM panels")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        assert_eq!(count.0, 0, "tenant B must not see tenant A panels");
    }
}
