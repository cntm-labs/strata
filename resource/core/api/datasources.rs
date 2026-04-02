use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
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
        .route("/{id}/query", post(super::query::proxy_query))
}

async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Datasource>>> {
    let rows =
        sqlx::query_as::<_, Datasource>("SELECT * FROM datasources ORDER BY created_at DESC")
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
        "prometheus" => reqwest::get(format!("{}/-/healthy", ds.url))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        "loki" => reqwest::get(format!("{}/ready", ds.url))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false),
        "postgresql" => sqlx::PgPool::connect(&ds.url)
            .await
            .map(|_| true)
            .unwrap_or(false),
        _ => false,
    };

    Ok(Json(serde_json::json!({ "success": ok })))
}
