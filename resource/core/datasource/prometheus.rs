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
