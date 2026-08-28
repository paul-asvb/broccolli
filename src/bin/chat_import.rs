use libsql::params;
use serde_json::Value;

fn flatten_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part {
                Value::String(s) => s.clone(),
                Value::Object(obj) => obj
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let path = std::env::args().nth(1).unwrap_or_else(|| "chat.json".to_string());
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let data: Value = serde_json::from_str(&raw).expect("failed to parse chat export json");

    let chat_id = data["id"].as_i64().expect("chat export missing top-level id");
    let messages = data["messages"].as_array().expect("chat export missing messages array");

    let url = std::env::var("TURSO_DATABASE_URL").expect("set TURSO_DATABASE_URL");
    let token = std::env::var("TURSO_AUTH_TOKEN").expect("set TURSO_AUTH_TOKEN");

    let db = libsql::Builder::new_remote(url, token)
        .build()
        .await
        .expect("failed to connect to turso");
    let conn = db.connect().expect("failed to open connection");

    conn.execute("BEGIN", ()).await.expect("failed to start transaction");

    let sql = "INSERT OR REPLACE INTO messages
        (chat_id, message_id, date_unixtime, from_name, from_id, text, reply_to_message_id, edited_unixtime, media_type, file_name, raw_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
    let stmt = conn.prepare(sql).await.expect("failed to prepare insert");

    let mut inserted = 0u64;
    for message in messages {
        let Some(message_id) = message["id"].as_i64() else {
            continue;
        };

        let date_unixtime: i64 = message["date_unixtime"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let from_name = message["from"]
            .as_str()
            .or_else(|| message["actor"].as_str())
            .map(str::to_string);
        let from_id = message["from_id"]
            .as_str()
            .or_else(|| message["actor_id"].as_str())
            .map(str::to_string);

        let text = flatten_text(&message["text"]);
        let text = if text.is_empty() { None } else { Some(text) };

        let reply_to_message_id = message["reply_to_message_id"].as_i64();
        let edited_unixtime: Option<i64> = message["edited_unixtime"]
            .as_str()
            .and_then(|s| s.parse().ok());
        let media_type = message["media_type"].as_str().map(str::to_string);
        let file_name = message["file_name"].as_str().map(str::to_string);
        let raw_json = message.to_string();

        stmt.execute(params![
            chat_id,
            message_id,
            date_unixtime,
            from_name,
            from_id,
            text,
            reply_to_message_id,
            edited_unixtime,
            media_type,
            file_name,
            raw_json
        ])
        .await
        .expect("failed to insert message");
        stmt.reset();

        inserted += 1;
    }

    conn.execute("COMMIT", ()).await.expect("failed to commit transaction");

    println!("imported {inserted} messages from {path} into chat_id {chat_id}");
}
