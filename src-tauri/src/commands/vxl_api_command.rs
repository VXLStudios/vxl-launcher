use serde_json::Value;
use tauri::command;

use crate::error::{AppError, CommandError};

const VXL_API_BASE: &str = "https://api.voxelstudios.co.uk";

/// Proxy GET requests to api.voxelstudios.co.uk from Rust (avoids browser CORS in the Tauri webview).
#[command]
pub async fn fetch_vxl_api_json(path: String) -> Result<Value, CommandError> {
    let path = path.trim();
    if !path.starts_with('/') || path.contains("..") {
        return Err(CommandError::from(AppError::Config(
            "Invalid VXL API path".to_string(),
        )));
    }

    let url = format!("{VXL_API_BASE}{path}");
    let response = reqwest::get(&url).await.map_err(AppError::MinecraftApi)?;

    if !response.status().is_success() {
        return Err(CommandError::from(AppError::Config(format!(
            "VXL API returned status: {}",
            response.status()
        ))));
    }

    response.json().await.map_err(AppError::MinecraftApi).map_err(Into::into)
}
