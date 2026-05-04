use std::time::Instant;

use axum::{extract::Path, Json};
use metrics::{counter, histogram};
use serde::Deserialize;
use uuid::Uuid;

use crate::datasource::{loki::LokiClient, postgresql, prometheus::PrometheusClient};
use crate::db::TenantTx;
use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub step: Option<String>,
    pub limit: Option<u32>,
}

pub async fn proxy_query(
    mut tx: TenantTx,
    Path(id): Path<Uuid>,
    Json(input): Json<QueryRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    // Commit the (read-only) transaction before doing the network proxy so we
    // don't hold a Postgres connection for the duration of the outbound call.
    tx.commit().await?;

    let datasource_type = ds.ds_type.clone();
    let start = Instant::now();
    let dispatch_result: Result<serde_json::Value, AppError> = async {
        match ds.ds_type.as_str() {
            "prometheus" => {
                let client = PrometheusClient::new(&ds.url);
                if let (Some(start), Some(end), Some(step)) =
                    (&input.start, &input.end, &input.step)
                {
                    let resp = client.query_range(&input.query, start, end, step).await?;
                    Ok(serde_json::to_value(resp)?)
                } else {
                    let resp = client.query(&input.query, None, None).await?;
                    Ok(serde_json::to_value(resp)?)
                }
            }
            "loki" => {
                let client = LokiClient::new(&ds.url);
                if let (Some(start), Some(end)) = (&input.start, &input.end) {
                    let resp = client
                        .query_range(&input.query, start, end, input.limit)
                        .await?;
                    Ok(serde_json::to_value(resp)?)
                } else {
                    let resp = client.query(&input.query, input.limit).await?;
                    Ok(serde_json::to_value(resp)?)
                }
            }
            "postgresql" => {
                let rows = postgresql::execute_query(&ds.url, &input.query).await?;
                Ok(serde_json::to_value(rows)?)
            }
            other => Err(AppError::BadRequest(format!(
                "Unsupported datasource type: {}",
                other
            ))),
        }
    }
    .await;
    let elapsed = start.elapsed().as_secs_f64();

    let status = if dispatch_result.is_ok() {
        "success"
    } else {
        "error"
    };
    counter!(
        "strata_query_proxy_total",
        "datasource_type" => datasource_type.clone(),
        "status" => status,
    )
    .increment(1);
    histogram!(
        "strata_query_proxy_duration_seconds",
        "datasource_type" => datasource_type,
    )
    .record(elapsed);

    Ok(Json(dispatch_result?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const MOCK_TENANT: Uuid = Uuid::from_u128(0);

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        // Mount via datasource routes since query is nested under /{id}/query
        crate::api::datasources::datasource_routes()
            .layer(axum::middleware::from_fn(
                crate::middleware::tenant::inject_mock_tenant,
            ))
            .with_state(crate::AppState {
                pool: db,
                config: crate::config::AppConfig {
                    database_url: String::new(),
                    database_url_admin: None,
                    strata_app_password: None,
                    host: "127.0.0.1".into(),
                    port: 3000,
                    nucleus_secret_key: None,
                    nucleus_base_url: None,
                    resend_api_key: None,
                    alert_from_email: "test@test.com".into(),
                },
                notifier: std::sync::Arc::new(crate::notifier::Notifier::new(
                    None,
                    "test@test.com",
                )),
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
            "INSERT INTO datasources (name, type, url, tenant_id) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(format!("Test {}", ds_type))
        .bind(ds_type)
        .bind(url)
        .bind(MOCK_TENANT)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[sqlx::test]
    async fn proxy_records_metric_on_success(pool: sqlx::PgPool) {
        crate::metrics::install(&pool);
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
                serde_json::json!({"query": "up"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let rendered = crate::metrics::render();
        assert!(
            rendered.contains("strata_query_proxy_total")
                && rendered.contains(r#"datasource_type="prometheus""#)
                && rendered.contains(r#"status="success""#),
            "expected success counter; got:\n{rendered}"
        );
    }

    #[sqlx::test]
    async fn proxy_records_metric_on_failure(pool: sqlx::PgPool) {
        crate::metrics::install(&pool);
        // Unreachable URL forces dispatch error.
        let ds_id = seed_ds(&pool, "prometheus", "http://127.0.0.1:1").await;
        let app = test_app(pool);
        let _ = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({"query": "up"}),
            ))
            .await
            .unwrap();
        let rendered = crate::metrics::render();
        assert!(
            rendered.contains("strata_query_proxy_total")
                && rendered.contains(r#"datasource_type="prometheus""#)
                && rendered.contains(r#"status="error""#),
            "expected error counter; got:\n{rendered}"
        );
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
    async fn proxy_postgresql(pool: sqlx::PgPool) {
        dotenvy::dotenv().ok();
        let pg_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        let ds_id = seed_ds(&pool, "postgresql", &pg_url).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                &format!("/{}/query", ds_id),
                serde_json::json!({
                    "query": "SELECT 1 AS value"
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
    async fn proxy_query_404s_for_other_tenant_datasource(pool: sqlx::PgPool) {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1,'A',$2),($3,'B',$4)")
            .bind(a)
            .bind(format!("a-{}", a))
            .bind(b)
            .bind(format!("b-{}", b))
            .execute(&pool)
            .await
            .unwrap();
        let ds_a: Uuid = sqlx::query_scalar(
            "INSERT INTO datasources (name, type, url, tenant_id) \
             VALUES ('p','prometheus','http://x',$1) RETURNING id",
        )
        .bind(a)
        .fetch_one(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL ROLE strata_app")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(b.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM datasources WHERE id = $1")
            .bind(ds_a)
            .fetch_optional(&mut *tx)
            .await
            .unwrap();
        assert!(row.is_none(), "tenant B must not see tenant A's datasource");
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
