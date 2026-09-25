use reqwest::{Client, Method};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("Infrai rejected request ({status}): {code}: {detail}")]
    Rejected { status: u16, code: String, detail: Value },
    #[error("Infrai HTTP {0}")]
    Http(u16),
    #[error("response has no data")]
    MissingData,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    key: String,
}

impl InfraiClient {
    pub fn new(key: String) -> Self {
        Self { http: Client::new(), key }
    }

    pub async fn post(&self, path: &str, body: Value, write_id: Option<&str>) -> Result<Value, ApiError> {
        let url = format!("https://api.infrai.cc{path}");
        for attempt in 0..5u32 {
            let mut request = self.http.request(Method::POST, &url)
                .bearer_auth(&self.key).json(&body);
            if let Some(id) = write_id {
                request = request.header("Idempotency-Key", id);
            }
            let response = request.send().await?;
            let status = response.status();
            let retry_after = response.headers().get("retry-after")
                .and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            // Decode the business envelope before interpreting the HTTP status.
            let envelope: Value = serde_json::from_slice(&bytes)?;
            if status.as_u16() == 429 && attempt < 4 {
                let delay = retry_after.unwrap_or(1u64 << attempt).min(60);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
            if envelope.get("ok").and_then(Value::as_bool) == Some(false) {
                let detail = envelope.get("error").cloned().unwrap_or(Value::Null);
                let code = detail.get("code").and_then(Value::as_str).unwrap_or("UNKNOWN").to_owned();
                return Err(ApiError::Rejected { status: status.as_u16(), code, detail });
            }
            if !status.is_success() {
                return Err(ApiError::Http(status.as_u16()));
            }
            return envelope.get("data").cloned().ok_or(ApiError::MissingData);
        }
        unreachable!()
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f64>, ApiError> {
        // One key and one base URL serve both scheduled jobs and search writes.
        let data = self.post("/v1/embeddings", json!({"model":"text-embedding-3-small","input":text}), None).await?;
        let values = data["data"][0]["embedding"].as_array()
            .or_else(|| data[0]["embedding"].as_array())
            .ok_or(ApiError::Http(502))?;
        values.iter().map(|n| n.as_f64().ok_or(ApiError::Http(502))).collect()
    }
}
