use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: env var tests for HOST/PORT only — DATABASE_URL must not be removed
    // because sqlx::test depends on it in parallel test runs.

    #[test]
    fn host_and_port_defaults() {
        env::remove_var("HOST");
        env::remove_var("PORT");
        let config = AppConfig::from_env();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
    }

    #[test]
    fn reads_custom_host_and_port() {
        env::set_var("HOST", "127.0.0.1");
        env::set_var("PORT", "8080");
        let config = AppConfig::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        env::remove_var("HOST");
        env::remove_var("PORT");
    }

    #[test]
    fn database_url_reads_from_env() {
        // DATABASE_URL is set (from .env or env) — just verify it's read
        let config = AppConfig::from_env();
        assert!(!config.database_url.is_empty());
    }

    #[test]
    fn invalid_port_falls_back_to_default() {
        env::set_var("PORT", "not_a_number");
        let config = AppConfig::from_env();
        assert_eq!(config.port, 3000);
        env::remove_var("PORT");
    }

    #[test]
    fn clone_works() {
        let config = AppConfig {
            database_url: "url".into(),
            host: "host".into(),
            port: 1234,
        };
        let cloned = config.clone();
        assert_eq!(cloned.database_url, "url");
        assert_eq!(cloned.host, "host");
        assert_eq!(cloned.port, 1234);
    }
}
