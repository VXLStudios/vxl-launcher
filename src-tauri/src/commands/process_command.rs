use crate::error::CommandError;
use crate::state::process_state::{ProcessMetadata, CrashlogDto};
use crate::state::state_manager::State;
use tauri::Manager;
use uuid::Uuid;
use serde_json::json;

#[tauri::command]
pub async fn get_processes() -> Result<Vec<ProcessMetadata>, CommandError> {
    let state = State::get().await?;
    let processes = state.process_manager.list_processes().await;
    Ok(processes)
}

#[tauri::command]
pub async fn get_process(process_id: Uuid) -> Result<Option<ProcessMetadata>, CommandError> {
    let state = State::get().await?;
    let process = state.process_manager.get_process_metadata(process_id).await;
    Ok(process)
}

#[tauri::command]
pub async fn get_processes_by_profile(
    profile_id: Uuid,
) -> Result<Vec<ProcessMetadata>, CommandError> {
    let state = State::get().await?;
    let processes = state
        .process_manager
        .get_process_metadata_by_profile(profile_id)
        .await;
    Ok(processes)
}

#[tauri::command]
pub async fn stop_process(process_id: Uuid) -> Result<(), CommandError> {
    let state = State::get().await?;
    
    // Get the profile ID associated with this process
    let profile_id = state
        .process_manager
        .get_process_metadata(process_id)
        .await
        .map(|metadata| metadata.profile_id);
    
    // Stop the process
    state.process_manager.stop_process(process_id).await?;
    
    // Perform file sync push after profile closes (if sync is enabled for this profile)
    if let Some(profile_id) = profile_id {
        if let Err(e) = perform_profile_sync_push(profile_id).await {
            log::warn!("File sync push failed for profile {}: {}", profile_id, e);
            // Don't fail the stop, just warn
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn get_full_log(process_id: Uuid) -> Result<String, CommandError> {
    let state = State::get().await?;
    let log_content = state
        .process_manager
        .get_full_log_content(process_id)
        .await?;
    Ok(log_content)
}

#[tauri::command]
pub async fn open_log_window<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    process_id: Option<Uuid>,
    is_live_logs: Option<bool>,
) -> Result<(), CommandError> {
    let state = State::get().await?;
    let config = state.config_manager.get_config().await;
    let allow_multiple_windows = config.multiple_log_windows;

    let process_id_str = process_id.map(|id| id.to_string()).unwrap_or_else(|| "all".to_string());
    let window_label = format!("log_window_{}", process_id_str);

    // If multiple windows are disabled, check if any log window exists and focus it
    if !allow_multiple_windows {
        // Try to find any existing log window
        for window in app.webview_windows().values() {
            if window.label().starts_with("log_window_") {
                window.set_focus().map_err(|e| {
                    CommandError::from(crate::error::AppError::Other(format!(
                        "Failed to focus existing log window: {}",
                        e
                    )))
                })?;
                return Ok(());
            }
        }
    }

    // If the specific window already exists, focus it
    if let Some(window) = app.get_webview_window(&window_label) {
        window.set_focus().map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to focus existing log window {}: {}",
                window_label, e
            )))
        })?;
        return Ok(());
    }

    let is_live = is_live_logs.unwrap_or(false);
    let title = if let Some(id) = process_id {
        format!("Minecraft Logs ({})", id)
    } else {
        "Minecraft Logs".to_string()
    };

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        &window_label,
        tauri::WebviewUrl::App(
            "minecraft-log-window.html".into(),
        ),
    )
    .title(title)
    .inner_size(1200.0, 800.0)
    .decorations(false)
    .center()
    .build()
    .map_err(|e| CommandError::from(crate::error::AppError::Other(e.to_string())))?;

    Ok(())
}

