#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let url = std::env::var("TURSO_DATABASE_URL").expect("set TURSO_DATABASE_URL");
    let token = std::env::var("TURSO_AUTH_TOKEN").expect("set TURSO_AUTH_TOKEN");

    let db = libsql::Builder::new_remote(url, token)
        .build()
        .await
        .expect("failed to connect to turso");
    let conn = db.connect().expect("failed to open connection");

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

    println!("messages table ready");
}
