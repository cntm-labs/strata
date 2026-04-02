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
