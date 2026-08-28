use axum::extract::Query;
use axum::Json;
use broccolli::db;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct MessagesQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Serialize)]
pub struct MessagesResponse {
    messages: Vec<db::Message>,
    page: i64,
    per_page: i64,
    total: i64,
}

pub async fn list(Query(params): Query<MessagesQuery>) -> Json<MessagesResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 500);

    let conn = db::connect().await;
    let messages = db::list_messages(&conn, page, per_page).await;
    let total = db::count_messages(&conn).await;

    Json(MessagesResponse {
        messages,
        page,
        per_page,
        total,
    })
}
