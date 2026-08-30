use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

const COOKIE_NAME: &str = "broccolli_session";
const SESSION_LIFETIME_SECS: u64 = 7 * 24 * 60 * 60;

type HmacSha256 = Hmac<Sha256>;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn sign(expires_at: u64, secret: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(expires_at.to_string().as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Stateless bearer token: `{expiry_unix_ts}.{hmac_signature}`, delivered via cookie.
fn issue_token(secret: &[u8]) -> String {
    let expires_at = now() + SESSION_LIFETIME_SECS;
    format!("{expires_at}.{}", sign(expires_at, secret))
}

fn verify_token(token: &str, secret: &[u8]) -> bool {
    let Some((expires_at_str, signature)) = token.split_once('.') else {
        return false;
    };
    let Ok(expires_at) = expires_at_str.parse::<u64>() else {
        return false;
    };
    if expires_at < now() {
        return false;
    }
    crate::constant_time_eq(signature.as_bytes(), sign(expires_at, secret).as_bytes())
}

fn session_secret() -> Option<String> {
    std::env::var("SESSION_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
}

fn cookie_from_headers(headers: &header::HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_string())
    })
}

/// `Secure` is dropped only when explicitly opted out, for local http-only dev.
fn set_cookie_header(value: &str, max_age: i64) -> String {
    let secure = if std::env::var("COOKIE_INSECURE").is_ok() {
        ""
    } else {
        "; Secure"
    };
    format!("{COOKIE_NAME}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}")
}

fn missing_config(what: &str) -> Response {
    log::error!("{what} not set");
    (StatusCode::INTERNAL_SERVER_ERROR, format!("{what} not set")).into_response()
}

pub async fn cookie_auth(req: Request, next: Next) -> Response {
    let Some(secret) = session_secret() else {
        return missing_config("SESSION_SECRET");
    };

    let path = req.uri().path().to_string();
    let token = cookie_from_headers(req.headers(), COOKIE_NAME);

    match token {
        Some(token) if verify_token(&token, secret.as_bytes()) => next.run(req).await,
        _ => {
            log::warn!("unauthorized request to {path}: missing or invalid session cookie");
            (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

pub async fn login(Json(body): Json<LoginRequest>) -> Response {
    let Some(secret) = session_secret() else {
        return missing_config("SESSION_SECRET");
    };
    let admin_auth = std::env::var("ADMIN_AUTH").unwrap_or_default();
    let Some((expected_user, expected_pass)) = admin_auth.split_once(':') else {
        return missing_config("ADMIN_AUTH (expected user:pass)");
    };

    if crate::constant_time_eq(body.username.as_bytes(), expected_user.as_bytes())
        && crate::constant_time_eq(body.password.as_bytes(), expected_pass.as_bytes())
    {
        let token = issue_token(secret.as_bytes());
        let cookie = set_cookie_header(&token, SESSION_LIFETIME_SECS as i64);
        (StatusCode::OK, [(header::SET_COOKIE, cookie)]).into_response()
    } else {
        log::warn!("failed login attempt for user {}", body.username);
        (StatusCode::UNAUTHORIZED, "invalid credentials").into_response()
    }
}

pub async fn logout() -> Response {
    let cookie = set_cookie_header("", 0);
    (StatusCode::OK, [(header::SET_COOKIE, cookie)]).into_response()
}

pub async fn session() -> StatusCode {
    StatusCode::OK
}
