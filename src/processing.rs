use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use broccolli::db;
use serde::Serialize;

#[derive(Serialize)]
pub struct MessageDetail {
    message: Option<db::Message>,
    processing: Option<db::ProcessingState>,
    analysis: Option<db::Analysis>,
    tiktok_analysis: Option<db::TiktokAnalysis>,
    web_article_analysis: Option<db::WebArticleAnalysis>,
}

pub async fn message_detail(Path((chat_id, message_id)): Path<(i64, i64)>) -> Json<MessageDetail> {
    log::debug!("fetching message detail for chat {chat_id} message {message_id}");
    let conn = db::connect().await;
    let message = db::get_message(&conn, chat_id, message_id).await;
    let processing = db::get_processing_state(&conn, chat_id, message_id).await;
    let analysis = db::get_latest_analysis(&conn, chat_id, message_id).await;
    let tiktok_analysis = db::get_latest_tiktok_analysis(&conn, chat_id, message_id).await;
    let web_article_analysis = db::get_latest_web_article_analysis(&conn, chat_id, message_id).await;

    Json(MessageDetail {
        message,
        processing,
        analysis,
        tiktok_analysis,
        web_article_analysis,
    })
}

pub async fn delete_message(Path((chat_id, message_id)): Path<(i64, i64)>) -> StatusCode {
    log::debug!("deleting chat {chat_id} message {message_id}");
    let conn = db::connect().await;
    if db::mark_deleted(&conn, chat_id, message_id).await {
        StatusCode::NO_CONTENT
    } else {
        log::warn!("cannot delete chat {chat_id} message {message_id}: not found");
        StatusCode::NOT_FOUND
    }
}

pub async fn enqueue(
    Path((chat_id, message_id)): Path<(i64, i64)>,
) -> Result<Json<db::ProcessingState>, StatusCode> {
    log::debug!("enqueueing chat {chat_id} message {message_id} for processing");
    let conn = db::connect().await;
    if !db::enqueue_message(&conn, chat_id, message_id).await {
        log::warn!("cannot enqueue chat {chat_id} message {message_id}: not found");
        return Err(StatusCode::NOT_FOUND);
    }

    db::get_processing_state(&conn, chat_id, message_id)
        .await
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
pub struct ProcessingSummary {
    counts: Vec<db::StatusCount>,
    recent_errors: Vec<db::ErrorEntry>,
    recent_processed: Vec<db::RecentAnalysis>,
}

pub async fn summary() -> Json<ProcessingSummary> {
    log::debug!("fetching processing summary");
    let conn = db::connect().await;
    let counts = db::processing_status_counts(&conn).await;
    let recent_errors = db::recent_errors(&conn, 20).await;
    let recent_processed = db::recent_analyses(&conn, 20).await;

    Json(ProcessingSummary {
        counts,
        recent_errors,
        recent_processed,
    })
}
