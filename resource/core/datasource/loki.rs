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
