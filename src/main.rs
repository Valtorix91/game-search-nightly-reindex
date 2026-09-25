mod infrai_client;
mod reindex;

use axum::{extract::{Path, State}, http::StatusCode, routing::post, Json, Router};
use infrai_client::{ApiError, InfraiClient};
use reindex::{ReindexError, Source, COLLECTION};
use serde_json::{json, Value};
use std::{env, sync::Arc};

struct AppState {
    client: InfraiClient,
    token: String,
    sources: Vec<Source>,
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

fn client_status(error: &ApiError) -> StatusCode {
    match error {
        ApiError::Rejected { status, .. } if (400..500).contains(status) =>
            StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST),
        _ => StatusCode::BAD_GATEWAY,
    }
}

async fn reindex_handler(
    State(state): State<Arc<AppState>>, Path(token): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if token != state.token {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({"error":"unauthorized"}))));
    }
    reindex::run(&state.client, &state.sources).await
        .map(|result| Json(json!(result)))
        .map_err(|error| {
            let status = match &error {
                ReindexError::Api(api) => client_status(api),
                ReindexError::MissingText(_) => StatusCode::BAD_GATEWAY,
            };
            (status, Json(json!({"error":error.to_string()})))
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = env::args().nth(1).unwrap_or_else(|| "serve".into());
    let client = InfraiClient::new(required("INFRAI_API_KEY"));
    match command.as_str() {
        "init" => {
            let data = client.post("/v1/vector/collection/create", json!({
                "collection":COLLECTION,"dimension":1536,"metric":"cosine"
            }), Some("game-live-search-collection-v1")).await?;
            println!("{}", json!({"collection":COLLECTION,"data":data}));
        }
        "schedule" => {
            let task = format!("{}/reindex/{}", required("SERVICE_URL").trim_end_matches('/'), required("REINDEX_TOKEN"));
            let data = client.post("/v1/cron/create", json!({"cron_expr":"0 2 * * *","task":task}),
                Some("game-live-search-nightly-v1")).await?;
            println!("{}", json!({"job_id":data["job_id"]}));
        }
        "serve" => {
            let sources = [
                ("assets", "ASSET_FEED_URL"),
                ("events", "EVENT_FEED_URL"),
                ("moderation", "MODERATION_FEED_URL"),
            ].into_iter().map(|(kind, name)| Source { kind, url: required(name) }).collect();
            let state = Arc::new(AppState { client, token: required("REINDEX_TOKEN"), sources });
            let app = Router::new().route("/reindex/:token", post(reindex_handler)).with_state(state);
            let listener = tokio::net::TcpListener::bind(required("LISTEN_ADDR")).await?;
            axum::serve(listener, app).await?;
        }
        _ => return Err("expected init, serve or schedule".into()),
    }
    Ok(())
}
