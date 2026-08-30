mod auth;
mod messages;
mod processing;
mod telegram;
mod worker;

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::{Html, IntoResponse};
use axum::{
    routing::{get, post},
    Router,
};
use rust_embed::RustEmbed;
use std::net::SocketAddr;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

async fn health() -> &'static str {
    "ok"
}

async fn index() -> impl IntoResponse {
    match WebAssets::get("index.html") {
        Some(content) => Html(content.data.into_owned()).into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "web assets not built - run `trunk build` in web/",
        )
            .into_response(),
    }
}

async fn web_asset(Path(path): Path<String>) -> impl IntoResponse {
    if let Some(content) = WebAssets::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        return (
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            content.data.into_owned(),
        )
            .into_response();
    }

    // unmatched api routes are a real 404, not a client-side route
    if path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    // fall back to index.html so the yew router can handle client-side routes
    // (e.g. /messages) that don't correspond to an embedded asset
    match WebAssets::get("index.html") {
        Some(content) => Html(content.data.into_owned()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Constant-time byte comparison to avoid leaking credential length/content via timing.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init();

    // the compiled frontend shell carries no secrets, so it's served unauthenticated;
    // only the api routes that actually touch data require a valid session cookie.
    let protected_api = Router::new()
        .route("/api/messages", get(messages::list))
        .route(
            "/api/messages/{chat_id}/{message_id}",
            get(processing::message_detail).delete(processing::delete_message),
        )
        .route(
            "/api/messages/{chat_id}/{message_id}/process",
            axum::routing::post(processing::enqueue),
        )
        .route("/api/processing", get(processing::summary))
        .route("/api/session", get(auth::session))
        .route_layer(middleware::from_fn(auth::cookie_auth));

    let app = Router::new()
        .route("/", get(index))
        .route("/{*path}", get(web_asset))
        .route("/health", get(health))
        .route("/telegram/updates", get(telegram::updates))
        .route("/telegram/webhook", post(telegram::webhook))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        .merge(protected_api);

    log::debug!("spawning background worker");
    tokio::spawn(worker::run());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    log::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
