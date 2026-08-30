mod messages;
mod processing;
mod telegram;
mod worker;

use axum::body::Body;
use axum::extract::{Path, Request};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::{
    routing::{get, post},
    Router,
};
use base64::Engine;
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

async fn basic_auth(req: Request, next: Next) -> Response {
    let unauthorized = || {
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, r#"Basic realm="broccolli""#)
            .body(Body::from("unauthorized"))
            .unwrap()
    };

    let expected_user = std::env::var("BASIC_AUTH_USER").unwrap_or_default();
    let expected_pass = std::env::var("BASIC_AUTH_PASS").unwrap_or_default();
    if expected_user.is_empty() || expected_pass.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "BASIC_AUTH_USER / BASIC_AUTH_PASS not set",
        )
            .into_response();
    }

    let Some(credentials) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Basic "))
        .and_then(|encoded| base64::engine::general_purpose::STANDARD.decode(encoded).ok())
        .and_then(|decoded| String::from_utf8(decoded).ok())
    else {
        return unauthorized();
    };

    let Some((user, pass)) = credentials.split_once(':') else {
        return unauthorized();
    };

    if constant_time_eq(user.as_bytes(), expected_user.as_bytes())
        && constant_time_eq(pass.as_bytes(), expected_pass.as_bytes())
    {
        next.run(req).await
    } else {
        unauthorized()
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    env_logger::init();

    let protected = Router::new()
        .route("/", get(index))
        .route("/api/messages", get(messages::list))
        .route(
            "/api/messages/{chat_id}/{message_id}",
            get(processing::message_detail),
        )
        .route(
            "/api/messages/{chat_id}/{message_id}/process",
            axum::routing::post(processing::enqueue),
        )
        .route("/api/processing", get(processing::summary))
        .route("/{*path}", get(web_asset))
        .route_layer(middleware::from_fn(basic_auth));

    let app = Router::new()
        .route("/health", get(health))
        .route("/telegram/updates", get(telegram::updates))
        .route("/telegram/webhook", post(telegram::webhook))
        .merge(protected);

    tokio::spawn(worker::run());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
