use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
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
    let row = sqlx::query_as::<_, Dashboard>("SELECT * FROM dashboards WHERE slug = $1")
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
