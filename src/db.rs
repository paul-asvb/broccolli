use libsql::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS processing_state (
            chat_id INTEGER NOT NULL,
            message_id INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            attempts INTEGER NOT NULL DEFAULT 0,
            error TEXT,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (chat_id, message_id)
        )",
        (),
    )
    .await
    .expect("failed to create processing_state table");

    conn.execute(
        "CREATE INDEX IF NOT EXISTS processing_state_status_idx ON processing_state (status, updated_at)",
        (),
    )
    .await
    .expect("failed to create processing_state status index");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS analyses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id INTEGER NOT NULL,
            message_id INTEGER NOT NULL,
            category TEXT NOT NULL,
            needs_followup INTEGER NOT NULL,
            reasoning TEXT,
            model TEXT NOT NULL,
            raw_response TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
        (),
    )
    .await
    .expect("failed to create analyses table");

    conn.execute(
        "CREATE INDEX IF NOT EXISTS analyses_message_idx ON analyses (chat_id, message_id)",
        (),
    )
    .await
    .expect("failed to create analyses message index");
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

#[derive(Serialize)]
pub struct Message {
    pub chat_id: i64,
    pub message_id: i64,
    pub date_unixtime: i64,
    pub from_name: Option<String>,
    pub text: Option<String>,
}

pub async fn get_message(conn: &Connection, chat_id: i64, message_id: i64) -> Option<Message> {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages WHERE chat_id = ?1 AND message_id = ?2",
            params![chat_id, message_id],
        )
        .await
        .expect("failed to query message");

    rows.next().await.unwrap().map(|row| Message {
        chat_id: row.get(0).unwrap(),
        message_id: row.get(1).unwrap(),
        date_unixtime: row.get(2).unwrap(),
        from_name: row.get(3).unwrap(),
        text: row.get(4).unwrap(),
    })
}

pub async fn list_messages(conn: &Connection, page: i64, per_page: i64) -> Vec<Message> {
    let offset = (page.max(1) - 1) * per_page;
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages ORDER BY date_unixtime LIMIT ?1 OFFSET ?2",
            params![per_page, offset],
        )
        .await
        .expect("failed to list messages");

    let mut messages = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        messages.push(Message {
            chat_id: row.get(0).unwrap(),
            message_id: row.get(1).unwrap(),
            date_unixtime: row.get(2).unwrap(),
            from_name: row.get(3).unwrap(),
            text: row.get(4).unwrap(),
        });
    }
    messages
}

/// Claims up to `limit` pending messages for processing, marking them 'processing'
/// so a concurrent poll doesn't pick them up again.
pub async fn claim_pending(conn: &Connection, limit: i64) -> Vec<(i64, i64)> {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id FROM processing_state WHERE status = 'pending' ORDER BY updated_at LIMIT ?1",
            params![limit],
        )
        .await
        .expect("failed to query pending processing_state");

    let mut pending: Vec<(i64, i64)> = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        let chat_id: i64 = row.get(0).unwrap();
        let message_id: i64 = row.get(1).unwrap();
        pending.push((chat_id, message_id));
    }

    for (chat_id, message_id) in &pending {
        conn.execute(
            "UPDATE processing_state SET status = 'processing', updated_at = ?3 WHERE chat_id = ?1 AND message_id = ?2",
            params![*chat_id, *message_id, now_unix()],
        )
        .await
        .expect("failed to mark processing_state processing");
    }

    pending
}

/// Fetches the target message plus up to `context_size` preceding messages from the same chat.
pub async fn message_context(
    conn: &Connection,
    chat_id: i64,
    message_id: i64,
    context_size: i64,
) -> (Option<Message>, Vec<Message>) {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages WHERE chat_id = ?1 AND message_id = ?2",
            params![chat_id, message_id],
        )
        .await
        .expect("failed to query target message");

    let target = rows.next().await.unwrap().map(|row| Message {
        chat_id: row.get(0).unwrap(),
        message_id: row.get(1).unwrap(),
        date_unixtime: row.get(2).unwrap(),
        from_name: row.get(3).unwrap(),
        text: row.get(4).unwrap(),
    });

    let mut ctx_rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages
             WHERE chat_id = ?1 AND message_id < ?2 ORDER BY message_id DESC LIMIT ?3",
            params![chat_id, message_id, context_size],
        )
        .await
        .expect("failed to query context messages");

    let mut context = Vec::new();
    while let Some(row) = ctx_rows.next().await.unwrap() {
        context.push(Message {
            chat_id: row.get(0).unwrap(),
            message_id: row.get(1).unwrap(),
            date_unixtime: row.get(2).unwrap(),
            from_name: row.get(3).unwrap(),
            text: row.get(4).unwrap(),
        });
    }
    context.reverse();

    (target, context)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_analysis(
    conn: &Connection,
    chat_id: i64,
    message_id: i64,
    category: &str,
    needs_followup: bool,
    reasoning: &str,
    model: &str,
    raw_response: &str,
) {
    conn.execute(
        "INSERT INTO analyses (chat_id, message_id, category, needs_followup, reasoning, model, raw_response, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            chat_id,
            message_id,
            category,
            needs_followup as i64,
            reasoning,
            model,
            raw_response,
            now_unix()
        ],
    )
    .await
    .expect("failed to insert analysis");
}

pub async fn mark_processing_done(conn: &Connection, chat_id: i64, message_id: i64) {
    conn.execute(
        "UPDATE processing_state SET status = 'done', updated_at = ?3 WHERE chat_id = ?1 AND message_id = ?2",
        params![chat_id, message_id, now_unix()],
    )
    .await
    .expect("failed to mark processing_state done");
}

