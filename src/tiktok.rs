use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

const TEMP_SUBDIR: &str = "broccolli-tiktok";

/// The `apidojo/tiktok-scraper` actor — pay-per-result ($0.0002/post), no monthly rental.
const APIFY_METADATA_ACTOR_RUN_URL: &str =
    "https://api.apify.com/v2/acts/5K30i8aFccKNF5ICs/run-sync-get-dataset-items";

/// Downloads a TikTok video (including `vm.tiktok.com` short links) by shelling out to
/// the `yt-dlp` binary, which must be installed and on `$PATH` (see the Dockerfile).
/// Saves directly to `$TMPDIR/broccolli-tiktok/<name>.mp4` and returns that path.
/// `name` should be a caller-controlled, filesystem-safe identifier (e.g. a message id).
pub async fn download(tiktok_url: &str, name: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(TEMP_SUBDIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {e}"))?;
    let path = dir.join(format!("{name}.mp4"));

    log::debug!("downloading tiktok video via yt-dlp: {tiktok_url} -> {}", path.display());

    let output = Command::new("yt-dlp")
        .arg("--no-playlist")
        .arg("-o")
        .arg(&path)
        .arg(tiktok_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("failed to spawn yt-dlp (is it installed and on $PATH?): {e}"))?;

    log::debug!(
        "yt-dlp exited with {}; stdout: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout).trim()
    );

    if !output.status.success() {
        return Err(format!(
            "yt-dlp exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    if !path.exists() {
        return Err(format!(
            "yt-dlp reported success but {} does not exist",
            path.display()
        ));
    }

    log::debug!("downloaded tiktok video to {}", path.display());

    Ok(path)
}

/// Fetches TikTok video metadata (caption, stats, author, hashtags, etc.) via the
/// `apidojo/tiktok-scraper` Apify actor and returns the raw dataset item as JSON.
/// Requires `APIFY_API_TOKEN` to be set.
pub async fn fetch_metadata(client: &reqwest::Client, tiktok_url: &str) -> Result<Value, String> {
    let token = std::env::var("APIFY_API_TOKEN").map_err(|_| "APIFY_API_TOKEN not set".to_string())?;

    log::debug!("fetching tiktok metadata via apify for {tiktok_url}");

    let body = serde_json::json!({
        "startUrls": [tiktok_url],
        "maxItems": 1,
    });

    let resp = client
        .post(APIFY_METADATA_ACTOR_RUN_URL)
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
        .ok_or_else(|| "apify returned no metadata for this video".to_string())
}

/// Writes metadata into `$TMPDIR/broccolli-tiktok/<name>.json`, returning the file path.
pub fn save_metadata_to_temp(metadata: &Value, name: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(TEMP_SUBDIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

    let path = dir.join(format!("{name}.json"));
    let pretty = serde_json::to_vec_pretty(metadata).map_err(|e| format!("failed to serialize metadata: {e}"))?;
    std::fs::write(&path, pretty).map_err(|e| format!("failed to write metadata file: {e}"))?;

    log::debug!("saved tiktok metadata to {}", path.display());

    Ok(path)
}

/// Finds the first TikTok URL in free-form message text.
pub fn find_url(text: &str) -> Option<&str> {
    let found = text.split_whitespace().find(|token| token.contains("tiktok.com"));
    log::debug!("find_url on {} chars of text -> {found:?}", text.len());
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "hits the live TikTok CDN via yt-dlp; leaves the result in tmp/ for manual inspection"]
    async fn downloads_a_real_video() {
        let downloaded = download("https://vm.tiktok.com/ZGdxtAF4f/", "unit-test-download")
            .await
            .expect("download failed");

        // Copy into a repo-local tmp/ dir (gitignored) instead of the OS temp dir, so the
        // result survives the test run and can be opened/played directly for inspection.
        let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp");
        std::fs::create_dir_all(&repo_tmp).expect("failed to create repo tmp dir");
        let dest = repo_tmp.join("unit-test-download.mp4");
        std::fs::copy(&downloaded, &dest).expect("failed to copy into repo tmp dir");

        let metadata = std::fs::metadata(&dest).expect("copied file missing");
        assert!(metadata.len() > 100_000);

        println!("downloaded tiktok video for inspection: {}", dest.display());
    }

    #[tokio::test]
    #[ignore = "hits the live Apify API; requires APIFY_API_TOKEN; leaves the result in tmp/ for manual inspection"]
    async fn fetches_real_metadata() {
        dotenvy::dotenv().ok();

        let client = reqwest::Client::new();
        let metadata = fetch_metadata(&client, "https://vm.tiktok.com/ZGdxtAF4f/")
            .await
            .expect("fetch failed");

        assert_eq!(metadata["id"].as_str(), Some("7677667260730838294"));
        assert!(metadata["views"].as_i64().unwrap_or(0) > 0);

        let repo_tmp = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp");
        std::fs::create_dir_all(&repo_tmp).expect("failed to create repo tmp dir");
        let dest = repo_tmp.join("unit-test-metadata.json");
        std::fs::write(&dest, serde_json::to_vec_pretty(&metadata).unwrap()).expect("failed to write metadata");

        println!("fetched tiktok metadata for inspection: {}", dest.display());
    }

    #[test]
    fn finds_tiktok_url_among_other_text() {
        let text = "check this out https://vm.tiktok.com/ZGdxtAF4f/ lol";
        assert_eq!(find_url(text), Some("https://vm.tiktok.com/ZGdxtAF4f/"));
        assert_eq!(find_url("no link here"), None);
    }
}
