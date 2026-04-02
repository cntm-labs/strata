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
                    "INT4" | "INT8" => row
                        .try_get::<i64, _>(col.ordinal())
                        .map(Value::from)
                        .unwrap_or(Value::Null),
                    "FLOAT4" | "FLOAT8" => row
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
