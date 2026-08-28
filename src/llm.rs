use crate::db::Message;
use serde::Deserialize;
use serde_json::Value;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Categories a message can be triaged into, based on what it contains rather than
/// what it means. Each category maps to a distinct downstream processing pipeline
/// (e.g. a TikTok downloader, a web crawler). `needs_followup` signals whether the
/// message contains something that actually requires fetching/downloading external
/// content (false for e.g. `plain_text` with nothing to act on).
const CATEGORIES: &[&str] = &[
    "tiktok_video",
    "youtube_video",
    "instagram_content",
    "web_article",
    "media_attachment",
    "plain_text",
];

#[derive(Debug, Clone, Deserialize)]
pub struct Classification {
    pub category: String,
    pub needs_followup: bool,
    pub reasoning: String,
}

fn format_message(m: &Message) -> String {
    format!(
        "{}: {}",
        m.from_name.as_deref().unwrap_or("?"),
        m.text.as_deref().unwrap_or("")
    )
}

/// Classifies `target` given up to `context_size` preceding messages as context,
/// returning the parsed classification plus the raw provider response (for storage/audit).
pub async fn classify(
    client: &reqwest::Client,
    model: &str,
    context: &[Message],
    target: &Message,
) -> Result<(Classification, String), String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;

    let context_text = context.iter().map(format_message).collect::<Vec<_>>().join("\n");
    let target_text = format_message(target);

    let system_prompt = format!(
        "You triage messages from a group chat to decide what kind of automated processing, if any, \
         should be run on them next. Classify the TARGET message into exactly one category from: {}. \
         Use these definitions: \
         tiktok_video = contains a TikTok link (e.g. vm.tiktok.com); \
         youtube_video = contains a YouTube link (youtube.com or youtu.be); \
         instagram_content = contains an Instagram link; \
         web_article = contains any other link to a webpage, repo, article, product, or map that should be \
         crawled for its content; \
         media_attachment = the message itself is a file sent in the chat (voice message, video, audio, \
         image, or document) rather than a link; \
         plain_text = the message has no link and no attachment. \
         Set needs_followup to true only if the category implies something should be fetched or downloaded \
         (i.e. anything other than plain_text); set it to false for plain_text. \
         Respond with ONLY a JSON object of the form \
         {{\"category\": string, \"needs_followup\": boolean, \"reasoning\": string}}, no other text.",
        CATEGORIES.join(", ")
    );

    let user_prompt = if context_text.is_empty() {
        format!("TARGET message to classify:\n{target_text}")
    } else {
        format!("Recent context (oldest to newest):\n{context_text}\n\nTARGET message to classify:\n{target_text}")
    };

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt},
        ],
        "response_format": {"type": "json_object"},
    });

    let resp = client
        .post(OPENROUTER_URL)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("openrouter request failed: {e}"))?;

    let raw: Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse openrouter response: {e}"))?;

    let content = raw["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("unexpected openrouter response shape: {raw}"))?;

    let classification: Classification = serde_json::from_str(content)
        .map_err(|e| format!("failed to parse classification json ({content}): {e}"))?;

    Ok((classification, raw.to_string()))
}
