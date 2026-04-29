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
    .fetch_all(&state.pool)
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
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

async fn get_one(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Dashboard>> {
    let row = sqlx::query_as::<_, Dashboard>("SELECT * FROM dashboards WHERE slug = $1")
        .bind(&slug)
        .fetch_optional(&state.pool)
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
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;
    Ok(Json(row))
}

async fn remove(State(state): State<AppState>, Path(slug): Path<String>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM dashboards WHERE slug = $1")
        .bind(&slug)
        .execute(&state.pool)
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
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Dashboard not found".into()))?;
    Ok(Json(row))
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
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_secret_key: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        dashboard_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Dashboard> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/",
                serde_json::json!({
                    "title": "Test Dashboard",
                    "slug": "test-dash"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Dashboard = body_json(resp).await;
        assert_eq!(created.title, "Test Dashboard");
        assert_eq!(created.slug, "test-dash");
        assert_eq!(created.time_range, Some("1h".into()));
        assert_eq!(created.refresh_interval, Some(0));

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/test-dash").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: Dashboard = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/nonexistent").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_dashboard(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        app.oneshot(json_request(
            "POST",
            "/",
            serde_json::json!({
                "title": "Original", "slug": "update-test"
            }),
        ))
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "PUT",
                "/update-test",
                serde_json::json!({
                    "title": "Updated"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: Dashboard = body_json(resp).await;
        assert_eq!(updated.title, "Updated");
    }

    #[sqlx::test]
    async fn update_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "PUT",
                "/nonexistent",
                serde_json::json!({"title": "x"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_dashboard(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        app.oneshot(json_request(
            "POST",
            "/",
            serde_json::json!({
                "title": "To Delete", "slug": "delete-me"
            }),
        ))
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(Request::delete("/delete-me").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/delete-me").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn toggle_star(pool: sqlx::PgPool) {
        let app = test_app(pool.clone());
        app.oneshot(json_request(
            "POST",
            "/",
            serde_json::json!({
                "title": "Star Test", "slug": "star-test"
            }),
        ))
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::post("/star-test/star")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let starred: Dashboard = body_json(resp).await;
        assert_eq!(starred.is_starred, Some(true));

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post("/star-test/star")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let unstarred: Dashboard = body_json(resp).await;
        assert_eq!(unstarred.is_starred, Some(false));
    }

    #[sqlx::test]
    async fn toggle_star_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post("/nonexistent/star")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn create_with_all_optional_fields(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "POST",
                "/",
                serde_json::json!({
                    "title": "Full",
                    "slug": "full-dash",
                    "description": "A full dashboard",
                    "time_range": "24h",
                    "refresh_interval": 30,
                    "variables": [{"name": "host", "value": "localhost"}]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: Dashboard = body_json(resp).await;
        assert_eq!(created.description, Some("A full dashboard".into()));
        assert_eq!(created.time_range, Some("24h".into()));
        assert_eq!(created.refresh_interval, Some(30));
    }
}
