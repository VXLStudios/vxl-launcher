// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod config;
mod error;
pub mod integrations;
mod logging;
mod minecraft;
mod state;
mod utils;

use log::{debug, error, info};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Listener;
use tauri::Manager;
use utils::debug_utils;
use utils::splash_utils;
use utils::updater_utils;

use crate::commands::process_command::{
    fetch_crash_report, focus_main_window, get_full_log, get_process, get_processes, get_processes_by_profile,
    open_log_window, set_discord_state, stop_process,
};
use commands::minecraft_auth_command::{
    begin_login, cancel_login, get_accounts, get_active_account, is_flatpak, remove_account, set_active_account,
};
use commands::minecraft_command::{
    add_skin,
    add_skin_locally,
    apply_skin_from_base64,
    fetch_crafty_gg_skin_texture,
    // Local skin database commands
    get_all_skins,
    get_base64_from_skin_source_command,
    get_base64_with_metadata_from_skin_source_command,
    get_crafatar_avatar,
    get_fabric_loader_versions,
    get_forge_versions,
    get_minecraft_versions,
    get_neoforge_versions,
    get_quilt_loader_versions,
    get_skin_by_id,
    get_starlight_skin_render,
    // Skin management commands
    get_user_skin_data,
    ping_minecraft_server,
    remove_skin,
    reorder_skins,
    reset_skin,
    update_skin_properties,
    upload_log_to_mclogs_command,
    upload_skin,
};
use commands::profile_command::{
    abort_profile_launch, add_modrinth_content_to_profile, add_modrinth_mod_to_profile,
    batch_check_content_installed, check_for_group_migration_command, check_world_lock_status, copy_profile, copy_world,
    create_profile, delete_custom_mod, delete_mod_from_profile, delete_profile, delete_world,
    export_profile, get_all_profiles_and_last_played, get_custom_mods, get_local_content,
    get_local_datapacks, get_local_resourcepacks, get_local_shaderpacks, get_log_file_content,
    get_profile, get_profile_directory_structure,
    get_profile_latest_log_content, get_profile_log_files, get_servers_for_profile,
    get_system_ram_mb, get_worlds_for_profile, import_local_mods,
    import_profile, import_profile_from_file, import_world, is_content_installed, is_profile_launching,
    launch_profile, list_profile_screenshots, list_profiles, open_profile_folder, open_world_folder,
    open_profile_latest_log, repair_profile,
    resolve_loader_version, search_profiles, set_custom_mod_enabled,
    set_profile_mod_enabled, update_datapack_from_modrinth, update_modrinth_mod_version,
    update_profile, update_resourcepack_from_modrinth, update_shaderpack_from_modrinth,
};

use commands::skin_preview_command::{get_skin_preview_path, save_skin_preview};
use commands::skin_render_command::{get_cached_skin_render, store_skin_render};
use commands::cosmetic_command::get_random_local_emote;

// Use statements for registered commands only
use commands::curseforge_commands::{get_curseforge_mods_by_ids, import_curseforge_pack, download_and_install_curseforge_modpack_command, get_curseforge_file_changelog_command, get_curseforge_mod_description_command};

use commands::modrinth_commands::{
    check_modrinth_updates, check_mod_updates_unified_command, download_and_install_modrinth_modpack,
    get_all_modrinth_versions_for_contexts, get_modrinth_categories_command,
    get_modrinth_game_versions_command, get_modrinth_loaders_command, get_modrinth_mod_versions,
    get_modpack_versions_unified_command, get_modrinth_project_details, get_modrinth_project_members, get_modrinth_versions_by_hashes, search_modrinth_mods,
    search_modrinth_projects, search_mods_unified_command, get_mod_versions_unified_command,
    switch_modpack_version_command
};

use commands::file_command::{
    delete_file, get_file_size_bytes, get_icons_for_archives, import_custom_background, open_file, open_file_directory,
    read_file_bytes, set_file_enabled,
};

