use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NucleusClaims {
    pub sub: String,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar_url: Option<String>,
    pub exp: u64,
    pub iat: u64,
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let secret_key = state
        .config
        .nucleus_api_key
        .as_deref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.set_required_spec_claims(&["sub", "exp", "iat"]);

    let token_data = decode::<NucleusClaims>(
        token,
        &DecodingKey::from_secret(secret_key.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        tracing::debug!("JWT validation failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    req.extensions_mut().insert(token_data.claims);

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use tower::ServiceExt;

    fn make_test_token(secret: &str) -> String {
        let claims = NucleusClaims {
            sub: "user-123".into(),
            email: Some("test@test.com".into()),
            first_name: Some("Test".into()),
            last_name: Some("User".into()),
            avatar_url: None,
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as u64,
            iat: chrono::Utc::now().timestamp() as u64,
        };
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    fn make_expired_token(secret: &str) -> String {
        let claims = NucleusClaims {
            sub: "user-123".into(),
            email: Some("test@test.com".into()),
            first_name: None,
            last_name: None,
            avatar_url: None,
            exp: 1000,
            iat: 999,
        };
        encode(
            &Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[sqlx::test]
    async fn valid_token_passes_through(pool: sqlx::PgPool) {
        let secret = "test-secret-key";
        let state = crate::AppState {
            pool,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: Some(secret.into()),
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app = crate::build_router(state);

        let token = make_test_token(secret);
        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Should pass auth — dashboards returns 200 (empty list)
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test]
    async fn expired_token_returns_401(pool: sqlx::PgPool) {
        let secret = "test-secret-key";
        let state = crate::AppState {
            pool,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: Some(secret.into()),
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app = crate::build_router(state);

        let token = make_expired_token(secret);
        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn wrong_secret_returns_401(pool: sqlx::PgPool) {
        let state = crate::AppState {
            pool,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: Some("correct-secret".into()),
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app = crate::build_router(state);

        let token = make_test_token("wrong-secret");
        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test]
    async fn missing_bearer_prefix_returns_401(pool: sqlx::PgPool) {
        let state = crate::AppState {
            pool,
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_api_key: Some("secret".into()),
                nucleus_jwks_cache_ttl_secs: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app = crate::build_router(state);

        let resp = app
            .oneshot(
                Request::get("/api/v1/dashboards")
                    .header("Authorization", "Token some-token")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
