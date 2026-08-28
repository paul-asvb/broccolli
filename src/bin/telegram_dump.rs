use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;

const OFFSET_FILE: &str = "telegram_dump.offset";
const DUMP_FILE: &str = "telegram_dump.jsonl";

#[tokio::main]
async fn main() {
    let token = std::env::var("TELEGRAM_BOT_TOKEN")
        .expect("set TELEGRAM_BOT_TOKEN to your bot's token from BotFather");

    let client = reqwest::Client::new();
    let mut offset: i64 = std::fs::read_to_string(OFFSET_FILE)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    println!("polling for updates (Ctrl+C to stop)... dumping to {DUMP_FILE}");

    loop {
        let url = format!("https://api.telegram.org/bot{token}/getUpdates");
        let resp: Value = client
            .get(&url)
            .query(&[
                ("offset", offset.to_string()),
                ("timeout", "30".to_string()),
                ("allowed_updates", r#"["message"]"#.to_string()),
            ])
            .send()
            .await
            .expect("request to telegram failed")
            .json()
            .await
            .expect("failed to parse telegram response");

        let updates = resp["result"].as_array().cloned().unwrap_or_default();
        if !updates.is_empty() {
            let mut dump = OpenOptions::new()
                .create(true)
                .append(true)
                .open(DUMP_FILE)
                .expect("failed to open dump file");

            for update in &updates {
                writeln!(dump, "{update}").expect("failed to write dump line");
                if let Some(update_id) = update["update_id"].as_i64() {
                    offset = update_id + 1;
                }
            }
            dump.flush().expect("failed to flush dump file");
            std::fs::write(OFFSET_FILE, offset.to_string()).expect("failed to persist offset");

            println!("dumped {} update(s), offset now {offset}", updates.len());
        }
    }
}
