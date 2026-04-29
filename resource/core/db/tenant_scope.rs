use std::ops::{Deref, DerefMut};

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::{Postgres, Transaction};
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
    type Target = Transaction<'static, Postgres>;
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
