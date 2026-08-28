use axum::extract::Query;
use axum::http::StatusCode;
use axum::{routing::get, Json, Router};
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;

async fn health() -> &'static str {
    "ok"
}

async fn root() -> &'static str {
    "Hello from broccolli!"
}

#[derive(Deserialize)]
struct UpdatesQuery {
    offset: Option<i64>,
    timeout: Option<u64>,
}

async fn telegram_updates(
    Query(params): Query<UpdatesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let token = std::env::var("TELEGRAM_BOT_TOKEN").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "TELEGRAM_BOT_TOKEN not set".to_string(),
        )
    })?;

    let mut query = vec![("timeout".to_string(), params.timeout.unwrap_or(0).to_string())];
    if let Some(offset) = params.offset {
        query.push(("offset".to_string(), offset.to_string()));
    }

    let resp = reqwest::Client::new()
        .get(format!("https://api.telegram.org/bot{token}/getUpdates"))
        .query(&query)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("telegram request failed: {e}")))?
        .json::<Value>()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("failed to parse telegram response: {e}"),
            )
        })?;

    Ok(Json(resp))
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/telegram/updates", get(telegram_updates));

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
