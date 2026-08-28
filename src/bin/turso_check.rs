use broccolli::db;

#[tokio::main]
async fn main() {
    let conn = db::connect().await;
    let columns = db::table_info(&conn).await;

    println!("messages columns:");
    for (name, ty) in columns {
        println!("  {name}: {ty}");
    }
}
