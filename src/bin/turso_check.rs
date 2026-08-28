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

    let mut rows = conn
        .query("SELECT COUNT(*) FROM messages", ())
        .await
        .expect("failed to count");
    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    println!("row count: {count}");

    let mut rows = conn
        .query(
            "SELECT chat_id, message_id, date_unixtime, from_name, text FROM messages ORDER BY date_unixtime LIMIT 3",
            (),
        )
        .await
        .expect("failed to sample");
    while let Some(row) = rows.next().await.unwrap() {
        let chat_id: i64 = row.get(0).unwrap();
        let message_id: i64 = row.get(1).unwrap();
        let date_unixtime: i64 = row.get(2).unwrap();
        let from_name: Option<String> = row.get(3).unwrap();
        let text: Option<String> = row.get(4).unwrap();
        println!("{chat_id}/{message_id} @ {date_unixtime} from={from_name:?} text={text:?}");
    }
}
