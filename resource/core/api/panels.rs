use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
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
        .route(
            "/dashboards/{slug}/panels",
            get(list_by_dashboard).post(create_for_dashboard),
        )
        .route("/panels/{id}", axum::routing::put(update).delete(remove))
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
    let dashboard_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM dashboards WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;

    let row = sqlx::query_as::<_, Panel>(
        "INSERT INTO panels (dashboard_id, title, type, datasource_id, query, config, position)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING *",
    )
    .bind(dashboard_id)
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
            "INSERT INTO dashboards (title, slug) VALUES ('Test', 'test-dash') RETURNING id",
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

        // Verify empty
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
}
