use axum::extract::Query;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct UpdatesQuery {
    offset: Option<i64>,
    timeout: Option<u64>,
}

pub async fn updates(
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
