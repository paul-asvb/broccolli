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
    let vision_model =
        std::env::var("OPENROUTER_VISION_MODEL").unwrap_or_else(|_| llm::DEFAULT_VISION_MODEL.to_string());
    let poll_interval = std::env::var("WORKER_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_POLL_INTERVAL_SECS);

    log::debug!("worker started: model={model} vision_model={vision_model} poll_interval={poll_interval}s");

    loop {
        let pending = db::claim_pending(&conn, BATCH_SIZE).await;
        if pending.is_empty() {
            log::debug!("no pending messages, sleeping {poll_interval}s");
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        log::debug!("claimed {} pending message(s)", pending.len());

        for (chat_id, message_id) in pending {
            let (target, context) = db::message_context(&conn, chat_id, message_id, CONTEXT_SIZE).await;
            let Some(target) = target else {
                log::warn!("claimed processing_state for chat {chat_id} message {message_id} but message not found");
                db::mark_processing_error(&conn, chat_id, message_id, "message not found").await;
                continue;
            };

            match llm::classify(&client, &model, &context, &target).await {
                Ok((classification, raw)) => {
                    log::debug!(
                        "classified chat {chat_id} message {message_id} as {} (needs_followup={})",
                        classification.category,
                        classification.needs_followup
                    );

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
                        download_tiktok(&conn, &client, &vision_model, chat_id, message_id, &target).await;
                    }

                    db::mark_processing_done(&conn, chat_id, message_id).await;
                }
                Err(err) => {
                    log::error!("analysis failed for chat {chat_id} message {message_id}: {err}");
                    db::mark_processing_error(&conn, chat_id, message_id, &err).await;
                }
            }
        }
    }
}

/// Downloads the video, fetches its metadata, and stores a screenshot-derived summary
/// for a classified TikTok message, on a best-effort basis. A failure here doesn't fail
/// the message's processing state — the classification itself already succeeded.
async fn download_tiktok(
    conn: &libsql::Connection,
    client: &reqwest::Client,
    vision_model: &str,
    chat_id: i64,
    message_id: i64,
    target: &db::Message,
) {
    let Some(url) = target.text.as_deref().and_then(tiktok::find_url) else {
        log::warn!("tiktok_video classification for chat {chat_id} message {message_id} but no tiktok.com URL found in text");
        return;
    };

    match tiktok::download(url, &format!("{chat_id}_{message_id}")).await {
        Ok(path) => {
            log::debug!("saved tiktok video for chat {chat_id} message {message_id} to {}", path.display());

            match tiktok::capture_screenshots(&path, &format!("{chat_id}_{message_id}")).await {
                Ok(paths) => {
                    log::debug!(
                        "captured {} tiktok screenshots for chat {chat_id} message {message_id}",
                        paths.len()
                    );

                    match llm::analyze_screenshots(client, vision_model, &paths).await {
                        Ok((analysis, _raw)) => {
                            db::insert_tiktok_analysis(
                                conn,
                                chat_id,
                                message_id,
                                &analysis.summary,
                                &analysis.short_summary,
                                &analysis.on_screen_text,
                                &analysis.topics,
                                vision_model,
                            )
                            .await;
                            log::debug!("saved tiktok screenshot analysis for chat {chat_id} message {message_id}");
                        }
                        Err(err) => log::error!(
                            "failed to analyze tiktok screenshots for chat {chat_id} message {message_id}: {err}"
                        ),
                    }
                }
                Err(err) => log::error!("failed to capture tiktok screenshots for chat {chat_id} message {message_id}: {err}"),
            }
        }
        Err(err) => log::error!("failed to download tiktok video for chat {chat_id} message {message_id}: {err}"),
    }

    match tiktok::fetch_metadata(client, url).await {
        Ok(metadata) => match tiktok::save_metadata_to_temp(&metadata, &format!("{chat_id}_{message_id}")) {
            Ok(path) => log::debug!("saved tiktok metadata for chat {chat_id} message {message_id} to {}", path.display()),
            Err(err) => log::error!("failed to save tiktok metadata for chat {chat_id} message {message_id}: {err}"),
        },
        Err(err) => log::error!("failed to fetch tiktok metadata for chat {chat_id} message {message_id}: {err}"),
    }
}
