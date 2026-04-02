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
    let template =
        sqlx::query_as::<_, DashboardTemplate>("SELECT * FROM dashboard_templates WHERE slug = $1")
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
    let panels = template
        .dashboard_json
        .get("panels")
        .and_then(|p| p.as_array());
    if let Some(panels) = panels {
        for panel_json in panels {
            sqlx::query(
                "INSERT INTO panels (dashboard_id, title, type, datasource_id, query, config, position)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
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
            .execute(&state.db)
            .await?;
        }
    }

    Ok(Json(dashboard))
}
