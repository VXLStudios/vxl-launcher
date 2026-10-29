use serde::{Deserialize, Serialize};
use tauri::command;
use crate::error::{AppError, CommandError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub version: String,
    pub date: String,
    pub changes: Vec<String>,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub improvements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogResponse {
    pub current_version: String,
    pub entries: Vec<ChangelogEntry>,
}

/// Fetch changelog data for the current launcher version
#[command]
pub async fn fetch_changelog_command() -> Result<ChangelogResponse, CommandError> {
    // Fetch changelog from the API
    let launcher_version = concat!("v", env!("CARGO_PKG_VERSION"));
    let api_url = format!("https://api.voxelstudios.co.uk/v1/launcher/changelog?launcher_version={}", launcher_version);
    
    let response = reqwest::get(&api_url).await.map_err(|e| {
        AppError::MinecraftApi(e)
    })?;
    
    if !response.status().is_success() {
        return Err(CommandError::from(AppError::Config(
            format!("API returned status: {}", response.status())
        )));
    }
    
    let changelog_data: ChangelogResponse = response.json().await.map_err(|e| {
        AppError::MinecraftApi(e)
    })?;
    
    Ok(changelog_data)
}

/// Fetch changelog for a specific version
#[command]
pub async fn fetch_version_changelog_command(version: String) -> Result<Option<ChangelogEntry>, CommandError> {
    let changelog_data = fetch_changelog_command().await?;
    
    let entry = changelog_data
        .entries
        .into_iter()
        .find(|entry| entry.version == version);
    
    Ok(entry)
}