use commands::file_sync_command::{
    get_file_sync_configs,
    create_file_sync_config, update_file_sync_config, delete_file_sync_config,
    get_file_sync_status, check_profile_file_exists,
    pull_from_hub, push_to_hub, get_shared_files,
};

// Import config commands
use commands::config_commands::{get_app_version, get_launcher_config, is_launcher_ready, set_launcher_config};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

// Import path commands
use commands::path_commands::{get_launcher_directory, resolve_image_path};

// Import vanilla cape commands
use commands::vanilla_cape_command::{
    get_cached_cape_texture_path, get_cape_preview_path, get_currently_equipped_vanilla_cape,
    get_owned_vanilla_capes, get_vanilla_cape_info, equip_vanilla_cape, refresh_vanilla_cape_data,
    save_cape_preview, sync_cape_texture_cache,
};

// Import NRC commands
use commands::nrc_commands::{check_update_available_command, download_and_install_update_command};

// Import Content commands
use commands::content_command::{
    bulk_toggle_mod_updates, install_content_to_profile, install_local_content_to_profile,
    switch_content_version, toggle_content_from_profile, toggle_mod_updates,
    uninstall_content_from_profile,
};

// Import Java commands
use commands::java_command::{
    detect_java_installations_command, find_best_java_for_minecraft_command, get_java_info_command,
    invalidate_java_cache_command, validate_java_path_command,
};

// Import Discord commands
use commands::discord_command::{
    set_discord_state_browsing_library, set_discord_state_browsing_vxl_studios,
    set_discord_state_browsing_modded_content, set_discord_state_playing, set_discord_state_idle,
    set_discord_state_getting_ready_to_play, set_discord_state_browsing_outfits,
    set_discord_state_browsing_capes, set_discord_state_tinkering,
};

