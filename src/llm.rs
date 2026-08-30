use crate::db::Message;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Default vision-capable model used to analyze TikTok video screenshots.
pub const DEFAULT_VISION_MODEL: &str = "google/gemini-2.5-flash";

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

    log::debug!(
        "classifying chat {} message {} with model {model} ({} context messages)",
        target.chat_id,
        target.message_id,
        context.len()
    );

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

    log::debug!("openrouter request body: {body}");

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

    log::debug!("openrouter raw response: {raw}");

    let content = raw["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("unexpected openrouter response shape: {raw}"))?;

    let classification: Classification = serde_json::from_str(content)
        .map_err(|e| format!("failed to parse classification json ({content}): {e}"))?;

    log::debug!(
        "classified chat {} message {} as category={} needs_followup={}",
        target.chat_id,
        target.message_id,
        classification.category,
        classification.needs_followup
    );

    Ok((classification, raw.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSummary {
    pub summary: String,
    pub short_summary: String,
}

/// Caps how much crawled page text is sent to the model, to keep the request reasonably sized.
const MAX_ARTICLE_CHARS: usize = 12_000;

/// Summarizes crawled web article text via a text-capable OpenRouter model.
pub async fn summarize_article(
    client: &reqwest::Client,
    model: &str,
    title: Option<&str>,
    body_text: &str,
) -> Result<(ArticleSummary, String), String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;

    let truncated: String = body_text.chars().take(MAX_ARTICLE_CHARS).collect();
    log::debug!(
        "summarizing article with model {model} ({} of {} chars used)",
        truncated.len(),
        body_text.len()
    );

    let title_line = title.map(|t| format!("Title: {t}\n")).unwrap_or_default();
    let prompt = format!(
        "{title_line}The following is the crawled text content of a webpage. Write a concise summary of \
         what it says, plus a short_summary: a single sentence, under 100 characters, suitable for a table \
         column. Respond with ONLY a JSON object of the form \
         {{\"summary\": string, \"short_summary\": string}}, no other text.\n\nContent:\n{truncated}"
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt},
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

    log::debug!("openrouter article-summary raw response: {raw}");

    let response_text = raw["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("unexpected openrouter response shape: {raw}"))?;

    let analysis: ArticleSummary = serde_json::from_str(response_text)
        .map_err(|e| format!("failed to parse article summary json ({response_text}): {e}"))?;

    log::debug!("article summary: short_summary={}", analysis.short_summary);

    Ok((analysis, raw.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotAnalysis {
    pub summary: String,
    pub short_summary: String,
    pub on_screen_text: String,
    pub topics: Vec<String>,
}

/// Analyzes a chronologically-ordered sequence of video screenshots (e.g. from
/// `tiktok::capture_screenshots`) via a vision-capable OpenRouter model, extracting
/// on-screen text and a summary of what the video shows.
pub async fn analyze_screenshots(
    client: &reqwest::Client,
    model: &str,
    screenshots: &[std::path::PathBuf],
) -> Result<(ScreenshotAnalysis, String), String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| "OPENROUTER_API_KEY not set".to_string())?;

    log::debug!("analyzing {} screenshots with model {model}", screenshots.len());

    let prompt = format!(
        "The following {} images are screenshots taken at even 5% intervals throughout a video, \
         in chronological order. Extract any on-screen text (captions, overlays, subtitles) verbatim \
         where legible, and write a concise summary of what the video shows and its topics, plus a \
         short_summary: a single sentence, under 100 characters, suitable for a table column. \
         Respond with ONLY a JSON object of the form \
         {{\"summary\": string, \"short_summary\": string, \"on_screen_text\": string, \"topics\": [string]}}, \
         no other text.",
        screenshots.len()
    );

    let mut content = vec![serde_json::json!({"type": "text", "text": prompt})];
    for path in screenshots {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("failed to read screenshot {}: {e}", path.display()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        content.push(serde_json::json!({
            "type": "image_url",
            "image_url": {"url": format!("data:image/jpeg;base64,{encoded}")},
        }));
    }

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": content},
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

    log::debug!("openrouter screenshot-analysis raw response: {raw}");

    let content = raw["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("unexpected openrouter response shape: {raw}"))?;

    let analysis: ScreenshotAnalysis = serde_json::from_str(content)
        .map_err(|e| format!("failed to parse screenshot analysis json ({content}): {e}"))?;

    log::debug!("screenshot analysis: {} topics found", analysis.topics.len());

    Ok((analysis, raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "hits the live TikTok CDN via yt-dlp/ffmpeg and the live OpenRouter API; requires OPENROUTER_API_KEY"]
    async fn analyzes_real_screenshots() {
        dotenvy::dotenv().ok();

        let video = crate::tiktok::download("https://vm.tiktok.com/ZGdxtAF4f/", "unit-test-vision")
            .await
            .expect("download failed");
        let screenshots = crate::tiktok::capture_screenshots(&video, "unit-test-vision")
            .await
            .expect("screenshot capture failed");

        let client = reqwest::Client::new();
        let (analysis, _raw) = analyze_screenshots(&client, DEFAULT_VISION_MODEL, &screenshots)
            .await
            .expect("analysis failed");

        assert!(!analysis.summary.is_empty());
        assert!(!analysis.short_summary.is_empty());

        println!("summary: {}", analysis.summary);
        println!("short_summary: {}", analysis.short_summary);
        println!("on_screen_text: {}", analysis.on_screen_text);
        println!("topics: {:?}", analysis.topics);
    }
}
