use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::TenantTx;
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

async fn list(mut tx: TenantTx) -> AppResult<Json<Vec<Datasource>>> {
    let rows =
        sqlx::query_as::<_, Datasource>("SELECT * FROM datasources ORDER BY created_at DESC")
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn create(
    mut tx: TenantTx,
    Json(input): Json<CreateDatasource>,
) -> AppResult<Json<Datasource>> {
    let tenant_id = tx.tenant_id();
    let row = sqlx::query_as::<_, Datasource>(
        "INSERT INTO datasources (tenant_id, name, type, url, credentials_enc, is_default)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(tenant_id)
    .bind(&input.name)
    .bind(&input.ds_type)
    .bind(&input.url)
    .bind(&input.credentials)
    .bind(input.is_default.unwrap_or(false))
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn get_one(mut tx: TenantTx, Path(id): Path<Uuid>) -> AppResult<Json<Datasource>> {
    let row = sqlx::query_as::<_, Datasource>("SELECT * FROM datasources WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn update(
    mut tx: TenantTx,
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
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn remove(mut tx: TenantTx, Path(id): Path<Uuid>) -> AppResult<Json<()>> {
    sqlx::query("DELETE FROM datasources WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(()))
}

async fn test_connection(
    mut tx: TenantTx,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let ds = sqlx::query_as::<_, Datasource>("SELECT * FROM datasources WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("Datasource not found".into()))?;
    tx.commit().await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            pool: db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: None,
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        datasource_routes()
            .layer(axum::middleware::from_fn(
                crate::middleware::tenant::inject_mock_tenant,
            ))
            .with_state(state)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(resp: axum::response::Response) -> T {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn json_request(method_str: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method_str)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    }

    async fn create_ds(pool: &sqlx::PgPool, name: &str, ds_type: &str, url: &str) -> Datasource {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/",
                serde_json::json!({
                    "name": name, "type": ds_type, "url": url
                }),
            ))
            .await
            .unwrap();
        body_json(resp).await
    }

    #[sqlx::test]
    async fn list_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<Datasource> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "Prom", "prometheus", "http://prom:9090").await;
        assert_eq!(created.name, "Prom");
        assert_eq!(created.ds_type, "prometheus");

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(format!("/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: Datasource = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(
                Request::get(format!("/{}", fake_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_datasource(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "Old", "loki", "http://loki:3100").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/{}", created.id),
                serde_json::json!({
                    "name": "New Name"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: Datasource = body_json(resp).await;
        assert_eq!(updated.name, "New Name");
    }

    #[sqlx::test]
    async fn update_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/{}", fake_id),
                serde_json::json!({"name": "x"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_datasource(pool: sqlx::PgPool) {
        let created = create_ds(&pool, "ToDelete", "prometheus", "http://x:9090").await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::delete(format!("/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(format!("/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn create_with_default_flag(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request("POST", "/", serde_json::json!({
                "name": "Default", "type": "prometheus", "url": "http://prom:9090", "is_default": true
            })))
            .await.unwrap();
        let created: Datasource = body_json(resp).await;
        assert_eq!(created.is_default, Some(true));
    }

    #[sqlx::test]
    async fn test_connection_prometheus(pool: sqlx::PgPool) {
        let mock_prom = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/-/healthy"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_prom)
            .await;

        let ds = create_ds(&pool, "Prom", "prometheus", &mock_prom.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", ds.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], true);
    }

    #[sqlx::test]
    async fn test_connection_loki(pool: sqlx::PgPool) {
        let mock_loki = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ready"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_loki)
            .await;

        let ds = create_ds(&pool, "Loki", "loki", &mock_loki.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", ds.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], true);
    }

    #[sqlx::test]
    async fn test_connection_unknown_type(pool: sqlx::PgPool) {
        let ds = create_ds(&pool, "Unknown", "redis", "http://redis:6379").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", ds.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], false);
    }

    #[sqlx::test]
    async fn test_connection_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", fake_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn test_connection_postgresql(pool: sqlx::PgPool) {
        // Use real DATABASE_URL so PgPool::connect succeeds
        dotenvy::dotenv().ok();
        let pg_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        let ds = create_ds(&pool, "PG", "postgresql", &pg_url).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", ds.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], true);
    }

    #[sqlx::test]
    async fn datasources_isolated_by_tenant(pool: sqlx::PgPool) {
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
        sqlx::query(
            "INSERT INTO datasources (name, type, url, tenant_id) \
             VALUES ('promA','prometheus','http://a',$1), \
                    ('promB','prometheus','http://b',$2)",
        )
        .bind(a)
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();

        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL ROLE strata_app")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(a.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        let names: Vec<String> = sqlx::query_scalar("SELECT name FROM datasources ORDER BY name")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        assert_eq!(names, vec!["promA".to_string()]);
    }

    #[sqlx::test]
    async fn test_connection_prometheus_unhealthy(pool: sqlx::PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/-/healthy"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock)
            .await;

        let ds = create_ds(&pool, "Prom", "prometheus", &mock.uri()).await;

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/{}/test", ds.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let result: serde_json::Value = body_json(resp).await;
        assert_eq!(result["success"], false);
    }
}
