use broccolli::db;

#[tokio::main]
async fn main() {
    let conn = db::connect().await;
    db::ensure_schema(&conn).await;

    let affected = db::mark_empty_text_deleted(&conn).await;
    println!("marked {affected} message(s) with no content as deleted");
}
