use reqwest::header::{HeaderValue, USER_AGENT};
use serde_json::Value;
use std::env;
use std::time::Duration;
use rust_i18n::t;
const REPO: &str = "VLAZOF/dgrlauncher";
fn api_url() -> String {
    format!("https://api.github.com/repos/{REPO}/releases/latest")
}
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v').trim_start_matches('V');
    let s = s.split(['-', '+']).next().unwrap_or(s);
    let mut parts = s.split('.');
    let major = parts.next()?.trim().parse::<u64>().ok()?;
    let minor = parts
        .next()
        .map(|p| p.trim().parse::<u64>())
        .transpose()
        .ok()?
        .unwrap_or(0);
    let patch = parts
        .next()
        .map(|p| p.trim().parse::<u64>())
        .transpose()
        .ok()?
        .unwrap_or(0);
    Some((major, minor, patch))
}
pub async fn check_launcher_updates() -> Result<(String, String, String), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| t!("upd.http_client", err = e.to_string()).to_string())?;
    let mut request = client
        .get(api_url())
        .header(USER_AGENT, HeaderValue::from_static("dgrlauncher"));
    if let Ok(token) = env::var("GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            let value = format!("Bearer {}", token.trim());
            if let Ok(header) = HeaderValue::from_str(&value) {
                request = request.header("Authorization", header);
            }
        }
    }
    let response = request
        .send()
        .await
        .map_err(|e| t!("upd.check_fail", err = e.to_string()).to_string())?;
    if !response.status().is_success() {
        return Err(t!("upd.http_status", status = response.status()).to_string());
    }
    let text = response
        .text()
        .await
        .map_err(|e| t!("upd.read_response", err = e.to_string()).to_string())?;
    let release: Value =
        serde_json::from_str(&text).map_err(|e| t!("upd.read_json", err = e.to_string()).to_string())?;
    let latest_tag = release["tag_name"]
        .as_str()
        .ok_or_else(|| t!("upd.no_tag").to_string())?;
    let current_version = env!("CARGO_PKG_VERSION");
    let current = parse_version(current_version)
        .ok_or_else(|| t!("upd.parse_current").to_string())?;
    let latest = parse_version(latest_tag)
        .ok_or_else(|| t!("upd.parse_latest").to_string())?;
    if latest <= current {
        return Err(t!("upd.uptodate").to_string());
    }
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| t!("upd.no_assets").to_string())?;
    let (want_substr, want_ext) = match env::consts::OS {
        "windows" => ("windows", ".zip"),
        "linux" => ("linux", ".tar.gz"),
        other => return Err(t!("upd.os_unsupported", os = other).to_string()),
    };
    let mut url: Option<String> = None;
    for pass in 0..2 {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("").to_lowercase();
            let matches = if pass == 0 {
                name.contains(want_substr) && name.ends_with(want_ext)
            } else {
                name.contains(want_substr)
            };
            if matches {
                if let Some(u) = asset["browser_download_url"].as_str() {
                    url = Some(u.to_string());
                    break;
                }
            }
        }
        if url.is_some() {
            break;
        }
    }
    match url {
        Some(url) => {
            let body = release["body"].as_str().unwrap_or("").to_owned();
            Ok((url, latest_tag.to_string(), clean_release_notes(&body)))
        }
        None => Err(t!(
            "upd.no_asset",
            os = env::consts::OS,
            tag = latest_tag
        )
        .to_string()),
    }
}
/// Release body for the Settings notes box: the auto-generated
/// "Full Changelog" link line carries no info, drop it.
fn clean_release_notes(body: &str) -> String {
    body.lines()
        .filter(|line| !line.trim_start().starts_with("**Full Changelog**"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}
#[cfg(test)]
mod tests {
    use super::parse_version;
    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("0.7.1"), Some((0, 7, 1)));
        assert_eq!(parse_version("v0.7.1"), Some((0, 7, 1)));
        assert_eq!(parse_version("V0.7.10"), Some((0, 7, 10)));
        assert_eq!(parse_version("0.10.0"), Some((0, 10, 0)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert!((0, 7, 10) > (0, 7, 9));
        assert!((0, 10, 0) > (0, 9, 9));
    }
}
