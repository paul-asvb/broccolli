use broccolli::db;

#[tokio::main]
async fn main() {
    let conn = db::connect().await;

    let count = db::count_messages(&conn).await;
    println!("row count: {count}");

    for sample in db::sample_messages(&conn, 3).await {
        let db::MessageSample {
            chat_id,
            message_id,
            date_unixtime,
            from_name,
            text,
        } = sample;
        println!("{chat_id}/{message_id} @ {date_unixtime} from={from_name:?} text={text:?}");
    }
}
