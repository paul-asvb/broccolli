use broccolli::db;
use serde_json::Value;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let path = std::env::args().nth(1).unwrap_or_else(|| "chat.json".to_string());
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let data: Value = serde_json::from_str(&raw).expect("failed to parse chat export json");

    let chat_id = data["id"].as_i64().expect("chat export missing top-level id");
    let messages = data["messages"]
        .as_array()
        .expect("chat export missing messages array");

    let conn = db::connect().await;
    let inserted = db::insert_messages(&conn, chat_id, messages).await;

    println!("imported {inserted} messages from {path} into chat_id {chat_id}");
}
