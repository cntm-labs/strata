use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::datasource::{loki::LokiClient, postgresql, prometheus::PrometheusClient};
use crate::error::{AppError, AppResult};
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
            if let (Some(start), Some(end), Some(step)) = (&input.start, &input.end, &input.step) {
                let resp = client.query_range(&input.query, start, end, step).await?;
                serde_json::to_value(resp)?
            } else {
                let resp = client.query(&input.query, None, None).await?;
                serde_json::to_value(resp)?
            }
        }
        "loki" => {
            let client = LokiClient::new(&ds.url);
            if let (Some(start), Some(end)) = (&input.start, &input.end) {
                let resp = client
                    .query_range(&input.query, start, end, input.limit)
                    .await?;
                serde_json::to_value(resp)?
            } else {
                let resp = client.query(&input.query, input.limit).await?;
                serde_json::to_value(resp)?
            }
        }
        "postgresql" => {
            let rows = postgresql::execute_query(&ds.url, &input.query).await?;
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