#[tauri::command]
pub async fn open_single_log_window<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: String,
    instance_name: String,
    profile_id: String,
    account_name: Option<String>,
    start_time: f64,
) -> Result<(), CommandError> {
    let window_label = format!("single_log_window_{}", instance_id);

    if let Some(window) = app.get_webview_window(&window_label) {
        window.set_focus().map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to focus existing log window {}: {}",
                window_label, e
            )))
        })?;
        return Ok(());
    }

    let account_name_str = account_name.as_deref().unwrap_or("");
    let query_params = format!(
        "?instanceId={}&instanceName={}&profileId={}&accountName={}&startTime={}",
        urlencoding::encode(&instance_id),
        urlencoding::encode(&instance_name),
        urlencoding::encode(&profile_id),
        urlencoding::encode(account_name_str),
        start_time as i64
    );

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        &window_label,
        tauri::WebviewUrl::App(
            format!("single-log-window.html{}", query_params).into(),
        ),
    )
    .title(format!("{} - {}", instance_name, account_name.unwrap_or_else(|| "Logs".to_string())))
    .inner_size(1000.0, 700.0)
    .decorations(false)
    .center()
    .build()
    .map_err(|e| CommandError::from(crate::error::AppError::Other(e.to_string())))?;

    Ok(())
}

#[tauri::command]
pub async fn fetch_crash_report(profile_id: Uuid, process_id: Option<Uuid>) -> Result<Option<String>, CommandError> {
    let state = State::get().await?;
    let crash_content = state
        .process_manager
        .fetch_latest_crash_report(profile_id, process_id)
        .await?;
    Ok(crash_content)
}

#[tauri::command]
pub async fn set_discord_state(
    state_type: String,
    profile_name: Option<String>,
) -> Result<(), CommandError> {
    let state = State::get().await?;
    //TODO
    Ok(())
}

#[tauri::command]
pub async fn submit_crash_log_command(payload: CrashlogDto) -> Result<(), CommandError> {
    log::info!("Executing submit_crash_log_command");

    let raw_content = resolve_crash_log_content(&payload).await?;
    let sanitized_content = sanitize_crash_log_text(raw_content)?;

    // Prepare the payload for mclo.gs API
    let mclog_payload = json!({
        "content": sanitized_content
    });

    // Submit to mclo.gs API
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.mclo.gs/1/log")
        .json(&mclog_payload)
        .send()
        .await
        .map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to submit crash log to mclo.gs: {}",
                e
            )))
        })?;

    if !response.status().is_success() {
        return Err(CommandError::from(crate::error::AppError::Other(
            format!(
                "mclo.gs API returned error: {}",
                response.status()
            ),
        )));
    }

    log::info!("Successfully submitted crash log to mclo.gs");
    Ok(())
}

async fn resolve_crash_log_content(payload: &CrashlogDto) -> Result<String, CommandError> {
    if let Some(content) = payload
        .log_content
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Ok(content.to_string());
    }

    let Some(url) = payload
        .mc_logs_url
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
        return Err(CommandError::from(crate::error::AppError::Other(
            "Crash log submission requires log_content or an mclo.gs URL".to_string(),
        )));
    };

    if !crate::utils::download_url::is_allowed_mclogs_fetch_url(url) {
        return Err(CommandError::from(crate::error::AppError::Other(
            "Only mclo.gs log URLs may be fetched by the launcher".to_string(),
        )));
    }

    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| {
        CommandError::from(crate::error::AppError::Other(format!(
            "Failed to fetch crash log from URL: {}",
            e
        )))
    })?;

    response.text().await.map_err(|e| {
        CommandError::from(crate::error::AppError::Other(format!(
            "Failed to read crash log content: {}",
            e
        )))
    })
}

