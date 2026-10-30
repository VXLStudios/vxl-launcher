use crate::error::{AppError, CommandError};
use crate::state::{config_state::LauncherConfig, State};
use tauri::command;
use tauri::AppHandle;

type Result<T> = std::result::Result<T, CommandError>;

#[command]
pub async fn get_launcher_config() -> Result<LauncherConfig> {
    let state = State::get().await?;
    let config = state.config_manager.get_config().await;
    Ok(config)
}

#[command]
pub async fn set_launcher_config(config: LauncherConfig) -> Result<LauncherConfig> {
    let state = State::get().await?;

    // Validate concurrent downloads value
    if config.concurrent_downloads == 0 || config.concurrent_downloads > 10 {
        return Err(CommandError::from(AppError::Other(format!(
            "Concurrent downloads must be between 1 and 10 (got: {})",
            config.concurrent_downloads
        ))));
    }

    let mut config = config;
    if let Some(ref custom_dir) = config.custom_game_directory {
        let validated =
            crate::utils::path_security::validate_custom_game_directory(custom_dir)?;
        config.custom_game_directory = Some(validated);
    }

    // Set the entire configuration
    state.config_manager.set_config(config.clone()).await?;

    // Return the updated config
    Ok(config)
}

#[command]
pub fn get_app_version(app_handle: AppHandle) -> Result<String> {
    Ok(app_handle.package_info().version.to_string())
}

#[command]
pub fn is_launcher_ready() -> bool {
    crate::utils::splash_utils::is_app_ready()
}
