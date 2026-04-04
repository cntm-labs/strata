use crate::error::AppResult;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct PrometheusClient {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusResponse {
    pub status: String,
    pub data: serde_json::Value,
}

impl PrometheusClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Client::new(),
        }
    }

    pub async fn query(
        &self,
        query: &str,
        time: Option<&str>,
        timeout: Option<&str>,
    ) -> AppResult<PrometheusResponse> {
        let mut params = vec![("query", query.to_string())];
        if let Some(t) = time {
            params.push(("time", t.to_string()));
        }
        if let Some(t) = timeout {
            params.push(("timeout", t.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/api/v1/query", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<PrometheusResponse>()
            .await?;
        Ok(resp)
    }

    pub async fn query_range(
        &self,
        query: &str,
        start: &str,
        end: &str,
        step: &str,
    ) -> AppResult<PrometheusResponse> {
        let params = [
            ("query", query),
            ("start", start),
            ("end", end),
            ("step", step),
        ];

        let resp = self
            .client
            .get(format!("{}/api/v1/query_range", self.base_url))
            .query(&params)
            .send()
            .await?
            .json::<PrometheusResponse>()
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
        let client = PrometheusClient::new("http://localhost:9090/");
        assert_eq!(client.base_url, "http://localhost:9090");
    }

    #[tokio::test]
    async fn query_instant_minimal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "vector", "result": []}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client.query("up", None, None).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_with_time_and_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query"))
            .and(query_param("query", "up"))
            .and(query_param("time", "1234"))
            .and(query_param("timeout", "30s"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client.query("up", Some("1234"), Some("30s")).await.unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_range_sends_all_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/query_range"))
            .and(query_param("query", "rate(http_total[5m])"))
            .and(query_param("start", "1000"))
            .and(query_param("end", "2000"))
            .and(query_param("step", "15"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "success",
                "data": {"resultType": "matrix", "result": []}
            })))
            .mount(&server)
            .await;

        let client = PrometheusClient::new(&server.uri());
        let resp = client
            .query_range("rate(http_total[5m])", "1000", "2000", "15")
            .await
            .unwrap();
        assert_eq!(resp.status, "success");
    }

    #[tokio::test]
    async fn query_connection_error() {
        let client = PrometheusClient::new("http://127.0.0.1:1");
        let result = client.query("up", None, None).await;
        assert!(result.is_err());
    }
}
