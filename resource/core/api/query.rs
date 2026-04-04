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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        // Mount via datasource routes since query is nested under /{id}/query
        crate::api::datasources::datasource_routes().with_state(crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        })
    }

    fn json_request(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn seed_ds(pool: &sqlx::PgPool, ds_type: &str, url: &str) -> Uuid {
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
    async fn proxy_prometheus_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {"resultType": "vector", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "prometheus", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "up"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_prometheus_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "prometheus", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "up", "start": "1000", "end": "2000", "step": "15"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_loki_instant(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "loki", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "{job=\"app\"}"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_loki_range(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success", "data": {}
            })))
            .mount(&mock)
            .await;

        let ds_id = seed_ds(&pool, "loki", &mock.uri()).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "{job=\"app\"}", "start": "1000", "end": "2000"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn proxy_unsupported_type(pool: sqlx::PgPool) {
        let ds_id = seed_ds(&pool, "redis", "http://redis:6379").await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "KEYS *"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[sqlx::test]
    async fn proxy_datasource_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", Uuid::new_v4()),
                serde_json::json!({
                    "query": "up"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
