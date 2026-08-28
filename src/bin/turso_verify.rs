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
        .query("PRAGMA table_info(messages)", ())
        .await
        .expect("failed to query schema");

    println!("messages columns:");
    while let Some(row) = rows.next().await.expect("failed to read row") {
        let name: String = row.get(1).unwrap();
        let ty: String = row.get(2).unwrap();
        println!("  {name}: {ty}");
    }
}
