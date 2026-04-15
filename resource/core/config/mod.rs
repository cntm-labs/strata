use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
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

    // All env var mutation consolidated into one test to avoid parallel race conditions.
    // DATABASE_URL must not be removed because sqlx::test depends on it.
    #[test]
    fn from_env_reads_and_defaults() {
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
            host: "host".into(),
            port: 1234,
            nucleus_secret_key: Some("sk_test".into()),
            nucleus_base_url: None,
            resend_api_key: Some("re_test".into()),
            alert_from_email: "alerts@test.com".into(),
        };
        let cloned = config.clone();
        assert_eq!(cloned.database_url, "url");
        assert_eq!(cloned.host, "host");
        assert_eq!(cloned.port, 1234);
        assert_eq!(cloned.nucleus_secret_key.as_deref(), Some("sk_test"));
        assert!(cloned.nucleus_base_url.is_none());
        assert_eq!(cloned.resend_api_key.as_deref(), Some("re_test"));
        assert_eq!(cloned.alert_from_email, "alerts@test.com");
    }
}
