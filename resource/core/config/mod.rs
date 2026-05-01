use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub database_url_admin: Option<String>,
    pub strata_app_password: Option<String>,
    pub host: String,
    pub port: u16,
    pub nucleus_secret_key: Option<String>,
    pub nucleus_base_url: Option<String>,
    pub resend_api_key: Option<String>,
    pub alert_from_email: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://strata:secret@localhost:5432/strata".to_string()),
            database_url_admin: env::var("DATABASE_URL_ADMIN").ok(),
            strata_app_password: env::var("STRATA_APP_PASSWORD").ok(),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            nucleus_secret_key: env::var("NUCLEUS_SECRET_KEY").ok(),
            nucleus_base_url: env::var("NUCLEUS_BASE_URL").ok(),
            resend_api_key: env::var("RESEND_API_KEY").ok(),
            alert_from_email: env::var("ALERT_FROM_EMAIL")
                .unwrap_or_else(|_| "alerts@strata.dev".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests in this module mutate process-global env vars. Cargo test runs them in
    // parallel, which would race. Serialize via a shared mutex (poisoned-mutex
    // tolerated — failed test will already be reported).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // DATABASE_URL must not be removed because sqlx::test depends on it.
    #[test]
    fn from_env_reads_and_defaults() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // 1. DATABASE_URL is always set (from .env or env)
        let config = AppConfig::from_env();
        assert!(!config.database_url.is_empty());

        // 2. Custom HOST/PORT
        env::set_var("HOST", "127.0.0.1");
        env::set_var("PORT", "8080");
        let config = AppConfig::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);

        // 3. Invalid PORT falls back to default
        env::set_var("PORT", "not_a_number");
        let config = AppConfig::from_env();
        assert_eq!(config.port, 3000);

        // 4. Defaults when HOST/PORT unset
        env::remove_var("HOST");
        env::remove_var("PORT");
        let config = AppConfig::from_env();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);

        // 5. Nucleus/Chorus fields default to None/default
        assert!(config.nucleus_secret_key.is_none());
        assert!(config.nucleus_base_url.is_none());
        assert!(config.resend_api_key.is_none());
        assert_eq!(config.alert_from_email, "alerts@strata.dev");

        // 6. Nucleus/Chorus fields read from env
        env::set_var("NUCLEUS_SECRET_KEY", "sk_test");
        env::set_var("NUCLEUS_BASE_URL", "https://nucleus.test");
        env::set_var("RESEND_API_KEY", "re_test");
        env::set_var("ALERT_FROM_EMAIL", "custom@test.com");
        let config = AppConfig::from_env();
        assert_eq!(config.nucleus_secret_key.as_deref(), Some("sk_test"));
        assert_eq!(
            config.nucleus_base_url.as_deref(),
            Some("https://nucleus.test")
        );
        assert_eq!(config.resend_api_key.as_deref(), Some("re_test"));
        assert_eq!(config.alert_from_email, "custom@test.com");

        // Cleanup
        env::remove_var("NUCLEUS_SECRET_KEY");
        env::remove_var("NUCLEUS_BASE_URL");
        env::remove_var("RESEND_API_KEY");
        env::remove_var("ALERT_FROM_EMAIL");
    }

    #[test]
    fn clone_works() {
        let config = AppConfig {
            database_url: "url".into(),
            database_url_admin: Some("admin_url".into()),
            strata_app_password: Some("pw".into()),
            host: "host".into(),
            port: 1234,
            nucleus_secret_key: Some("sk_test".into()),
            nucleus_base_url: None,
            resend_api_key: Some("re_test".into()),
            alert_from_email: "alerts@test.com".into(),
        };
        let cloned = config.clone();
        assert_eq!(cloned.database_url, "url");
        assert_eq!(cloned.database_url_admin.as_deref(), Some("admin_url"));
        assert_eq!(cloned.strata_app_password.as_deref(), Some("pw"));
        assert_eq!(cloned.host, "host");
        assert_eq!(cloned.port, 1234);
        assert_eq!(cloned.nucleus_secret_key.as_deref(), Some("sk_test"));
        assert!(cloned.nucleus_base_url.is_none());
        assert_eq!(cloned.resend_api_key.as_deref(), Some("re_test"));
        assert_eq!(cloned.alert_from_email, "alerts@test.com");
    }

    // NOTE: These tests intentionally do NOT mutate DATABASE_URL. Other tests in
    // this binary (datasource::postgresql, sqlx::test) depend on DATABASE_URL
    // pointing at a real Postgres, and `cargo test` runs all tests in one process,
    // so any change here would race them. The spec's `set_var("DATABASE_URL", "postgres://x")`
    // line is omitted for that reason — DATABASE_URL is always set in the test env
    // (via .env, sqlx::test, or the command line) so `from_env` won't panic.
    #[test]
    fn from_env_reads_optional_admin_and_password() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("DATABASE_URL_ADMIN", "postgres://admin");
        std::env::set_var("STRATA_APP_PASSWORD", "s3cret");
        std::env::set_var("ALERT_FROM_EMAIL", "a@b");
        let cfg = AppConfig::from_env();
        assert_eq!(cfg.database_url_admin.as_deref(), Some("postgres://admin"));
        assert_eq!(cfg.strata_app_password.as_deref(), Some("s3cret"));
        std::env::remove_var("DATABASE_URL_ADMIN");
        std::env::remove_var("STRATA_APP_PASSWORD");
        std::env::remove_var("ALERT_FROM_EMAIL");
    }

    #[test]
    fn from_env_admin_and_password_default_to_none() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("DATABASE_URL_ADMIN");
        std::env::remove_var("STRATA_APP_PASSWORD");
        std::env::set_var("ALERT_FROM_EMAIL", "a@b");
        let cfg = AppConfig::from_env();
        assert!(cfg.database_url_admin.is_none());
        assert!(cfg.strata_app_password.is_none());
        // Cleanup so the shared `from_env_reads_and_defaults` test sees a clean slate
        // when it runs after this one.
        std::env::remove_var("ALERT_FROM_EMAIL");
    }
}
