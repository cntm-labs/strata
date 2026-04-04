use crate::error::AppResult;
use serde_json::Value;
use sqlx::{Column, PgPool};

pub async fn execute_query(connection_url: &str, query: &str) -> AppResult<Vec<Value>> {
    let pool = PgPool::connect(connection_url).await?;
    let rows = sqlx::query(query).fetch_all(&pool).await?;

    let results: Vec<Value> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            let columns = row.columns();
            let mut obj = serde_json::Map::new();
            for col in columns {
                let val: Value = match col.type_info().to_string().as_str() {
                    "INT4" => row
                        .try_get::<i32, _>(col.ordinal())
                        .map(|v| Value::from(v as i64))
                        .unwrap_or(Value::Null),
                    "INT8" => row
                        .try_get::<i64, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    "FLOAT4" => row
                        .try_get::<f32, _>(col.ordinal())
                        .map(|v| Value::from(v as f64))
                        .unwrap_or(Value::Null),
                    "FLOAT8" => row
                        .try_get::<f64, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    "BOOL" => row
                        .try_get::<bool, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    _ => row
                        .try_get::<String, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                };
                obj.insert(col.name().to_string(), val);
            }
            Value::Object(obj)
        })
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db_url() -> String {
        dotenvy::dotenv().ok();
        std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://strata:secret@localhost:5432/strata".to_string())
    }

    #[tokio::test]
    async fn execute_select_one() {
        let results = execute_query(&test_db_url(), "SELECT 1 AS value")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["value"], 1);
    }

    #[tokio::test]
    async fn execute_multiple_rows() {
        let results = execute_query(
            &test_db_url(),
            "SELECT * FROM (VALUES (1, 'a'), (2, 'b')) AS t(id, name)",
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn execute_empty_result() {
        let results = execute_query(&test_db_url(), "SELECT 1 WHERE false")
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn execute_various_types() {
        let results = execute_query(
            &test_db_url(),
            "SELECT 42::int4 AS int_val, 3.14::float8 AS float_val, true AS bool_val, 'hello'::text AS text_val",
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["int_val"], 42);
        assert_eq!(results[0]["bool_val"], true);
        assert_eq!(results[0]["text_val"], "hello");
    }

    #[tokio::test]
    async fn execute_invalid_connection() {
        let result = execute_query(
            "postgres://invalid:invalid@127.0.0.1:1/nonexistent",
            "SELECT 1",
        )
        .await;
        assert!(result.is_err());
    }
}
