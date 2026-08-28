use serde::Deserialize;
use std::path::PathBuf;

const TIKWM_API_URL: &str = "https://www.tikwm.com/api/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct TikwmResponse {
    code: i64,
    msg: String,
    data: Option<TikwmData>,
}

#[derive(Debug, Deserialize)]
struct TikwmData {
    title: String,
    play: String,
}

pub struct TiktokVideo {
    pub title: String,
    pub bytes: Vec<u8>,
}

/// Resolves a TikTok URL (including `vm.tiktok.com` short links) via the tikwm.com
/// API — an unofficial, no-auth mirror of TikTok's own resolver — to a no-watermark
/// CDN link, then downloads the video. tikwm's CDN links aren't IP-locked to the
/// resolving caller (unlike some scraper services), so this works as a single
/// self-contained step.
pub async fn download(client: &reqwest::Client, tiktok_url: &str) -> Result<TiktokVideo, String> {
    let resp = client
        .get(TIKWM_API_URL)
        .query(&[("url", tiktok_url)])
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("tikwm request failed: {e}"))?;

    let parsed: TikwmResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse tikwm response: {e}"))?;

    if parsed.code != 0 {
        return Err(format!("tikwm returned error {}: {}", parsed.code, parsed.msg));
    }
    let data = parsed
        .data
        .ok_or_else(|| "tikwm response missing data".to_string())?;

    let video_resp = client
        .get(&data.play)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("video download request failed: {e}"))?;

    if !video_resp.status().is_success() {
        return Err(format!("video download failed with status {}", video_resp.status()));
    }

    let bytes = video_resp
        .bytes()
        .await
        .map_err(|e| format!("failed to read video bytes: {e}"))?
        .to_vec();

    Ok(TiktokVideo {
        title: data.title,
        bytes,
    })
}

/// Writes the video into `$TMPDIR/broccolli-tiktok/<name>.mp4`, returning the file path.
/// `name` should be a caller-controlled, filesystem-safe identifier (e.g. a message id)
/// rather than the video's title, which can contain arbitrary text/emoji.
pub fn save_to_temp(video: &TiktokVideo, name: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("broccolli-tiktok");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

    let path = dir.join(format!("{name}.mp4"));
    std::fs::write(&path, &video.bytes).map_err(|e| format!("failed to write video file: {e}"))?;

    Ok(path)
}

/// Finds the first TikTok URL in free-form message text.
pub fn find_url(text: &str) -> Option<&str> {
    text.split_whitespace()
        .find(|token| token.contains("tiktok.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "hits the live tikwm.com API and TikTok's CDN"]
    async fn downloads_a_real_video() {
        let client = reqwest::Client::new();
        let video = download(&client, "https://vm.tiktok.com/ZGdxtAF4f/")
            .await
            .expect("download failed");

        assert!(!video.title.is_empty());
        assert!(video.bytes.len() > 100_000);
    }

    #[test]
    fn finds_tiktok_url_among_other_text() {
        let text = "check this out https://vm.tiktok.com/ZGdxtAF4f/ lol";
        assert_eq!(find_url(text), Some("https://vm.tiktok.com/ZGdxtAF4f/"));
        assert_eq!(find_url("no link here"), None);
    }

    #[test]
    fn saves_video_bytes_to_temp_dir() {
        let video = TiktokVideo {
            title: "test".to_string(),
            bytes: vec![1, 2, 3, 4],
        };
        let path = save_to_temp(&video, "unit-test-save").expect("save failed");
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3, 4]);
        std::fs::remove_file(&path).ok();
    }
}
