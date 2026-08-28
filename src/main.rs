use axum::body::Body;
use axum::extract::{Path, Query, Request};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::{routing::get, Json, Router};
use base64::Engine;
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

async fn health() -> &'static str {
    "ok"
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
    match WebAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref().to_string())],
                content.data.into_owned(),
            )
                .into_response()
        }
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

    let protected = Router::new()
        .route("/", get(index))
        .route("/{*path}", get(web_asset))
        .route_layer(middleware::from_fn(basic_auth));

    let app = Router::new()
        .route("/health", get(health))
        .route("/telegram/updates", get(telegram_updates))
        .merge(protected);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
