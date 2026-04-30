use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::AppConfig;

/// Build a runtime PgPool, ensuring migrations have been applied and the
/// `strata_app` role is loginable with the configured password.
///
/// In dev (no `DATABASE_URL_ADMIN`, no `STRATA_APP_PASSWORD`) this collapses
/// to the original single-pool behavior: connect to `DATABASE_URL`, run
/// migrations, return the pool.
///
/// In prod, opens an admin pool from `DATABASE_URL_ADMIN`, runs migrations,
/// runs `ALTER ROLE strata_app WITH LOGIN PASSWORD '...'` (with the password
/// validated and escaped — see `escape_pg_password`), closes the admin pool,
/// and returns a fresh runtime pool from `DATABASE_URL` (which connects as
/// `strata_app`).
pub async fn bootstrap_db(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let admin_url = config
        .database_url_admin
        .as_deref()
        .unwrap_or(&config.database_url);

    let admin_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(admin_url)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&admin_pool)
        .await
        .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;

    if let Some(password) = config.strata_app_password.as_deref() {
        let escaped = escape_pg_password(password)?;
        // PostgreSQL DDL (ALTER ROLE) cannot be parameterized, so we
        // interpolate. The escape function rejects characters that could
        // break the literal and the only remaining hazard ('\'' single quote)
        // is doubled to '' per Postgres SQL literal escaping rules.
        let sql = format!("ALTER ROLE strata_app WITH LOGIN PASSWORD '{}'", escaped);
        sqlx::query(&sql).execute(&admin_pool).await?;
    }

    admin_pool.close().await;

    PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
}

/// Escape a password for safe literal interpolation in PostgreSQL DDL.
///
/// Rejects NUL and backslash since they could subvert the literal even with
/// quote-doubling. Doubles single quotes per Postgres SQL standard.
///
/// PostgreSQL DDL statements (ALTER ROLE, CREATE TABLE, etc.) are utility
/// commands — they cannot be prepared, so `bind()` is unavailable and
/// the password value must be embedded as a literal in the SQL text.
fn escape_pg_password(password: &str) -> Result<String, sqlx::Error> {
    if password.is_empty() {
        return Err(sqlx::Error::Configuration(
            "STRATA_APP_PASSWORD must not be empty".into(),
        ));
    }
    if password.contains('\0') || password.contains('\\') {
        return Err(sqlx::Error::Configuration(
            "STRATA_APP_PASSWORD must not contain NUL or backslash".into(),
        ));
    }
    Ok(password.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(url: &str) -> AppConfig {
        AppConfig {
            database_url: url.to_string(),
            database_url_admin: None,
            strata_app_password: None,
            host: "127.0.0.1".into(),
            port: 3000,
            nucleus_secret_key: None,
            nucleus_base_url: None,
            resend_api_key: None,
            alert_from_email: "test@test.com".into(),
        }
    }

    #[test]
    fn escape_doubles_single_quotes() {
        assert_eq!(escape_pg_password("a'b").unwrap(), "a''b");
    }

    #[test]
    fn escape_passes_through_normal_chars() {
        assert_eq!(escape_pg_password("aBc123!@#").unwrap(), "aBc123!@#");
    }

    #[test]
    fn escape_rejects_empty() {
        assert!(escape_pg_password("").is_err());
    }

    #[test]
    fn escape_rejects_nul_byte() {
        assert!(escape_pg_password("a\0b").is_err());
    }

    #[test]
    fn escape_rejects_backslash() {
        assert!(escape_pg_password("a\\b").is_err());
    }

    // Note: this test reuses the live DATABASE_URL rather than `sqlx::test`
    // because `sqlx::test` owns the connection URL it picks. Migrations are
    // idempotent (PR #13 made them so).
    #[tokio::test]
    async fn dev_path_returns_a_working_pool_when_password_unset() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let cfg = test_config(&url);
        let pool = bootstrap_db(&cfg)
            .await
            .expect("bootstrap_db should succeed");
        let one: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(one.0, 1);
    }

    #[tokio::test]
    async fn prod_path_alters_strata_app_password() {
        let admin_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set as the migration role for tests");
        let mut cfg = test_config(&admin_url);
        cfg.database_url_admin = Some(admin_url.clone());
        // Use a password with a single quote to exercise the escape path.
        cfg.strata_app_password = Some("test_pwd_xyz'123".into());
        let _pool = bootstrap_db(&cfg)
            .await
            .expect("bootstrap_db should succeed");

        // Verify the role was altered: should now have rolcanlogin = true and
        // rolpassword IS NOT NULL.
        let admin = sqlx::PgPool::connect(&admin_url).await.unwrap();
        let row: (bool, bool) = sqlx::query_as(
            "SELECT pg_roles.rolcanlogin, pg_authid.rolpassword IS NOT NULL \
             FROM pg_roles LEFT JOIN pg_authid USING (oid) \
             WHERE pg_roles.rolname = 'strata_app'",
        )
        .fetch_one(&admin)
        .await
        .unwrap();
        assert!(row.0, "strata_app must be loginable");
        assert!(row.1, "strata_app must have a password set");
    }
}
