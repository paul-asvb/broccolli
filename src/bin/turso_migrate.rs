use broccolli::db;

#[tokio::main]
async fn main() {
    let conn = db::connect().await;
    db::ensure_schema(&conn).await;
    println!("messages table ready");
}