#[tokio::main]
async fn main() {
    if let Err(e) = logging::setup_logging().await {
        eprintln!("FEHLER: Logging konnte nicht initialisiert werden: {}", e);
    }

    info!("Starting VXL Launcher...");

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            info!("SingleInstance plugin: Second instance triggered with args: {:?}", argv);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // Focus the main window on second instance
            /*if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize(); // Ensure it's not minimized
                let _ = window.set_focus();   // Bring to front and focus
            }
            // Call the handler for .vxlpack files
            let app_handle_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                norisk_packs::handle_noriskpack_file_paths(&app_handle_clone, argv).await;
            });*/
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            splash_utils::show_main_window(&app_handle);

            // --- Initialize System Tray (Tauri 2.0) ---
            let show_item = MenuItem::with_id(app, "show", "Show VXL Launcher", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("VXL Launcher")
                .icon(app.default_window_icon().unwrap().clone())
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } => {
                        // Beim Klick auf das Tray-Icon das Fenster anzeigen/verstecken
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            let is_minimized = window.is_minimized().unwrap_or(false);
                            
                            if is_visible && !is_minimized {
                                // Fenster ist sichtbar und nicht minimiert -> verstecken
                                let _ = window.hide();
                            } else {
                                // Fenster ist versteckt oder minimiert -> anzeigen
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                    TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        // Doppelklick zeigt immer das Fenster
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            // --- Handle .vxlpack file opening on initial startup (all platforms) ---
            // The single-instance plugin does not handle the *very first* launch with arguments.
            // We still need to check std::env::args() here for that first launch.
            /*info!("Checking for startup file arguments...");
            let startup_args: Vec<String> = std::env::args().collect();
            if startup_args.len() > 1 { // args[0] is exe path, check if there are more
                let handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    // Pass all startup_args; handle_noriskpack_file_paths will skip the exe path if needed
                    norisk_packs::handle_noriskpack_file_paths(&handle_clone, startup_args).await;
                });
            }*/
            // --- End .vxlpack handling on startup ---

            // Task for State Init and Updater Window
            let state_init_app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // --- Create Updater Window (but keep hidden initially) ---
                let updater_window = match updater_utils::create_updater_window(&state_init_app_handle).await {
                    Ok(win) => {
                        info!("Updater window created successfully (initially hidden).");
                        Some(win)
                    }
                    Err(e) => {
                        error!("Failed to create updater window: {}", e);
                        None
                    }
                };

                // --- State Initialization --- 
                info!("Initiating state initialization...");
                if let Err(e) = state::state_manager::State::init(Arc::new(state_init_app_handle.clone())).await {
                    error!("CRITICAL: Failed to initialize state: {}. Update check and main window might not proceed correctly.", e);
                    if let Some(win) = updater_window {
                        updater_utils::emit_status(&state_init_app_handle, "close", "Closing due to state init error.".to_string(), None);
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        if let Err(close_err) = win.close() {
                            error!("Failed to close updater window after state init error: {}", close_err);
                        }
                    }
                    splash_utils::emit_app_ready(&state_init_app_handle);
                    return;
                }
                info!("State initialization finished successfully.");

                splash_utils::emit_app_ready(&state_init_app_handle);

                info!("Attempting to retrieve launcher configuration for update check...");
                match state::state_manager::State::get().await {
                    Ok(state_manager_instance) => {
                        let config = state_manager_instance.config_manager.get_config().await;
                        let check_beta_channel = config.check_beta_channel;
                        let mut auto_check_updates_enabled = config.auto_check_updates;

                        // Disable auto-updates when running in Flatpak
                        if updater_utils::is_flatpak() {
                            info!("Running in Flatpak environment - disabling automatic updates (Flatpak handles updates through its own mechanism).");
                            auto_check_updates_enabled = false;
                        }

                        if auto_check_updates_enabled {
                            info!("Initiating application update check (Channel determined by config: Beta={})...", check_beta_channel);
                            updater_utils::check_for_updates(state_init_app_handle.clone(), check_beta_channel, updater_window.clone()).await;
                            info!("Update check process has finished.");
                        } else {
                            info!("Auto-check for updates is disabled in settings. Skipping update check.");
                            // Ensure the updater window (if created) is closed if we skip the check.
                            if let Some(win) = updater_window {
                                updater_utils::emit_status(&state_init_app_handle, "close", "Auto-update disabled.".to_string(), None);
                                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; // Give time for emit to process
                                if let Err(close_err) = win.close() {
                                    error!("Failed to close updater window when skipping updates: {}", close_err);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get global state for update check: {}.", e);
                        if let Some(win) = updater_window { 
                            updater_utils::emit_status(&state_init_app_handle, "close", "Closing due to state fetch error.".to_string(), None);
                            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                            if let Err(close_err) = win.close() {
                                error!("Failed to close updater window after state fetch error: {}", close_err);
                            }
                        }
                    }
                }

                // --- Test Unified Mod Search ---
                //debug_utils::debug_unified_mod_search().await;

                // --- Test Unified Mod Versions ---
                //debug_utils::debug_unified_mod_versions().await;
            });

            // --- Register Focus/Blur Event Listeners for Discord RPC --- 
            if let Some(main_window) = app.get_webview_window("main") { 
                // Handle focus event (window gains focus)
                let focus_app_handle = app_handle.clone(); 
                main_window.listen("tauri://focus", move |_event| {
                    let listener_app_handle = focus_app_handle.clone(); 
                    tokio::spawn(async move {
                        // Only log debug if Discord is enabled to reduce noise
                        match state::state_manager::State::get().await {
                            Ok(state_manager_instance) => { 
                                if state_manager_instance.discord_manager.is_enabled().await {
                                    debug!("Main window focused - restoring Discord state");
                                }
                                if let Err(e) = state_manager_instance.discord_manager.handle_focus_event().await {
                                    error!("Error during DiscordManager focus handling: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Focus event listener: Failed to get global state using State::get(): {}", e);
                            }
                        }
                    });
                });

                // Handle blur event (window loses focus)
                let blur_app_handle = app_handle.clone();
                main_window.listen("tauri://blur", move |_event| {
                    let listener_app_handle = blur_app_handle.clone();
                    tokio::spawn(async move {
                        match state::state_manager::State::get().await {
                            Ok(state_manager_instance) => {
                                if state_manager_instance.discord_manager.is_enabled().await {
                                    debug!("Main window blurred - setting Discord to Idle");
                                }
                                if let Err(e) = state_manager_instance.discord_manager.handle_blur_event().await {
                                    error!("Error during DiscordManager blur handling: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Blur event listener: Failed to get global state using State::get(): {}", e);
                            }
                        }
                    });
                });

                // --- Handle window close request (from taskbar, etc.) ---
                main_window.listen("tauri://close-requested", move |_event| {
                    info!("Window close requested via system (taskbar, etc.). Exiting application.");
                    std::process::exit(0);
                });
            } else {
                error!("Could not get main window handle to attach focus listener!");
            }


            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_profile,
            get_profile,
            update_profile,
            delete_profile,
            repair_profile,
            resolve_loader_version,
            list_profiles,
            search_profiles,
            get_minecraft_versions,
            launch_profile,
            abort_profile_launch,
            is_profile_launching,
            get_processes,
            get_process,
            get_processes_by_profile,
            stop_process,
            open_log_window,
            focus_main_window,
            begin_login,
            cancel_login,
            is_flatpak,
            remove_account,
            get_active_account,
            set_active_account,
            get_accounts,
            search_modrinth_mods,
            search_modrinth_projects,
            search_mods_unified_command,
            get_mod_versions_unified_command,
            get_modpack_versions_unified_command,
            get_curseforge_mods_by_ids,
            import_curseforge_pack,
            download_and_install_curseforge_modpack_command,
            get_curseforge_file_changelog_command,
            get_curseforge_mod_description_command,
            get_modrinth_mod_versions,
            add_modrinth_mod_to_profile,
            add_modrinth_content_to_profile,
            get_modrinth_project_details,
            get_modrinth_project_members,
            check_modrinth_updates,
            check_mod_updates_unified_command,
            get_icons_for_archives,
            set_profile_mod_enabled,
            delete_mod_from_profile,
            update_modrinth_mod_version,
            get_all_modrinth_versions_for_contexts,
            get_full_log,
            fetch_crash_report,
            get_custom_mods,
            get_local_resourcepacks,
            get_local_shaderpacks,
            get_local_datapacks,
            set_custom_mod_enabled,
            import_local_mods,
            get_system_ram_mb,
            delete_custom_mod,
            open_profile_folder,
            open_world_folder,
            import_profile_from_file,
            import_profile, 
            upload_log_to_mclogs_command,
            get_fabric_loader_versions,
            get_forge_versions,
            get_neoforge_versions,
            get_quilt_loader_versions,
            set_file_enabled,
            delete_file,
            open_file_directory,
            download_and_install_modrinth_modpack,
            get_profile_directory_structure,
            copy_profile,
            export_profile,
            get_launcher_config,
            set_launcher_config,
            get_launcher_directory,
            resolve_image_path,
            commands::path_commands::upload_profile_images,
            update_resourcepack_from_modrinth,
            update_shaderpack_from_modrinth,
            update_datapack_from_modrinth,
            get_user_skin_data,
            upload_skin,
            reset_skin,
            apply_skin_from_base64,
            get_all_skins,
            get_skin_by_id,
            add_skin,
            remove_skin,
            update_skin_properties,
            get_base64_from_skin_source_command,
            get_base64_with_metadata_from_skin_source_command,
            fetch_crafty_gg_skin_texture,
            get_skin_preview_path,
            save_skin_preview,
            get_cached_skin_render,
            store_skin_render,
            get_random_local_emote,
            set_discord_state,
            get_owned_vanilla_capes,
            get_currently_equipped_vanilla_cape,
            equip_vanilla_cape,
            get_vanilla_cape_info,
            refresh_vanilla_cape_data,
            sync_cape_texture_cache,
            get_cached_cape_texture_path,
            get_cape_preview_path,
            save_cape_preview,
            is_content_installed,
            batch_check_content_installed,
            check_for_group_migration_command,
            open_profile_latest_log,
            get_profile_latest_log_content,
            detect_java_installations_command,
            get_java_info_command,
            find_best_java_for_minecraft_command,
            invalidate_java_cache_command,
            validate_java_path_command,
            get_worlds_for_profile,
            get_servers_for_profile,
            copy_world,
            import_world,
            check_world_lock_status,
            ping_minecraft_server,
            delete_world,
            get_profile_log_files,
            get_log_file_content,
            list_profile_screenshots,
            open_file,
            read_file_bytes,
            get_file_size_bytes,
            import_custom_background,
            get_app_version,
            is_launcher_ready,
            check_update_available_command,
            download_and_install_update_command,
            get_modrinth_categories_command,
            get_modrinth_loaders_command,
            get_modrinth_game_versions_command,
            get_modrinth_versions_by_hashes,
            switch_modpack_version_command,
            uninstall_content_from_profile,
            toggle_content_from_profile,
            toggle_mod_updates,
            bulk_toggle_mod_updates,
            install_content_to_profile,
            commands::minecraft_command::get_profile_by_name_or_uuid,
            commands::minecraft_command::add_skin_locally,
            commands::minecraft_command::get_base64_from_skin_source_command,
            commands::file_command::get_image_preview,
            get_all_profiles_and_last_played,
            get_local_content,
            install_local_content_to_profile,
            switch_content_version,
            commands::minecraft_command::get_starlight_skin_render,
            get_skin_preview_path,
            save_skin_preview,
            commands::minecraft_command::get_crafatar_avatar,
            commands::minecraft_command::reorder_skins,
            commands::nrc_commands::log_message_command,
            equip_vanilla_cape,
            get_vanilla_cape_info,
            refresh_vanilla_cape_data,
            sync_cape_texture_cache,
            get_cached_cape_texture_path,
            get_cape_preview_path,
            save_cape_preview,
            commands::profile_command::add_profile_symlink,
            commands::profile_command::remove_profile_symlink,
            commands::profile_command::get_profile_symlinks,
            commands::profile_command::get_profile_instance_path,
            commands::profile_command::get_default_profile_path,
            set_discord_state_browsing_library,
            set_discord_state_browsing_vxl_studios,
            set_discord_state_browsing_modded_content,
            set_discord_state_playing,
            set_discord_state_idle,
            set_discord_state_getting_ready_to_play,
            set_discord_state_browsing_outfits,
            set_discord_state_browsing_capes,
            set_discord_state_tinkering,
            get_file_sync_configs,
            create_file_sync_config,
            update_file_sync_config,
            delete_file_sync_config,
            get_file_sync_status,
            check_profile_file_exists,
            pull_from_hub,
            push_to_hub,
            get_shared_files,
            commands::changelog_command::fetch_changelog_command,
            commands::changelog_command::fetch_version_changelog_command,
            commands::vxl_api_command::fetch_vxl_api_json,
            commands::mojang_status_command::fetch_mojang_service_status_command
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(
            #[allow(unused_variables)]
            |app_handle, event| {
                // Removed macOS/iOS specific Opened event handling as single-instance handles args now
                // Keep other run event handling if needed, e.g., for window events, exit requested, etc.
                if let tauri::RunEvent::ExitRequested { api, .. } = event {
                    info!("Exit requested, preventing default to allow async tasks to finish if any.");
                    // api.prevent_exit(); // Example: if you need to do cleanup before exit
                }
            },
        );
}