/// Sanitizes crash log content by removing sensitive information
fn sanitize_crash_log_text(mut content: String) -> Result<String, CommandError> {

    // First, extract and store all domains/IPs that appear in "Connecting to" lines
    // so we can redact them everywhere they appear in the log
    let mut redacted_addresses = Vec::new();
    
    // Match "Connecting to <address>, <port>" patterns
    // This pattern captures domains (including subdomains), IPs, and masked IPs like **.**.**.** 
    // The pattern captures everything up to the comma, including trailing periods
    if let Ok(connecting_regex) = regex::Regex::new(r"Connecting to\s+([a-zA-Z0-9\.\*\-]+\.?)\s*,\s*\d+") {
        for cap in connecting_regex.captures_iter(&content) {
            if let Some(address) = cap.get(1) {
                let addr_str = address.as_str().to_string();
                // Also add the version without trailing period for matching
                let addr_without_period = addr_str.trim_end_matches('.').to_string();
                redacted_addresses.push(addr_str.clone());
                if addr_without_period != addr_str {
                    redacted_addresses.push(addr_without_period);
                }
            }
        }
    }

    // IMPORTANT: Redact all extracted domains/IPs everywhere they appear in the log FIRST
    // This must be done before generic IP/pattern replacements to avoid conflicts
    for address in redacted_addresses {
        // Escape special regex characters in the address
        let escaped_address = regex::escape(&address);
        if let Ok(address_regex) = regex::Regex::new(&escaped_address) {
            content = address_regex
                .replace_all(&content, "[ADDRESS_REDACTED]")
                .to_string();
        }
    }

    // Remove sensitive data patterns
    // Remove file paths that might contain usernames
    content = regex::Regex::new(r"C:\\Users\\[^\\]+")
        .unwrap()
        .replace_all(&content, "C:\\Users\\[REDACTED]")
        .to_string();

    content = regex::Regex::new(r"/home/[^/]+")
        .unwrap()
        .replace_all(&content, "/home/[REDACTED]")
        .to_string();

    // Remove IP addresses
    content = regex::Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
        .unwrap()
        .replace_all(&content, "[IP_REDACTED]")
        .to_string();

    // Remove email addresses
    content = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
        .unwrap()
        .replace_all(&content, "[EMAIL_REDACTED]")
        .to_string();

    // Remove UUIDs that might be user identifiers (but keep some for context)
    // Only redact UUIDs that appear in suspicious contexts
    content = regex::Regex::new(r"(?i)(user|account|player)[-_]?id[:\s]*([a-f0-9\-]{36})")
        .unwrap()
        .replace_all(&content, "$1: [UUID_REDACTED]")
        .to_string();

    // Redact "Connecting to" lines completely (in case any addresses slip through)
    content = regex::Regex::new(r"Connecting to\s+[a-zA-Z0-9\.\*\-]+\s*,\s*\d+")
        .unwrap()
        .replace_all(&content, "Connecting to [ADDRESS_REDACTED], [PORT_REDACTED]")
        .to_string();

    Ok(content)
}

#[tauri::command]
pub async fn focus_main_window<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to show main window: {}",
                e
            )))
        })?;
        window.unminimize().map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to unminimize main window: {}",
                e
            )))
        })?;
        // Trick to bring window to front on Windows: temporarily set always on top
        let _ = window.set_always_on_top(true);
        let _ = window.set_always_on_top(false);
        window.set_focus().map_err(|e| {
            CommandError::from(crate::error::AppError::Other(format!(
                "Failed to focus main window: {}",
                e
            )))
        })?;
    }
    Ok(())
}

/// Helper function to perform file sync push for a profile when it closes
pub async fn perform_profile_sync_push(profile_id: uuid::Uuid) -> Result<(), String> {
    use crate::commands::file_sync_command::{load_sync_configs, push_to_hub};

    // Load all sync configs
    let configs = load_sync_configs()
        .map_err(|e| format!("Failed to load sync configs: {}", e))?;

    // Find configs that include this profile and push from it
    for config in configs {
        if !config.enabled {
            continue;
        }

        // Check if this profile is part of the sync config (either source or target)
        let is_part_of_config = if config.sync_all_profiles {
            // If sync_all_profiles is true, all profiles are part of it
            true
        } else {
            // Otherwise, check if profile is in the target list or is the source
            profile_id.to_string() == config.source_profile_id
                || config.profile_ids.iter().any(|id| id == &profile_id.to_string())
        };

        if is_part_of_config {
            log::info!(
                "Pushing files from profile {} to hub from sync config {}",
                profile_id, config.id
            );

            // Push files from this profile to hub
            push_to_hub(profile_id.to_string(), config.files_to_sync)
                .await
                .map_err(|e| format!("Failed to push files: {}", e))?;
        }
    }

    Ok(())
}
