use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
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

#[derive(Debug, Deserialize)]
pub struct UpdateAlertRule {
    pub name: Option<String>,
    pub query: Option<String>,
    pub condition: Option<String>,
    pub threshold: Option<f64>,
    pub duration_secs: Option<i32>,
    pub severity: Option<String>,
    pub notification_channels: Option<JsonValue>,
    pub notification_recipients: Option<JsonValue>,
    pub is_active: Option<bool>,
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
pub struct EventsQuery {
    pub rule_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub fn alert_routes() -> Router<AppState> {
    Router::new()
        .route("/rules", get(list_rules).post(create_rule))
        .route(
            "/rules/{id}",
            get(get_rule).put(update_rule).delete(delete_rule),
        )
        .route("/events", get(list_events))
}

async fn list_rules(State(state): State<AppState>) -> AppResult<Json<Vec<AlertRule>>> {
    let rows = sqlx::query_as::<_, AlertRule>("SELECT * FROM alert_rules ORDER BY created_at DESC")
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
    Json(input): Json<UpdateAlertRule>,
) -> AppResult<Json<AlertRule>> {
    let row = sqlx::query_as::<_, AlertRule>(
        "UPDATE alert_rules SET
            name = COALESCE($2, name),
            query = COALESCE($3, query),
            condition = COALESCE($4, condition),
            threshold = COALESCE($5, threshold),
            duration_secs = COALESCE($6, duration_secs),
            severity = COALESCE($7, severity),
            notification_channels = COALESCE($8, notification_channels),
            notification_recipients = COALESCE($9, notification_recipients),
            is_active = COALESCE($10, is_active),
            updated_at = now()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.query)
    .bind(&input.condition)
    .bind(input.threshold)
    .bind(input.duration_secs)
    .bind(&input.severity)
    .bind(&input.notification_channels)
    .bind(&input.notification_recipients)
    .bind(input.is_active)
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

async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>,
) -> AppResult<Json<Vec<AlertEvent>>> {
    let limit = params.limit.unwrap_or(100);

    let rows = if let Some(rule_id) = params.rule_id {
        sqlx::query_as::<_, AlertEvent>(
            "SELECT * FROM alert_events WHERE rule_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(rule_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, AlertEvent>(
            "SELECT * FROM alert_events ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(rows))
}
