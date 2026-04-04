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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        explore_routes().with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_datasource(pool: &sqlx::PgPool, ds_type: &str, url: &str) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(format!("Test {}", ds_type))
        .bind(ds_type)
        .bind(url)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn explore_prometheus_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "vector", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": ds_id,
                    "query": "up"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_prometheus_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": ds_id,
                    "query": "up",
                    "start": "1000",
                    "end": "2000",
                    "step": "15"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_loki_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "streams", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": ds_id,
                    "query": "{job=\"app\"}"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_loki_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": ds_id,
                    "query": "{job=\"app\"}",
                    "start": "1000",
                    "end": "2000",
                    "limit": 50
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn explore_unsupported_type(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool, "redis", "http://redis:6379").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": ds_id,
                    "query": "KEYS *"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn explore_datasource_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "/query",
                serde_json::json!({
                    "datasource_id": Uuid::new_v4(),
                    "query": "up"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn explore_saves_history(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool.clone());
        app.oneshot(json_request(
            "/query",
            serde_json::json!({
                "datasource_id": ds_id, "query": "up"
            }),
        ))
        .await
        .unwrap();

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/history").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].query, "up");
        assert_eq!(items[0].query_type, "prometheus");
    }

    #[sqlx::test]
    async fn history_with_datasource_filter(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool.clone());
        app.oneshot(json_request(
            "/query",
            serde_json::json!({
                "datasource_id": ds_id, "query": "up"
            }),
        ))
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::get(&format!("/history?datasource_id={}&limit=5", ds_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert_eq!(items.len(), 1);

        // Different datasource — empty
        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(&format!("/history?datasource_id={}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let items: Vec<ExploreHistory> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn label_values_prometheus(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": ["__name__", "job", "instance"]
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(&format!("/labels/{}", ds_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["data"].as_array().unwrap().len(), 3);
    }

    #[sqlx::test]
    async fn label_values_loki(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": ["job"]
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_datasource(&pool, "loki", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(&format!("/labels/{}", ds_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn label_values_unsupported_returns_empty(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool, "postgresql", "postgres://x").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(&format!("/labels/{}", ds_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["data"].as_array().unwrap().len(), 0);
    }

    #[sqlx::test]
    async fn label_values_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(&format!("/labels/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