pub async fn mark_processing_error(conn: &Connection, chat_id: i64, message_id: i64, error: &str) {
    conn.execute(
        "UPDATE processing_state SET status = 'error', attempts = attempts + 1, error = ?3, updated_at = ?4
         WHERE chat_id = ?1 AND message_id = ?2",
        params![chat_id, message_id, error, now_unix()],
    )
    .await
    .expect("failed to mark processing_state error");
}

#[derive(Serialize)]
pub struct ProcessingState {
    pub status: String,
    pub attempts: i64,
    pub error: Option<String>,
    pub updated_at: i64,
}

pub async fn get_processing_state(
    conn: &Connection,
    chat_id: i64,
    message_id: i64,
) -> Option<ProcessingState> {
    let mut rows = conn
        .query(
            "SELECT status, attempts, error, updated_at FROM processing_state WHERE chat_id = ?1 AND message_id = ?2",
            params![chat_id, message_id],
        )
        .await
        .expect("failed to query processing_state");

    rows.next().await.unwrap().map(|row| ProcessingState {
        status: row.get(0).unwrap(),
        attempts: row.get(1).unwrap(),
        error: row.get(2).unwrap(),
        updated_at: row.get(3).unwrap(),
    })
}

#[derive(Serialize)]
pub struct Analysis {
    pub category: String,
    pub needs_followup: bool,
    pub reasoning: Option<String>,
    pub model: String,
    pub created_at: i64,
}

pub async fn get_latest_analysis(conn: &Connection, chat_id: i64, message_id: i64) -> Option<Analysis> {
    let mut rows = conn
        .query(
            "SELECT category, needs_followup, reasoning, model, created_at FROM analyses
             WHERE chat_id = ?1 AND message_id = ?2 ORDER BY created_at DESC LIMIT 1",
            params![chat_id, message_id],
        )
        .await
        .expect("failed to query analyses");

    rows.next().await.unwrap().map(|row| {
        let needs_followup: i64 = row.get(1).unwrap();
        Analysis {
            category: row.get(0).unwrap(),
            needs_followup: needs_followup != 0,
            reasoning: row.get(2).unwrap(),
            model: row.get(3).unwrap(),
            created_at: row.get(4).unwrap(),
        }
    })
}

#[derive(Serialize)]
pub struct RecentAnalysis {
    pub chat_id: i64,
    pub message_id: i64,
    pub category: String,
    pub needs_followup: bool,
    pub created_at: i64,
}

pub async fn recent_analyses(conn: &Connection, limit: i64) -> Vec<RecentAnalysis> {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, category, needs_followup, created_at FROM analyses
             ORDER BY created_at DESC LIMIT ?1",
            params![limit],
        )
        .await
        .expect("failed to query recent analyses");

    let mut recent = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        let needs_followup: i64 = row.get(3).unwrap();
        recent.push(RecentAnalysis {
            chat_id: row.get(0).unwrap(),
            message_id: row.get(1).unwrap(),
            category: row.get(2).unwrap(),
            needs_followup: needs_followup != 0,
            created_at: row.get(4).unwrap(),
        });
    }
    recent
}

/// (Re-)queues a message for processing. Returns false if the message doesn't exist.
pub async fn enqueue_message(conn: &Connection, chat_id: i64, message_id: i64) -> bool {
    if get_message(conn, chat_id, message_id).await.is_none() {
        return false;
    }

    conn.execute(
        "INSERT INTO processing_state (chat_id, message_id, status, updated_at)
         VALUES (?1, ?2, 'pending', ?3)
         ON CONFLICT (chat_id, message_id) DO UPDATE SET status = 'pending', error = NULL, updated_at = excluded.updated_at",
        params![chat_id, message_id, now_unix()],
    )
    .await
    .expect("failed to enqueue message");

    true
}

#[derive(Serialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

pub async fn processing_status_counts(conn: &Connection) -> Vec<StatusCount> {
    let mut rows = conn
        .query("SELECT status, COUNT(*) FROM processing_state GROUP BY status", ())
        .await
        .expect("failed to query processing_state counts");

    let mut counts = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        counts.push(StatusCount {
            status: row.get(0).unwrap(),
            count: row.get(1).unwrap(),
        });
    }
    counts
}

#[derive(Serialize)]
pub struct ErrorEntry {
    pub chat_id: i64,
    pub message_id: i64,
    pub attempts: i64,
    pub error: Option<String>,
    pub updated_at: i64,
}

pub async fn recent_errors(conn: &Connection, limit: i64) -> Vec<ErrorEntry> {
    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, attempts, error, updated_at FROM processing_state
             WHERE status = 'error' ORDER BY updated_at DESC LIMIT ?1",
            params![limit],
        )
        .await
        .expect("failed to query recent errors");

    let mut errors = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        errors.push(ErrorEntry {
            chat_id: row.get(0).unwrap(),
            message_id: row.get(1).unwrap(),
            attempts: row.get(2).unwrap(),
            error: row.get(3).unwrap(),
            updated_at: row.get(4).unwrap(),
        });
    }
    errors
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

    let pending_sql = "INSERT OR IGNORE INTO processing_state (chat_id, message_id, status, updated_at)
        VALUES (?1, ?2, 'pending', ?3)";
    let pending_stmt = conn
        .prepare(pending_sql)
        .await
        .expect("failed to prepare processing_state insert");

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

        pending_stmt
            .execute(params![chat_id, message_id, now_unix()])
            .await
            .expect("failed to insert processing_state");
        pending_stmt.reset();

        inserted += 1;
    }

    conn.execute("COMMIT", ())
        .await
        .expect("failed to commit transaction");

    inserted
}
