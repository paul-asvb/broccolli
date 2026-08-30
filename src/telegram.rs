use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct UpdatesQuery {
    offset: Option<i64>,
    timeout: Option<u64>,
}

pub async fn updates(
    Query(params): Query<UpdatesQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    log::debug!("polling telegram getUpdates: offset={:?} timeout={:?}", params.offset, params.timeout);

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
        .map_err(|e| {
            log::error!("telegram getUpdates request failed: {e}");
            (StatusCode::BAD_GATEWAY, format!("telegram request failed: {e}"))
        })?
        .json::<Value>()
        .await
        .map_err(|e| {
            log::error!("failed to parse telegram getUpdates response: {e}");
            (
                StatusCode::BAD_GATEWAY,
                format!("failed to parse telegram response: {e}"),
            )
        })?;

    let update_count = resp["result"].as_array().map_or(0, |a| a.len());
    log::debug!("received {update_count} update(s) from telegram");

    Ok(Json(resp))
}

/// Maps a Telegram Bot API `Message` object into the shape `db::insert_messages`
/// expects (the Telegram Desktop export schema), returning its chat id alongside.
fn bot_message_to_export(message: &Value) -> Option<(i64, Value)> {
    let chat_id = message["chat"]["id"].as_i64()?;
    let message_id = message["message_id"].as_i64()?;
    let date_unixtime = message["date"].as_i64()?;

    let from = message.get("from");
    let from_name = from
        .and_then(|f| f["username"].as_str().or_else(|| f["first_name"].as_str()))
        .map(str::to_string);
    let from_id = from
        .and_then(|f| f["id"].as_i64())
        .map(|id| id.to_string());

    let text = message["text"]
        .as_str()
        .or_else(|| message["caption"].as_str())
        .map(str::to_string);

    let reply_to_message_id = message["reply_to_message"]["message_id"].as_i64();
    let edited_unixtime = message["edit_date"].as_i64().map(|t| t.to_string());

    let media_type = if message.get("photo").is_some() {
        Some("photo")
    } else if message.get("video").is_some() {
        Some("video_file")
    } else if message.get("voice").is_some() {
        Some("voice_message")
    } else if message.get("audio").is_some() {
        Some("audio_file")
    } else if message.get("document").is_some() {
        Some("document")
    } else if message.get("sticker").is_some() {
        Some("sticker")
    } else {
        None
    }
    .map(str::to_string);

    let file_name = message["document"]["file_name"].as_str().map(str::to_string);

    let export = json!({
        "id": message_id,
        "date_unixtime": date_unixtime.to_string(),
        "from": from_name,
        "from_id": from_id,
        "text": text,
        "reply_to_message_id": reply_to_message_id,
        "edited_unixtime": edited_unixtime,
        "media_type": media_type,
        "file_name": file_name,
    });

    Some((chat_id, export))
}

/// Receives pushed Telegram Bot API updates (set up via `setWebhook`), verifying the
/// `X-Telegram-Bot-Api-Secret-Token` header against `TELEGRAM_WEBHOOK_SECRET` before
/// mapping the message into `messages` via `db::insert_messages`.
pub async fn webhook(headers: HeaderMap, Json(update): Json<Value>) -> StatusCode {
    log::debug!("received telegram webhook update {:?}", update["update_id"]);

    let expected_secret = std::env::var("TELEGRAM_WEBHOOK_SECRET").unwrap_or_default();
    if expected_secret.is_empty() {
        log::error!("TELEGRAM_WEBHOOK_SECRET not set; rejecting webhook request");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    let provided = headers
        .get("x-telegram-bot-api-secret-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !crate::constant_time_eq(provided.as_bytes(), expected_secret.as_bytes()) {
        log::warn!("webhook request had an invalid or missing secret token");
        return StatusCode::UNAUTHORIZED;
    }

    let Some(message) = update.get("message").or_else(|| update.get("edited_message")) else {
        // updates we don't care about yet (channel posts, callback queries, ...) - ack anyway
        log::debug!("ignoring webhook update with no message/edited_message field");
        return StatusCode::OK;
    };

    let Some((chat_id, export)) = bot_message_to_export(message) else {
        log::warn!("webhook update missing required fields: {update}");
        return StatusCode::OK;
    };

    let conn = broccolli::db::connect().await;
    broccolli::db::insert_messages(&conn, chat_id, &[export]).await;
    log::debug!("processed webhook message for chat {chat_id}");

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_plain_text_message() {
        let message = json!({
            "message_id": 42,
            "date": 1_700_000_000i64,
            "chat": {"id": -100123, "type": "group"},
            "from": {"id": 555, "username": "paul", "first_name": "Paul"},
            "text": "check this out https://vm.tiktok.com/ZGdxtAF4f/",
            "reply_to_message": {"message_id": 41},
        });

        let (chat_id, export) = bot_message_to_export(&message).expect("should map");
        assert_eq!(chat_id, -100123);
        assert_eq!(export["id"], 42);
        assert_eq!(export["date_unixtime"], "1700000000");
        assert_eq!(export["from"], "paul");
        assert_eq!(export["from_id"], "555");
        assert_eq!(export["text"], "check this out https://vm.tiktok.com/ZGdxtAF4f/");
        assert_eq!(export["reply_to_message_id"], 41);
        assert!(export["media_type"].is_null());
    }

    #[test]
    fn maps_a_document_message() {
        let message = json!({
            "message_id": 7,
            "date": 1_700_000_000i64,
            "chat": {"id": 123, "type": "private"},
            "from": {"id": 1, "first_name": "Ada"},
            "document": {"file_name": "notes.pdf"},
            "caption": "see attached",
        });

        let (_, export) = bot_message_to_export(&message).expect("should map");
        assert_eq!(export["from"], "Ada");
        assert_eq!(export["text"], "see attached");
        assert_eq!(export["media_type"], "document");
        assert_eq!(export["file_name"], "notes.pdf");
    }

    #[test]
    fn rejects_a_message_missing_chat_id() {
        let message = json!({"message_id": 1, "date": 1_700_000_000i64});
        assert!(bot_message_to_export(&message).is_none());
    }
}
