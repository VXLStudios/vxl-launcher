use crate::config::{ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::error::{AppError, Result};
use crate::minecraft::api::mc_api::MinecraftApiService;
use crate::minecraft::downloads::java_download::JavaDownloadService;
use crate::minecraft::downloads::mc_assets_download::MinecraftAssetsDownloadService;
use crate::minecraft::downloads::mc_client_download::MinecraftClientDownloadService;
use crate::minecraft::downloads::mc_libraries_download::MinecraftLibrariesDownloadService;
use crate::minecraft::downloads::mc_natives_download::{
    MinecraftNativesDownloadService, uses_subdirectory_native_layout_from_jvm_args,
};
use crate::minecraft::downloads::ModDownloadService;
use crate::minecraft::dto::JavaDistribution;
use crate::minecraft::{MinecraftLaunchParameters, MinecraftLauncher};
use crate::state::event_state::{EventPayload, EventType};
use crate::state::profile_state::{ModLoader, Profile};
use crate::state::state_manager::State;
use log::{error, info, warn};
use rand::Rng;
use uuid::Uuid;

use super::minecraft_auth::Credentials;
use super::modloader::ModloaderFactory;
use crate::minecraft::downloads::MinecraftLoggingDownloadService;
use crate::utils::mc_utils;
use tokio::fs as async_fs;

async fn emit_progress_event(
    state: &State,
    event_type: EventType,
    profile_id: Uuid,
    message: &str,
    progress: f64,
    error: Option<String>,
) -> Result<Uuid> {
    let event_id = Uuid::new_v4();
    state
        .emit_event(EventPayload {
            event_id,
            event_type,
            target_id: Some(profile_id),
            message: message.to_string(),
            progress: Some(progress),
            error,
        })
        .await?;
    Ok(event_id)
}

pub async fn install_minecraft_version(
    version_id: &str,
    modloader_str: &str,
    profile: &Profile,
    credentials: Option<Credentials>,
    quick_play_singleplayer: Option<String>,
    quick_play_multiplayer: Option<String>,
    migration_info: Option<crate::utils::profile_utils::MigrationInfo>,
) -> Result<()> {
    // Convert string modloader to ModLoader enum
    let modloader_enum = match modloader_str {
        "vanilla" => ModLoader::Vanilla,
        "fabric" => ModLoader::Fabric,
        "forge" => ModLoader::Forge,
        "neoforge" => ModLoader::NeoForge,
        "quilt" => ModLoader::Quilt,
        _ => {
            return Err(AppError::Unknown(format!(
                "Unbekannter Modloader: {}",
                modloader_str
            )))
        }
    };

    // Get version manifest and find the specific version
    info!(
        "Installing Minecraft version: {} with modloader: {:?}",
        version_id, modloader_enum
    );

    // Get experimental mode from global config
    let state = State::get().await?;
    let is_experimental_mode = state.config_manager.is_experimental_mode().await;
    let launcher_config = state.config_manager.get_config().await;

    info!(
        "[Launch] Setting experimental mode: {}",
        is_experimental_mode
    );
    info!(
        "[Launch] Using concurrent downloads: {}",
        launcher_config.concurrent_downloads
    );

    // <--- HARDCODED TEST ERROR (50% CHANCE) --- >
    let should_throw_error = {
        let mut rng = rand::thread_rng(); // Create and use RNG in a tight scope
        rng.gen_bool(0.5) // 0.5 means 50% probability
    }; // rng goes out of scope here

    if should_throw_error {
        info!("[InstallTest] Randomly decided to throw test error.");
        //return Err(AppError::Unknown("Testfehler (50% Chance) für das Error-Handling!".to_string()));
    } else {
        info!("[InstallTest] Randomly decided NOT to throw test error. Proceeding normally.");
    }
    // <--- END HARDCODED TEST ERROR --- >

    // Execute migration if provided
    if let Some(migration) = &migration_info {
        info!("[Launch] Executing migration before installation: {:?}", migration);

        // Execute the migration (detailed progress events are sent from within execute_group_migration)
        match crate::utils::profile_utils::execute_group_migration(migration.clone(), Some(profile.id)).await {
            Ok(_) => {
                info!("[Launch] Migration completed successfully");
            }
            Err(e) => {
                error!("[Launch] Migration failed: {:?}", e);

                // Send migration failed event
                let migration_failed_payload = crate::state::event_state::EventPayload {
                    event_id: uuid::Uuid::new_v4(),
                    event_type: crate::state::event_state::EventType::MigrationFailed,
                    target_id: Some(profile.id),
                    message: format!("Migration failed: {:?}", e),
                    progress: Some(0.0),
                    error: Some(format!("{:?}", e)),
                };

                if let Err(e) = state.event_state.emit(migration_failed_payload).await {
                    warn!("[Launch] Failed to emit migration failed event: {}", e);
                }

                // Return the error to stop the launch process
                return Err(e);
            }
        }
    }

    if let Some(world) = &quick_play_singleplayer {
        info!(
            "[Launch] Quick Play: Launching directly into singleplayer world: {}",
            world
        );
    } else if let Some(server) = &quick_play_multiplayer {
        info!(
            "[Launch] Quick Play: Connecting directly to server: {}",
            server
        );
    }

    let api_service = MinecraftApiService::new();
    let manifest = api_service.get_version_manifest().await?;
    let version = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| AppError::VersionNotFound(format!("Version {} not found", version_id)))?;

    // Get version metadata
    let piston_meta = api_service.get_piston_meta(&version.url).await?;
    piston_meta.display_info();

    // Get Java version from Minecraft version manifest
    let java_version = piston_meta.java_version.major_version as u32;
    info!("\nChecking Java {} for Minecraft...", java_version);

    // Emit Java installation event
    let event_id = emit_progress_event(
        &state,
        EventType::InstallingJava,
        profile.id,
        &format!("Installing Java {}...", java_version),
        0.0,
        None,
    )
    .await?;

    // Check if profile uses a custom Java path
    let mut custom_java_valid = false;
    let java_path = if profile.settings.use_custom_java_path && profile.settings.java_path.is_some()
    {
        // Try to use the custom Java path
        let custom_path = profile.settings.java_path.as_ref().unwrap();
        info!("Using custom Java path from profile: {}", custom_path);

        // Verify that the custom Java path exists and is valid
        let path = std::path::PathBuf::from(custom_path);
        if path.exists() {
            // Check if it's a valid Java installation
            use crate::utils::java_detector;
            match java_detector::get_java_info(&path).await {
                Ok(java_info) => {
                    use crate::minecraft::downloads::java_download::JavaDownloadService;
                    use crate::utils::system_info::Architecture;

                    info!(
                        "Verified custom Java: Version {}, Major version {}, 64-bit: {}, architecture: {:?}",
                        java_info.version,
                        java_info.major_version,
                        java_info.is_64bit,
                        java_info.architecture
                    );

                    let wrong_arch = java_info.architecture == Architecture::AARCH64
                        && JavaDownloadService::new()
                            .needs_x86_64_java(Some(&piston_meta.java_version.component));

                    if wrong_arch {
                        info!(
                            "Custom Java at {} is arm64, but this version needs x86_64 Java. Downloading Java...",
                            path.display()
                        );
                        custom_java_valid = false;
                        std::path::PathBuf::new()
                    } else if java_info.major_version >= java_version {
                        info!(
                            "Custom Java version {} meets the required version {}",
                            java_info.major_version, java_version
                        );
                        custom_java_valid = true;
                        path
                    } else {
                        info!(
                            "Custom Java version {} is lower than required version {}. Downloading Java...",
                            java_info.major_version, java_version
                        );
                        // The custom Java is too old, we need to download a newer version
                        custom_java_valid = false;
                        // Will be set by the download code below
                        std::path::PathBuf::new()
                    }
                }
                Err(e) => {
                    info!(
                        "Custom Java path exists but is not valid: {}. Downloading Java...",
                        e
                    );
                    // Will be set by the download code below
                    std::path::PathBuf::new()
                }
            }
        } else {
            info!(
                "Custom Java path does not exist: {}. Downloading Java...",
                custom_path
            );
            // Will be set by the download code below
            std::path::PathBuf::new()
        }
    } else {
        // No custom path or not enabled, initialize with empty path
        std::path::PathBuf::new()
    };

    // Download and setup Java if necessary
    let java_path = if custom_java_valid {
        info!("Using verified custom Java path: {:?}", java_path);

        // Update progress to 100% since we're using a custom path
        emit_progress_event(
            &state,
            EventType::InstallingJava,
            profile.id,
            "Using custom Java installation!",
            1.0,
            None,
        )
        .await?;

        java_path
    } else {
        // Download Java since custom path is not valid or not set
        info!("Downloading Java {}...", java_version);
        let java_service = JavaDownloadService::new();
        let downloaded_path = java_service
            .get_or_download_java(
                java_version,
                &JavaDistribution::Zulu,
                Some(&piston_meta.java_version.component),
            )
            .await?;

        info!("Java installation path: {:?}", downloaded_path);

        // Update progress to 100%
        emit_progress_event(
            &state,
            EventType::InstallingJava,
            profile.id,
            &format!("Java {} installation completed!", java_version),
            1.0,
            None,
        )
        .await?;

        downloaded_path
    };

    // Create game directory
    let game_directory = state
        .profile_manager
        .calculate_instance_path_for_profile(profile)?;
    std::fs::create_dir_all(&game_directory)?;

    // --- Copy StartUpHelper data FIRST ---
    info!("\nChecking for StartUpHelper data to import...");

    // Removed: No pre-installed modpacks - norisk pack support removed
    if let Err(e) = mc_utils::copy_startup_helper_data(profile, &game_directory).await {
        // We will only log a warning because this is not a critical step for launching the game.
        // The installation can proceed even if this fails.
        warn!("Failed to import StartUpHelper data (non-critical error): {}", e);
    }
    info!("StartUpHelper data import check complete.");

    // --- Copy initial data from default Minecraft installation ---
    info!("\nChecking for user data to import...");
    if let Err(e) =
        mc_utils::copy_initial_data_from_default_minecraft(profile, &game_directory).await
    {
        // We will only log a warning because this is not a critical step for launching the game.
        // The installation can proceed even if this fails.
        warn!("Failed to import user data (non-critical error): {}", e);
    }
    info!("User data import check complete.");

    // Emit libraries download event
    let libraries_event_id = emit_progress_event(
        &state,
        EventType::DownloadingLibraries,
        profile.id,
        "Downloading libraries...",
        0.0,
        None,
    )
    .await?;

    // Download all required files
    info!("\nDownloading libraries...");
    let libraries_service = MinecraftLibrariesDownloadService::new()
        .with_concurrent_downloads(launcher_config.concurrent_downloads);
    libraries_service
        .download_libraries(&piston_meta.libraries)
        .await?;
    info!("Library download completed!");

    emit_progress_event(
        &state,
        EventType::DownloadingLibraries,
        profile.id,
        "Libraries download completed!",
        1.0,
        None,
    )
    .await?;

    // Emit natives extraction event
    let natives_event_id = emit_progress_event(
        &state,
        EventType::ExtractingNatives,
        profile.id,
        "Extracting natives...",
        0.0,
        None,
    )
    .await?;

    info!("\nExtracting natives...");
    let use_subdirectory_layout = piston_meta
        .arguments
        .as_ref()
        .map(|args| uses_subdirectory_native_layout_from_jvm_args(&args.jvm))
        .unwrap_or(false);
    let natives_service = MinecraftNativesDownloadService::new();
    natives_service
        .extract_natives(
            &piston_meta.libraries,
            version_id,
            use_subdirectory_layout,
        )
        .await?;
    info!("Native extraction completed!");

    emit_progress_event(
        &state,
        EventType::ExtractingNatives,
        profile.id,
        "Natives extraction completed!",
        1.0,
        None,
    )
    .await?;

    info!("\nDownloading assets...");
    let assets_service = MinecraftAssetsDownloadService::new()
        .with_concurrent_downloads(launcher_config.concurrent_downloads);
    assets_service
        .download_assets_with_progress(&piston_meta.asset_index, profile.id)
        .await?;
    info!("Asset download completed!");

    // Emit client download event
    let client_event_id = emit_progress_event(
        &state,
        EventType::DownloadingClient,
        profile.id,
        "Downloading Minecraft client...",
        0.0,
        None,
    )
    .await?;

    info!("\nDownloading Minecraft client...");
    let client_service = MinecraftClientDownloadService::new();
    client_service
        .download_client(&piston_meta.downloads.client, &piston_meta.id)
        .await?;
    info!("Client download completed!");

    emit_progress_event(
        &state,
        EventType::DownloadingClient,
        profile.id,
        "Minecraft client download completed!",
        1.0,
        None,
    )
    .await?;

    // Create and use Minecraft launcher
    let launcher = MinecraftLauncher::new(
        java_path.clone(),
        game_directory.clone(),
        credentials.clone(),
    );

    info!("\nPreparing launch parameters...");

    // Get memory settings (global for standard profiles, profile-specific for custom)
    let memory_max = if profile.is_standard_version {
        let state = State::get().await?;
        let config = state.config_manager.get_config().await;
        config.global_memory_settings.max
    } else {
        profile.settings.memory.max
    };

    let mut launch_params = MinecraftLaunchParameters::new(profile.id, memory_max)
        .with_old_minecraft_arguments(piston_meta.minecraft_arguments.clone())
        .with_resolution(profile.settings.resolution.clone())
        .with_experimental_mode(is_experimental_mode);

    // Add Quick Play parameters if provided
    if let Some(world_name) = quick_play_singleplayer {
        launch_params = launch_params.with_quick_play_singleplayer(world_name);
    } else if let Some(server_address) = quick_play_multiplayer {
        launch_params = launch_params.with_quick_play_multiplayer(server_address);
    }

    // Install modloader using the factory
    if modloader_enum != ModLoader::Vanilla {
        // Resolve loader version using the new modloader factory method
        let resolved_loader = crate::minecraft::modloader::ModloaderFactory::resolve_loader_version(
            profile,
            version_id,
        ).await;

        if let Some(version) = resolved_loader.version {
            let reason_str = match resolved_loader.reason {
                crate::minecraft::modloader::LoaderVersionReason::UserOverwrite => "user overwrite",
                crate::minecraft::modloader::LoaderVersionReason::ProfileDefault => "profile default",
                crate::minecraft::modloader::LoaderVersionReason::NotResolved => "not resolved",
            };
            
            info!(
                "Applying loader version '{}' from {} for MC {} ({:?})",
                version,
                reason_str,
                version_id,
                modloader_enum
            );
        }

        let modloader_installer = ModloaderFactory::create_installer_with_config(
            &modloader_enum,
            java_path.clone(),
            launcher_config.concurrent_downloads,
        );
        let modloader_result = modloader_installer.install(version_id, profile).await?;

        // Apply modloader specific parameters to launch parameters
        if let Some(main_class) = modloader_result.main_class {
            launch_params = launch_params.with_main_class(&main_class);
        } else {
            launch_params = launch_params.with_main_class(&piston_meta.main_class);
        }

        if !modloader_result.libraries.is_empty() {
            launch_params = launch_params.with_additional_libraries(modloader_result.libraries);
        }

        if let Some(jvm_args) = modloader_result.jvm_args {
            launch_params = launch_params.with_additional_jvm_args(jvm_args);
        }

        if let Some(game_args) = modloader_result.game_args {
            launch_params = launch_params.with_additional_game_args(game_args);
        }

        if let Some(minecraft_arguments) = modloader_result.minecraft_arguments {
            launch_params = launch_params.with_old_minecraft_arguments(Some(minecraft_arguments));
        }

        if let Some(custom_client_path) = modloader_result.custom_client_path {
            launch_params = launch_params.with_custom_client_jar(custom_client_path);
        }

        if modloader_result.force_include_minecraft_jar {
            launch_params = launch_params.with_force_include_minecraft_jar(true);
        }
    } else {
        // Vanilla main class
        launch_params = launch_params.with_main_class(&piston_meta.main_class);
    }

    // Add custom JVM arguments from profile settings string
    if let Some(jvm_args_str) = &profile.settings.custom_jvm_args {
        if !jvm_args_str.trim().is_empty() {
            let mut current_jvm_args = launch_params.additional_jvm_args.clone();
            let mut custom_args: Vec<String> =
                jvm_args_str.split_whitespace().map(String::from).collect();
            
            // Check if user has specified a conflicting GC
            let conflicting_gc_patterns = [
                "-XX:+UseZGC",
                "-XX:+UseShenandoahGC",
                "-XX:+UseParallelGC",
                "-XX:+UseSerialGC",
            ];
            
            let has_conflicting_gc = custom_args.iter().any(|arg| {
                conflicting_gc_patterns.iter().any(|pattern| arg.contains(pattern))
            });
            
            // If conflicting GC detected, filter out G1GC-related args
            if has_conflicting_gc {
                let g1gc_patterns = [
                    "-XX:+UseG1GC",
                    "-XX:G1NewSizePercent",
                    "-XX:G1ReservePercent",
                    "-XX:MaxGCPauseMillis",
                    "-XX:G1HeapRegionSize",
                    "-XX:+UnlockExperimentalVMOptions",
                ];
                
                custom_args = custom_args.into_iter()
                    .filter(|arg| !g1gc_patterns.iter().any(|pattern| arg.contains(pattern)))
                    .collect();
                
                info!("Conflicting GC detected in custom JVM args, removed G1GC-related arguments");
            }
            
            info!(
                "Adding custom JVM arguments from profile: {:?}",
                custom_args
            );
            current_jvm_args.extend(custom_args);
            launch_params = launch_params.with_additional_jvm_args(current_jvm_args);
        }
    }

    // Combine Game arguments from modloader (if any) and profile settings (extra_game_args)
    let mut final_game_args = launch_params.additional_game_args.clone();
    final_game_args.extend(profile.settings.extra_game_args.clone());
    launch_params = launch_params.with_additional_game_args(final_game_args);

    // --- Step: Ensure profile-defined mods are downloaded/verified in cache ---
    let mods_event_id = emit_progress_event(
        &state,
        EventType::DownloadingMods,
        profile.id,
        "Downloading/Checking Profile Mods... (Phase 1)",
        0.0,
        None,
    )
    .await?;

    info!(
        "Ensuring profile-defined mods for profile '{}' are downloaded to cache...",
        profile.name
    );
    let mod_downloader_service =
        ModDownloadService::with_concurrency(launcher_config.concurrent_downloads);
    mod_downloader_service
        .download_mods_to_cache(&profile)
        .await?;
    info!(
        "Profile mod cache check/download completed successfully for profile '{}'",
        profile.name
    );

    emit_progress_event(
        &state,
        EventType::DownloadingMods,
        profile.id,
        "Profile Mods downloaded successfully! (Phase 1)",
        1.0,
        None,
    )
    .await?;

    // --- Step: Resolve final mod list for syncing ---
    let resolve_event_id = emit_progress_event(
        &state,
        EventType::SyncingMods,
        profile.id,
        "Resolving final mod list...",
        0.0,
        None,
    )
    .await?;

    let mod_cache_dir = LAUNCHER_DIRECTORY.meta_dir().join("mod_cache");

    // ---> NEW: Get custom mods for this profile <---
    info!("Listing custom mods for profile '{}'...", profile.name);
    let custom_mod_infos = state.profile_manager.list_custom_mods(&profile).await?;
    info!(
        "Found {} custom mods for profile '{}'",
        custom_mod_infos.len(),
        profile.name
    );
    // ---> END NEW <---

    // Call the resolver function
    let target_mods = crate::minecraft::downloads::mod_resolver::resolve_target_mods(
        profile,
        Some(&custom_mod_infos),
        version_id,
        modloader_enum.as_str(),
        &mod_cache_dir,
    )
    .await?;

    emit_progress_event(
        &state,
        EventType::SyncingMods,
        profile.id,
        &format!("Resolved {} mods for sync.", target_mods.len()),
        1.0,
        None,
    )
    .await?;

    // --- Step: Sync mods from cache to profile directory ---
    let sync_event_id = emit_progress_event(
        &state,
        EventType::SyncingMods,
        profile.id,
        "Syncing mods to profile directory... (Phase 3)",
        0.0,
        None,
    )
    .await?;

    info!(
        "Syncing mods from cache to profile directory for '{}'...",
        profile.name
    );

    // Get the correct mods directory path for the profile
    let profile_mods_path = state.profile_manager.get_profile_mods_path(profile)?;

    // Ensure mods folder exists for all loaders before launch/sync
    async_fs::create_dir_all(&profile_mods_path).await?;

    // Pass the resolved target_mods list and the specific mods path to the sync function
    mod_downloader_service
        .sync_mods_to_profile(&target_mods, &profile_mods_path)
        .await?;

    info!("Mod sync completed for profile '{}'", profile.name);
    emit_progress_event(
        &state,
        EventType::SyncingMods,
        profile.id,
        "Mod sync complete! (Phase 3)",
        1.0,
        None,
    )
    .await?;

    // Download log4j configuration if available
    let mut log4j_arg = None;
    if let Some(logging) = &piston_meta.logging {
        info!("\nDownloading log4j configuration...");
        let logging_service = MinecraftLoggingDownloadService::new();
        let config_path = logging_service
            .download_logging_config(&logging.client)
            .await?;
        log4j_arg = Some(logging_service.get_jvm_argument(&config_path));
        info!("Log4j configuration download completed!");
    }

    // Add log4j configuration to JVM arguments if available
    if let Some(log4j_argument) = log4j_arg {
        info!("Adding log4j configuration: {}", log4j_argument);
        let mut jvm_args = launch_params.additional_jvm_args.clone();
        jvm_args.push(log4j_argument);
        launch_params = launch_params.with_additional_jvm_args(jvm_args);
    }

    // --- Execute pre-launch hooks ---
    let launcher_config = state.config_manager.get_config().await;
    if let Some(hook) = &launcher_config.hooks.pre_launch {
        info!("Executing pre-launch hook: {}", hook);
        let hook_event_id = emit_progress_event(
            &state,
            EventType::LaunchingMinecraft,
            profile.id,
            "Executing pre-launch hook...",
            0.0,
            None,
        )
        .await?;

        let mut child = crate::utils::hook_utils::spawn_hook(hook, &game_directory)?;
        let result = child.wait().map_err(AppError::Io)?;

        if !result.success() {
            let error_msg = format!(
                "Pre-launch hook failed with exit code: {}",
                result.code().unwrap_or(-1)
            );
            error!("{}", error_msg);
            return Err(AppError::Other(error_msg));
        }
        info!("Pre-launch hook executed successfully");
    }

    // --- Launch Minecraft ---
    // Emit launch event
    let launch_event_id = emit_progress_event(
        &state,
        EventType::LaunchingMinecraft,
        profile.id,
        "Starting Minecraft...",
        0.0,
        None,
    )
    .await?;

    launcher
        .launch(&piston_meta, launch_params, Some(profile.clone()))
        .await?;

    emit_progress_event(
        &state,
        EventType::LaunchingMinecraft,
        profile.id,
        "Minecraft launched successfully!",
        1.0,
        None,
    )
    .await?;

    Ok(())
}
