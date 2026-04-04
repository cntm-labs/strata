use crate::error::AppResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct LokiClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LokiResponse {
    pub status: String,
    pub data: serde_json::Value,
}

impl LokiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub async fn query(&self, query: &str, limit: Option<u32>) -> AppResult<LokiResponse> {
        let mut params = vec![("query", query.to_string())];
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/loki/api/v1/query", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<LokiResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn query_range(
        &self,
        query: &str,
        start: &str,
        end: &str,
        limit: Option<u32>,
    ) -> AppResult<LokiResponse> {
        let mut params = vec![
            ("query", query.to_string()),
            ("start", start.to_string()),
            ("end", end.to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit", l.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/loki/api/v1/query_range", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<LokiResponse>()
            .await?;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn new_trims_trailing_slash() {
        let client = LokiClient::new("http://localhost:3100/");
        assert_eq!(client.base_url, "http://localhost:3100");
    }

    #[tokio::test]
    async fn query_without_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .and(query_param("query", "{job=\"app\"}"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "streams", "result": []}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query("{job=\"app\"}", None).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_with_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client.query("{job=\"app\"}", Some(50)).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_all_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client
            .query_range("{job=\"app\"}", "1000", "2000", Some(100))
            .await
            .unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_without_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/loki/api/v1/query_range"))
            .and(query_param("query", "{job=\"app\"}"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = LokiClient::new(&server.uri());
        let resp = client
            .query_range("{job=\"app\"}", "1000", "2000", None)
            .await
            .unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_connection_error() {
        let client = LokiClient::new("http://127.0.0.1:1");
        let result = client.query("{job=\"app\"}", None).await;
        assert!(result.is_err());
    }
}
