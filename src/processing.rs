use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use broccolli::{db, tiktok};
use serde::Serialize;

#[derive(Serialize)]
pub struct MessageDetail {
    message: Option<db::Message>,
    processing: Option<db::ProcessingState>,
    analysis: Option<db::Analysis>,
}

pub async fn message_detail(Path((chat_id, message_id)): Path<(i64, i64)>) -> Json<MessageDetail> {
    let conn = db::connect().await;
    let message = db::get_message(&conn, chat_id, message_id).await;
    let processing = db::get_processing_state(&conn, chat_id, message_id).await;
    let analysis = db::get_latest_analysis(&conn, chat_id, message_id).await;

    Json(MessageDetail {
        message,
        processing,
        analysis,
    })
}

pub async fn enqueue(
    Path((chat_id, message_id)): Path<(i64, i64)>,
) -> Result<Json<db::ProcessingState>, StatusCode> {
    let conn = db::connect().await;
    if !db::enqueue_message(&conn, chat_id, message_id).await {
        return Err(StatusCode::NOT_FOUND);
    }

    db::get_processing_state(&conn, chat_id, message_id)
        .await
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Serialize)]
pub struct TiktokDownloadResult {
    path: String,
}

pub async fn download_tiktok(
    Path((chat_id, message_id)): Path<(i64, i64)>,
) -> Result<Json<TiktokDownloadResult>, (StatusCode, String)> {
    let conn = db::connect().await;
    let message = db::get_message(&conn, chat_id, message_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "message not found".to_string()))?;

    let url = message
        .text
        .as_deref()
        .and_then(tiktok::find_url)
        .ok_or((StatusCode::BAD_REQUEST, "no tiktok.com URL found in message text".to_string()))?;

    let path = tiktok::download(url, &format!("{chat_id}_{message_id}"))
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(TiktokDownloadResult {
        path: path.display().to_string(),
    }))
}

#[derive(Serialize)]
pub struct ProcessingSummary {
    counts: Vec<db::StatusCount>,
    recent_errors: Vec<db::ErrorEntry>,
    recent_processed: Vec<db::RecentAnalysis>,
}

pub async fn summary() -> Json<ProcessingSummary> {
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
