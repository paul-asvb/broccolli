use serde_json::Value;

/// The `apify/website-content-crawler` actor, run synchronously for a single page.
const APIFY_CRAWLER_ACTOR_RUN_URL: &str =
    "https://api.apify.com/v2/acts/apify~website-content-crawler/run-sync-get-dataset-items";

/// Finds the first `http(s)://` URL in free-form message text.
pub fn find_url(text: &str) -> Option<&str> {
    let found = text
        .split_whitespace()
        .find(|token| token.starts_with("http://") || token.starts_with("https://"));
    log::debug!("find_url on {} chars of text -> {found:?}", text.len());
    found
}

/// Crawls a single page via the `apify/website-content-crawler` Apify actor and returns the
/// raw dataset item as JSON (fields include `url`, `text`, and `metadata.title`).
/// Requires `APIFY_API_TOKEN` to be set.
pub async fn crawl(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let token = std::env::var("APIFY_API_TOKEN").map_err(|_| "APIFY_API_TOKEN not set".to_string())?;

    log::debug!("crawling web article via apify for {url}");

    let body = serde_json::json!({
        "startUrls": [{"url": url}],
        "maxCrawlPages": 1,
        "crawlerType": "cheerio",
    });

    let resp = client
        .post(APIFY_CRAWLER_ACTOR_RUN_URL)
        .query(&[("token", token.as_str())])
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("apify request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("apify returned {status}: {text}"));
    }

    let items: Vec<Value> = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse apify response: {e}"))?;

    log::debug!("apify returned {} dataset item(s)", items.len());

    items
        .into_iter()
        .next()
        .ok_or_else(|| "apify returned no content for this page".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_url_among_other_text() {
        let text = "check this out https://example.com/article lol";
        assert_eq!(find_url(text), Some("https://example.com/article"));
        assert_eq!(find_url("no link here"), None);
    }

    #[tokio::test]
    #[ignore = "hits the live Apify API; requires APIFY_API_TOKEN"]
    async fn crawls_a_real_page() {
        dotenvy::dotenv().ok();

        let client = reqwest::Client::new();
        let item = crawl(&client, "https://example.com/").await.expect("crawl failed");

        assert!(item["text"].as_str().is_some_and(|t| !t.is_empty()));
        println!("crawled: {item}");
    }
}
