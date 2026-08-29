use broccolli::{db, llm, tiktok};
use std::time::Duration;

const BATCH_SIZE: i64 = 5;
const CONTEXT_SIZE: i64 = 10;
const DEFAULT_MODEL: &str = "anthropic/claude-3.5-haiku";
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// Polls `processing_state` for pending messages and classifies each via an LLM,
/// recording the result in `analyses` and advancing its processing state.
pub async fn run() {
    let conn = db::connect().await;
    let client = reqwest::Client::new();

    let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let poll_interval = std::env::var("WORKER_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS);

    loop {
        let pending = db::claim_pending(&conn, BATCH_SIZE).await;
        if pending.is_empty() {
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        for (chat_id, message_id) in pending {
            let (target, context) = db::message_context(&conn, chat_id, message_id, CONTEXT_SIZE).await;
            let Some(target) = target else {
                db::mark_processing_error(&conn, chat_id, message_id, "message not found").await;
                continue;
            };

            match llm::classify(&client, &model, &context, &target).await {
                Ok((classification, raw)) => {
                    db::insert_analysis(
                        &conn,
                        chat_id,
                        message_id,
                        &classification.category,
                        classification.needs_followup,
                        &classification.reasoning,
                        &model,
                        &raw,
                    )
                    .await;

                    if classification.category == "tiktok_video" {
                        download_tiktok(chat_id, message_id, &target).await;
                    }

                    db::mark_processing_done(&conn, chat_id, message_id).await;
                }
                Err(err) => {
                    eprintln!("analysis failed for chat {chat_id} message {message_id}: {err}");
                    db::mark_processing_error(&conn, chat_id, message_id, &err).await;
                }
            }
        }
    }
}

/// Downloads and stashes a classified TikTok video in a temp dir, on a best-effort
/// basis. A failure here doesn't fail the message's processing state — the
/// classification itself already succeeded.
async fn download_tiktok(chat_id: i64, message_id: i64, target: &db::Message) {
    let Some(url) = target.text.as_deref().and_then(tiktok::find_url) else {
        eprintln!("tiktok_video classification for chat {chat_id} message {message_id} but no tiktok.com URL found in text");
        return;
    };

    match tiktok::download(url, &format!("{chat_id}_{message_id}")).await {
        Ok(path) => println!("saved tiktok video for chat {chat_id} message {message_id} to {}", path.display()),
        Err(err) => eprintln!("failed to download tiktok video for chat {chat_id} message {message_id}: {err}"),
    }
}
