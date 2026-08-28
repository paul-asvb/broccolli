use libsql::{params, Connection};
use serde_json::Value;

pub async fn connect() -> Connection {
    dotenvy::dotenv().ok();

    let url = std::env::var("TURSO_DATABASE_URL").expect("set TURSO_DATABASE_URL");
    let token = std::env::var("TURSO_AUTH_TOKEN").expect("set TURSO_AUTH_TOKEN");

    let db = libsql::Builder::new_remote(url, token)
        .build()
        .await
        .expect("failed to connect to turso");
    db.connect().expect("failed to open connection")
}

pub async fn ensure_schema(conn: &Connection) {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            chat_id INTEGER NOT NULL,
            message_id INTEGER NOT NULL,
            date_unixtime INTEGER NOT NULL,
            from_name TEXT,
            from_id TEXT,
            text TEXT,
            reply_to_message_id INTEGER,
            edited_unixtime INTEGER,
            media_type TEXT,
            file_name TEXT,
            raw_json TEXT NOT NULL,
            PRIMARY KEY (chat_id, message_id)
        )",
        (),
    )
    .await
    .expect("failed to create messages table");

    conn.execute(
        "CREATE INDEX IF NOT EXISTS messages_date_idx ON messages (date_unixtime)",
        (),
    )
    .await
    .expect("failed to create date index");
}

pub async fn table_info(conn: &Connection) -> Vec<(String, String)> {
    let mut rows = conn
        .query("PRAGMA table_info(messages)", ())
        .await
        .expect("failed to query schema");

    let mut columns = Vec::new();
    while let Some(row) = rows.next().await.expect("failed to read row") {
        let name: String = row.get(1).unwrap();
        let ty: String = row.get(2).unwrap();
        columns.push((name, ty));
    }
    columns
}

pub async fn count_messages(conn: &Connection) -> i64 {
    let mut rows = conn
        .query("SELECT COUNT(*) FROM messages", ())
        .await
        .expect("failed to count");
    let row = rows.next().await.unwrap().unwrap();
    row.get(0).unwrap()
}

pub struct MessageSample {
    pub chat_id: i64,
    pub message_id: i64,
    pub date_unixtime: i64,
    pub from_name: Option<String>,
    pub text: Option<String>,
}

pub async fn sample_messages(conn: &Connection, limit: i64) -> Vec<MessageSample> {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages ORDER BY date_unixtime LIMIT ?1",
            params![limit],
        )
        .await
        .expect("failed to sample");

    let mut samples = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        samples.push(MessageSample {
            chat_id: row.get(0).unwrap(),
            message_id: row.get(1).unwrap(),
            date_unixtime: row.get(2).unwrap(),
            from_name: row.get(3).unwrap(),
            text: row.get(4).unwrap(),
        });
    }
    samples
}

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

pub async fn insert_messages(conn: &Connection, chat_id: i64, messages: &[Value]) -> u64 {
    conn.execute("BEGIN", ())
        .await
        .expect("failed to start transaction");

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

    conn.execute("COMMIT", ())
        .await
        .expect("failed to commit transaction");

    inserted
}
