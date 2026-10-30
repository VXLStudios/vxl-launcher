use crate::error::{AppError, Result};
use log::warn;
use url::Url;

/// Hosts allowed for content/mod downloads initiated from IPC payloads (Modrinth, CurseForge, GitHub releases).
fn host_allowed_for_content_download(host: &str) -> bool {
    if host == "cdn.modrinth.com" {
        return true;
    }
    if host == "modrinth.com" || host.ends_with(".modrinth.com") {
        return true;
    }
    if host == "media.forgecdn.net" || host == "edge.forgecdn.net" {
        return true;
    }
    if host == "github.com"
        || host == "objects.githubusercontent.com"
        || host == "raw.githubusercontent.com"
    {
        return true;
    }
    false
}

/// Rejects non-HTTPS and untrusted hosts to limit SSRF from renderer-supplied download URLs.
pub fn validate_trusted_content_download_url(url_str: &str) -> Result<()> {
    let parsed = Url::parse(url_str).map_err(|e| {
        AppError::Other(format!("Invalid download URL: {}", e))
    })?;

    if parsed.scheme() != "https" {
        return Err(AppError::Other(
            "Download URLs must use HTTPS".to_string(),
        ));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Other("Download URL has no host".to_string()))?;

    if host_allowed_for_content_download(host) {
        return Ok(());
    }

    warn!("Rejected download URL with disallowed host: {}", host);
    Err(AppError::Other(format!(
        "Download host is not allowed: {}",
        host
    )))
}

/// Returns true if the URL may be fetched when re-loading an mclo.gs log for crash submission.
pub fn is_allowed_mclogs_fetch_url(url_str: &str) -> bool {
    let Ok(parsed) = Url::parse(url_str) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    match parsed.host_str() {
        Some("api.mclo.gs") | Some("mclo.gs") | Some("www.mclo.gs") => true,
        _ => false,
    }
}
