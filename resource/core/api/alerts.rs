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
        .route("/rules/{id}/test", axum::routing::post(test_fire_rule))
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

async fn test_fire_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AlertEvent>> {
    let rule = sqlx::query_as::<_, AlertRule>("SELECT * FROM alert_rules WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("Alert rule not found".into()))?;

    let ds = sqlx::query_as::<_, super::datasources::Datasource>(
        "SELECT * FROM datasources WHERE id = $1",
    )
    .bind(rule.datasource_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("Datasource not found".into()))?;

    // Execute query against datasource
    let value = match ds.ds_type.as_str() {
        "prometheus" => {
            let client = crate::datasource::prometheus::PrometheusClient::new(&ds.url);
            let resp = client.query(&rule.query, None, None).await?;
            resp.data
                .get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|m| m.get("value"))
                .and_then(|v| v.as_array())
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
        }
        _ => None,
    };

    let val = value.unwrap_or(0.0);
    let firing = match rule.condition.as_str() {
        "gt" => val > rule.threshold,
        "gte" => val >= rule.threshold,
        "lt" => val < rule.threshold,
        "lte" => val <= rule.threshold,
        "eq" => (val - rule.threshold).abs() < f64::EPSILON,
        _ => false,
    };

    let state_str = if firing { "firing" } else { "ok" };
    let event = sqlx::query_as::<_, AlertEvent>(
        "INSERT INTO alert_events (rule_id, state, value, message)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(rule.id)
    .bind(state_str)
    .bind(val)
    .bind(format!(
        "Test fire: {} = {:.2}, threshold {:.2} ({})",
        rule.query, val, rule.threshold, rule.condition
    ))
    .fetch_one(&state.db)
    .await?;

    Ok(Json(event))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app(db: sqlx::PgPool) -> axum::Router {
        let state = crate::AppState {
            db,
            config: crate::config::AppConfig {
                database_url: String::new(),
                host: "127.0.0.1".into(),
                port: 3000,
            },
        };
        alert_routes().with_state(state)
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

    async fn seed_datasource(pool: &sqlx::PgPool) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ('Test', 'prometheus', 'http://prom:9090') RETURNING id"
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn create_rule_helper(pool: &sqlx::PgPool, ds_id: Uuid, name: &str) -> AlertRule {
        let app = test_app(pool.clone());
        let resp = app
            .oneshot(json_request(
                "POST",
                "/rules",
                serde_json::json!({
                    "name": name,
                    "datasource_id": ds_id,
                    "query": "up == 0",
                    "condition": "gt",
                    "threshold": 0.5,
                    "notification_channels": ["sms"],
                    "notification_recipients": ["+66123456789"]
                }),
            ))
            .await
            .unwrap();
        body_json(resp).await
    }

    #[sqlx::test]
    async fn list_rules_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/rules").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertRule> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn create_and_get_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule_helper(&pool, ds_id, "CPU Alert").await;
        assert_eq!(created.name, "CPU Alert");
        assert_eq!(created.condition, "gt");
        assert!(created.is_active);
        assert_eq!(created.severity, "warning");
        assert_eq!(created.duration_secs, 60);

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(format!("/rules/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: AlertRule = body_json(resp).await;
        assert_eq!(fetched.id, created.id);
    }

    #[sqlx::test]
    async fn get_rule_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(
                Request::get(format!("/rules/{}", fake_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn update_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule_helper(&pool, ds_id, "Original").await;

        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/rules/{}", created.id),
                serde_json::json!({
                    "name": "Updated",
                    "severity": "critical",
                    "is_active": false
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let updated: AlertRule = body_json(resp).await;
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.severity, "critical");
        assert!(!updated.is_active);
    }

    #[sqlx::test]
    async fn update_rule_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let fake_id = Uuid::new_v4();
        let resp = app
            .oneshot(json_request(
                "PUT",
                &format!("/rules/{}", fake_id),
                serde_json::json!({"name": "x"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn delete_rule(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let created = create_rule_helper(&pool, ds_id, "ToDelete").await;

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::delete(format!("/rules/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::get(format!("/rules/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn list_events_empty(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/events").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertEvent> = body_json(resp).await;
        assert!(items.is_empty());
    }

    #[sqlx::test]
    async fn list_events_with_filter(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let rule = create_rule_helper(&pool, ds_id, "Rule1").await;

        sqlx::query(
            "INSERT INTO alert_events (rule_id, state, value, message) VALUES ($1, $2, $3, $4)",
        )
        .bind(rule.id)
        .bind("firing")
        .bind(1.5_f64)
        .bind("CPU high")
        .execute(&pool)
        .await
        .unwrap();

        let app = test_app(pool.clone());
        let resp = app
            .oneshot(
                Request::get(format!("/events?rule_id={}&limit=10", rule.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let items: Vec<AlertEvent> = body_json(resp).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].state, "firing");

        let app = test_app(pool);
        let resp = app
            .oneshot(Request::get("/events").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let all_items: Vec<AlertEvent> = body_json(resp).await;
        assert_eq!(all_items.len(), 1);
    }

    #[sqlx::test]
    async fn test_fire_rule_creates_event(pool: sqlx::PgPool) {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v1/query"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "status": "success",
                    "data": {"resultType": "vector", "result": [
                        {"metric": {"__name__": "up"}, "value": [1700000000, "1.0"]}
                    ]}
                })),
            )
            .mount(&mock)
            .await;

        let ds_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO datasources (name, type, url) VALUES ('Prom', 'prometheus', $1) RETURNING id",
        )
        .bind(mock.uri())
        .fetch_one(&pool)
        .await
        .unwrap();

        let rule = {
            let app = test_app(pool.clone());
            let resp = app
                .oneshot(json_request(
                    "POST",
                    "/rules",
                    serde_json::json!({
                        "name": "Test Fire",
                        "datasource_id": ds_id,
                        "query": "up",
                        "condition": "gt",
                        "threshold": 0.5,
                        "notification_channels": ["sms"],
                        "notification_recipients": ["+66123456789"]
                    }),
                ))
                .await
                .unwrap();
            body_json::<AlertRule>(resp).await
        };

        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/rules/{}/test", rule.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let event: AlertEvent = body_json(resp).await;
        assert_eq!(event.rule_id, rule.id);
        assert_eq!(event.state, "firing");
        assert!(event.value.unwrap() > 0.0);
    }

    #[sqlx::test]
    async fn test_fire_rule_not_found(pool: sqlx::PgPool) {
        let app = test_app(pool);
        let resp = app
            .oneshot(
                Request::post(format!("/rules/{}/test", Uuid::new_v4()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test]
    async fn create_rule_with_custom_severity_and_duration(pool: sqlx::PgPool) {
        let ds_id = seed_datasource(&pool).await;
        let app = test_app(pool);
        let resp = app
            .oneshot(json_request(
                "POST",
                "/rules",
                serde_json::json!({
                    "name": "Custom",
                    "datasource_id": ds_id,
                    "query": "up",
                    "condition": "lt",
                    "threshold": 1.0,
                    "duration_secs": 300,
                    "severity": "critical",
                    "notification_channels": ["email"],
                    "notification_recipients": ["admin@test.com"]
                }),
            ))
            .await
            .unwrap();
        let created: AlertRule = body_json(resp).await;
        assert_eq!(created.duration_secs, 300);
        assert_eq!(created.severity, "critical");
    }
}
