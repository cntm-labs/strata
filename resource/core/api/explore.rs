use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::datasource::{loki::LokiClient, prometheus::PrometheusClient};
use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExploreHistory {
    pub id: Uuid,
    pub datasource_id: Uuid,
    pub query: String,
    pub query_type: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ExploreQueryRequest {
    pub datasource_id: Uuid,
    pub query: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub step: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub datasource_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub fn explore_routes() -> Router<AppState> {
    Router::new()
        .route("/query", post(explore_query))
        .route("/history", get(list_history))
        .route("/labels/{datasource_id}", get(label_values))
}

async fn explore_query(
    State(state): State<AppState>,
    Json(input): Json<ExploreQueryRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(input.datasource_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    // Save to history
    let query_type = ds.ds_type.clone();
    sqlx::query(
        "INSERT INTO explore_history (datasource_id, query, query_type) VALUES ($1, $2, $3)",
    )
    .bind(input.datasource_id)
    .bind(&input.query)
    .bind(&query_type)
    .execute(&state.db)
    .await?;

    // Execute query via proxy
    let result = match ds.ds_type.as_str() {
        "prometheus" => {
            let client = PrometheusClient::new(&ds.url);
            if let (Some(start), Some(end), Some(step)) = (&input.start, &input.end, &input.step) {
                serde_json::to_value(client.query_range(&input.query, start, end, step).await?)?
            } else {
                serde_json::to_value(client.query(&input.query, None, None).await?)?
            }
        }
        "loki" => {
            let client = LokiClient::new(&ds.url);
            if let (Some(start), Some(end)) = (&input.start, &input.end) {
                serde_json::to_value(
                    client
                        .query_range(&input.query, start, end, input.limit)
                        .await?,
                )?
            } else {
                serde_json::to_value(client.query(&input.query, input.limit).await?)?
            }
        }
        "postgresql" => {
            let rows = crate::datasource::postgresql::execute_query(&ds.url, &input.query).await?;
            serde_json::to_value(rows)?
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unsupported datasource type: {}",
                other
            )))
        }
    };

    Ok(Json(result))
}

async fn list_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> AppResult<Json<Vec<ExploreHistory>>> {
    let limit = params.limit.unwrap_or(50);

    let rows = if let Some(ds_id) = params.datasource_id {
        sqlx::query_as::<_, ExploreHistory>(
            "SELECT * FROM explore_history WHERE datasource_id = $1
             ORDER BY created_at DESC LIMIT $2",
        )
        .bind(ds_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, ExploreHistory>(
            "SELECT * FROM explore_history ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(rows))
}

async fn label_values(
    State(state): State<AppState>,
    Path(datasource_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(datasource_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    let result = match ds.ds_type.as_str() {
        "prometheus" => {
            let client = reqwest::Client::new();
            let resp = client
                .get(format!("{}/api/v1/labels", ds.url.trim_end_matches('/')))
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
            resp
        }
        "loki" => {
            let client = reqwest::Client::new();
            let resp = client
                .get(format!(
                    "{}/loki/api/v1/labels",
                    ds.url.trim_end_matches('/')
                ))
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
            resp
        }
        _ => serde_json::json!({ "data": [] }),
    };

    Ok(Json(result))
}
