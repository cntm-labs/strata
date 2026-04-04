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
