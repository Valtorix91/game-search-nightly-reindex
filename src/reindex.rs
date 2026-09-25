use crate::infrai_client::{ApiError, InfraiClient};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const COLLECTION: &str = "game-live-search";

#[derive(Clone)]
pub struct Source {
    pub kind: &'static str,
    pub url: String,
}

#[derive(Debug, Error)]
pub enum ReindexError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("scrape response has no text for {0}")]
    MissingText(String),
}

#[derive(Serialize)]
pub struct ReindexResult {
    pub collection: &'static str,
    pub indexed: usize,
}

pub fn should_index(text: &str) -> bool {
    !text.trim().is_empty()
}

fn stable_id(kind: &str, url: &str) -> String {
    format!("{:x}", Sha256::digest(format!("{kind}:{url}").as_bytes()))
}

pub async fn run(client: &InfraiClient, sources: &[Source]) -> Result<ReindexResult, ReindexError> {
    let mut indexed = 0;
    for source in sources {
        let page = client.post("/v1/web/scrape", json!({"url":source.url,"format":"text"}), None).await?;
        let text = page.get("text").or_else(|| page.get("content"))
            .and_then(Value::as_str).ok_or_else(|| ReindexError::MissingText(source.url.clone()))?;
        if !should_index(text) {
            continue;
        }
        let embedding = client.embed(text).await?;
        let id = stable_id(source.kind, &source.url);
        client.post("/v1/vector/upsert", json!({"collection":COLLECTION,"vectors":[{
            "id":id,"values":embedding,"metadata":{"kind":source.kind,"url":source.url}
        }]}), Some(&format!("reindex-{id}-{:x}", Sha256::digest(text.as_bytes())))).await?;
        indexed += 1;
    }
    Ok(ReindexResult { collection: COLLECTION, indexed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_moderation_feed_does_not_replace_its_existing_vector() {
        assert!(!should_index(" \n  "));
        assert!(should_index("pending review: asset 42"));
        assert_eq!(stable_id("events", "https://example.org/feed"),
                   stable_id("events", "https://example.org/feed"));
        assert_ne!(stable_id("events", "https://example.org/feed"),
                   stable_id("assets", "https://example.org/feed"));
    }
}
