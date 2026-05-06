use std::ops::{Deref, DerefMut};

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Copy)]
pub struct TenantId(pub Uuid);

pub struct TenantTx {
    tenant_id: Uuid,
    tx: Transaction<'static, Postgres>,
}

impl TenantTx {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }
}

impl Deref for TenantTx {
    type Target = PgConnection;
    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl DerefMut for TenantTx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

pub enum TenantTxError {
    MissingTenant,
    Sqlx(sqlx::Error),
}

impl IntoResponse for TenantTxError {
    fn into_response(self) -> Response {
        match self {
            TenantTxError::MissingTenant => {
                tracing::error!("TenantTx extracted on a route without TenantId middleware");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            TenantTxError::Sqlx(e) => {
                tracing::error!("TenantTx db error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl FromRequestParts<AppState> for TenantTx {
    type Rejection = TenantTxError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let tenant = parts
            .extensions
            .get::<TenantId>()
            .copied()
            .ok_or(TenantTxError::MissingTenant)?;

        let mut tx = state.pool.begin().await.map_err(TenantTxError::Sqlx)?;

        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant.0.to_string())
            .execute(&mut *tx)
            .await
            .map_err(TenantTxError::Sqlx)?;

        Ok(Self {
            tenant_id: tenant.0,
            tx,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    // Shared dummy handler for the negative extractor tests. Returns 200 only
    // if the extractor succeeds; both tests below expect the extractor to fail
    // before this body runs, so reaching it would itself be the failure mode.
    async fn extractor_probe_handler(_tx: TenantTx) -> &'static str {
        "ok"
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn extractor_returns_500_when_tenant_extension_missing(pool: sqlx::PgPool) {
        // Mount a route that uses TenantTx but does NOT install the mock-tenant
        // middleware. The extractor must observe the missing TenantId and reject
        // the request with 500.
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
                nucleus_secret_key: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app: Router = Router::new()
            .route("/x", get(extractor_probe_handler))
            .with_state(state);
        let resp = app
            .oneshot(Request::get("/x").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn missing_tenant_error_renders_500() {
        let resp = TenantTxError::MissingTenant.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn sqlx_error_renders_500() {
        let resp = TenantTxError::Sqlx(sqlx::Error::PoolTimedOut).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deref_exposes_inner_connection(pool: sqlx::PgPool) {
        // The Deref impl is dead at runtime (handlers always use &mut *tx),
        // but it's a required supertrait of DerefMut. Exercise it once so the
        // coverage tool sees a call site.
        let tx = pool.begin().await.unwrap();
        let tenant_tx = TenantTx {
            tenant_id: Uuid::nil(),
            tx,
        };
        // Auto-deref coercion at the type ascription invokes Deref::deref.
        let _: &PgConnection = &tenant_tx;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn extractor_succeeds_with_mock_tenant(pool: sqlx::PgPool) {
        // Positive path through the extractor — also covers the probe handler
        // body so coverage on it isn't a phantom.
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
                nucleus_secret_key: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app: Router = Router::new()
            .route("/x", get(extractor_probe_handler))
            .layer(axum::middleware::from_fn(
                crate::middleware::tenant::inject_mock_tenant,
            ))
            .with_state(state);
        let resp = app
            .oneshot(Request::get("/x").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn extractor_returns_500_when_pool_is_closed(pool: sqlx::PgPool) {
        // Close the pool BEFORE the extractor runs so pool.begin() fails and
        // the Sqlx error path of from_request_parts is exercised.
        let state = crate::AppState {
            pool: pool.clone(),
            config: crate::config::AppConfig {
                database_url: String::new(),
                database_url_admin: None,
                strata_app_password: None,
                sentry_dsn: None,
                strata_env: None,
                host: "127.0.0.1".into(),
                port: 3000,
                nucleus_secret_key: None,
                nucleus_base_url: None,
                resend_api_key: None,
                alert_from_email: "test@test.com".into(),
            },
            notifier: std::sync::Arc::new(crate::notifier::Notifier::new(None, "test@test.com")),
        };
        let app: Router = Router::new()
            .route("/x", get(extractor_probe_handler))
            .layer(axum::middleware::from_fn(
                crate::middleware::tenant::inject_mock_tenant,
            ))
            .with_state(state);
        pool.close().await;
        let resp = app
            .oneshot(Request::get("/x").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn set_config_uses_parameter_binding(pool: sqlx::PgPool) {
        let suspicious = "00000000-0000-0000-0000-000000000000', false); DROP TABLE tenants; --";

        let mut tx = pool.begin().await.unwrap();
        let res = sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(suspicious)
            .execute(&mut *tx)
            .await;

        assert!(res.is_ok());
        let still_exists: (bool,) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
             WHERE table_name = 'tenants')",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap();
        assert!(still_exists.0, "tenants table must still exist after bind");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn set_local_does_not_leak_across_transactions(pool: sqlx::PgPool) {
        let a = Uuid::new_v4();
        let mut tx_a = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO tenants (id, name, slug) VALUES ($1, $2, $3)")
            .bind(a)
            .bind("A")
            .bind(format!("a-{}", a))
            .execute(&mut *tx_a)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(a.to_string())
            .execute(&mut *tx_a)
            .await
            .unwrap();
        let seen_a: (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&mut *tx_a)
            .await
            .unwrap();
        assert_eq!(seen_a.0, a.to_string());
        tx_a.commit().await.unwrap();

        let mut tx_b = pool.begin().await.unwrap();
        let seen_b: (Option<String>,) =
            sqlx::query_as("SELECT NULLIF(current_setting('app.tenant_id', true), '')")
                .fetch_one(&mut *tx_b)
                .await
                .unwrap();
        assert!(
            seen_b.0.is_none(),
            "expected app.tenant_id unset in a fresh tx, got {:?}",
            seen_b.0
        );
    }
}
