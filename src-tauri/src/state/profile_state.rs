use crate::config::LAUNCHER_DIRECTORY;
use crate::error::AppError;
use crate::error::Result;
use crate::integrations::modrinth::{self, ModrinthDependencyType, ModrinthVersion};
use crate::state::post_init::PostInitializationHandler;
use crate::utils::backup_utils::{self, BackupConfig, safe_write_with_backup};
use crate::utils::hash_utils;
use crate::utils::mc_utils;
use crate::utils::path_utils;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tauri_plugin_dialog::FilePath;
use tokio::fs;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ModSource {
    Local {
        file_name: String,
    },
    Url {
        url: String,
        file_name: Option<String>,
    },
    Maven {
        coordinates: String,
        repository_url: Option<String>,
    }, // e.g., "net.fabricmc:fabric-api:0.91.0+1.20.1"
    Embedded {
        name: String,
    }, // e.g., "Fabric API" provided by the loader itself
    Modrinth {
        project_id: String,             // Modrinth Project ID (e.g., "AANobbMI")
        version_id: String,             // Modrinth Version ID (e.g., "tFw0iWAk")
        file_name: String, // The actual filename (e.g., "sodium-fabric-mc1.20.1-0.5.3.jar")
        download_url: String, // The direct download URL used when adding
        file_hash_sha1: Option<String>, // Optional SHA1 hash for verification
    }, // New variant for Modrinth mods
    CurseForge {
        project_id: String,             // CurseForge Project ID (e.g., "238222")
        file_id: String,                // CurseForge File ID (e.g., "6829086")
        file_name: String, // The actual filename (e.g., "jei-1.21.1-neoforge-19.22.1.316.jar")
        download_url: String, // The direct download URL used when adding
        file_hash_sha1: Option<String>, // Optional SHA1 hash for verification
        file_fingerprint: Option<u64>, // CurseForge fingerprint for update checking
    }, // New variant for CurseForge mods
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Mod {
    #[serde(default = "Uuid::new_v4")] // Assign a new UUID if missing
    pub id: Uuid,
    pub source: ModSource,
    pub enabled: bool,
    pub display_name: Option<String>, // Optional: Could be inferred from mod metadata later
    pub version: Option<String>,      // Optional: Could be inferred
    pub game_versions: Option<Vec<String>>, // Changed: List of supported Minecraft versions
    pub file_name_override: Option<String>, // Optional: To store the actual filename on disk if needed
    pub associated_loader: Option<ModLoader>, // Optional: Tracks the loader this mod was originally intended for
    /// Origin modpack identifier in format: "platform:project_id[:version_id]"
    /// Example: "modrinth:AANobbMI:tFw0iWAk" or "curseforge:12345:67890"
    /// None for manually added mods
    pub modpack_origin: Option<String>,
    /// True if automatic updates are enabled for this mod (default: true)
    #[serde(default = "default_true")]
    pub updates_enabled: bool,
}

// New struct to uniquely identify a Norisk Pack mod within a specific context
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NoriskModIdentifier {
    pub pack_id: String,
    pub mod_id: String,
    pub game_version: String,
    pub loader: ModLoader,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ImageSource {
    Url {
        url: String,
    },
    RelativePath {
        path: String,
    }, // Relative to launcher_directory
    RelativeProfile {
        path: String,
    }, // Relative to profile directory
    AbsolutePath {
        path: String,
    },
    Base64 {
        data: String,
        mime_type: Option<String>, // Optional MIME type, e.g., "image/png"
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProfileBanner {
    pub source: ImageSource,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    #[serde(default = "Uuid::new_v4")] // Use new_v4 for a default ID
    pub id: Uuid, // Eindeutige ID
    pub name: String,                   // Anzeigename
    pub path: String,                   // Dateipfad
    pub game_version: String,           // Minecraft Version
    pub loader: ModLoader,              // Modloader Typ
    pub loader_version: Option<String>, // Modloader Version
    #[serde(default)]
    pub created: DateTime<Utc>, // Erstellungsdatum
    pub last_played: Option<DateTime<Utc>>, // Letzter Start
    #[serde(default)]
    pub settings: ProfileSettings, // Profil Einstellungen
    #[serde(default)]
    pub state: ProfileState, // Aktueller Status
    #[serde(default)] // Add default for backward compatibility when loading old profiles
    pub mods: Vec<Mod>, // List of mods for this profile
    /// Optional: If this profile was created from a standard profile, store its original ID
    #[serde(default)]
    pub source_standard_profile_id: Option<Uuid>,
    /// Optional group name for UI organization and filtering
    #[serde(default)]
    pub group: Option<String>,
    /// Whether this profile should use a shared Minecraft folder
    #[serde(default)]
    pub use_shared_minecraft_folder: bool,
    /// True if this is a standard profile template, false if it's a user profile.
    #[serde(default)] // Defaults to false for existing user profiles
    pub is_standard_version: bool,
    pub description: Option<String>,
    #[serde(default)]
    pub banner: Option<ProfileBanner>, // Banner/background image for the profile
    #[serde(default)]
    pub background: Option<ProfileBanner>,
    /// Information about this profile's modpack origin (if it was created from a modpack)
    #[serde(default)]
    pub modpack_info: Option<ModPackInfo>,
    /// Optional preferred account UUID for launching this profile
    /// If set, this account will be used instead of the global active account
    #[serde(default)]
    pub preferred_account_id: Option<Uuid>,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NoriskInformation {
    #[serde(default)]
    pub keep_local_assets: bool,
    #[serde(default)]
    pub is_experimental: bool,
    #[serde(default = "default_true")]
    pub copy_initial_mc_data: bool,
    #[serde(default)]
    pub is_main_version: bool,
}

/// Information about a modpack source (Modrinth or CurseForge)
/// This allows tracking the origin and versions of modpacks for updates
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ModPackSource {
    /// Modrinth modpack source
    Modrinth {
        /// Modrinth Project ID (e.g., "AANobbMI")
        project_id: String,
        /// Modrinth Version ID (e.g., "tFw0iWAk")
        version_id: String,
    },
    /// CurseForge modpack source
    CurseForge {
        /// CurseForge Project ID
        project_id: u32,
        /// CurseForge File ID
        file_id: u32,
    },
}

/// Information about a modpack installation
/// Stores metadata about installed modpacks for tracking and updates
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPackInfo {
    /// The source platform and identifiers
    pub source: ModPackSource,
    /// File hash for verification (SHA1 for Modrinth, fingerprint for CurseForge)
    pub file_hash: Option<String>,
}



#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Forge,
    Fabric,
    Quilt,
    NeoForge,
}

impl ModLoader {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neoforge",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "vanilla" => Ok(ModLoader::Vanilla),
            "forge" => Ok(ModLoader::Forge),
            "fabric" => Ok(ModLoader::Fabric),
            "quilt" => Ok(ModLoader::Quilt),
            "neoforge" => Ok(ModLoader::NeoForge),
            _ => Err(AppError::Other(format!("Invalid mod loader: {}", s))),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileSettings {
    pub java_path: Option<String>, // Java Pfad
    #[serde(default)]
    pub use_custom_java_path: bool, // Ob der benutzerdefinierte Java-Pfad verwendet werden soll
    #[serde(default)]
    pub use_overwrite_loader_version: bool, // Ob die überschriebene Loader-Version verwendet werden soll
    pub overwrite_loader_version: Option<String>, // Überschriebene Loader-Version
    pub memory: MemorySettings,    // Speicher Einstellungen
    #[serde(default)]
    pub resolution: Option<WindowSize>, // Auflösung
    #[serde(default)]
    pub fullscreen: bool, // Vollbild
    #[serde(default)]
    pub extra_game_args: Vec<String>, // Zusätzliche Argumente für das Spiel
    #[serde(default)] // Für Abwärtskompatibilität
    pub custom_jvm_args: Option<String>, // Zusätzliche JVM-Argumente als String
    #[serde(default)]
    pub quick_play_path: Option<String>, // Quick Play Pfad für direkten Welt-/Server-Start
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemorySettings {
    pub min: u32, // in MB
    pub max: u32, // in MB
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileState {
    NotInstalled, // Profil existiert nur in der DB
    Installing,   // Wird installiert
    Installed,    // Installiert und bereit
    Running,      // Läuft gerade
    Error,        // Fehler aufgetreten
}

impl Default for ProfileState {
    fn default() -> Self {
        ProfileState::NotInstalled
    }
}

// --- Custom Mod Structs & Enums ---

#[derive(Serialize, Clone, Debug)]
pub struct CustomModInfo {
    pub filename: String, // Base filename (e.g., OptiFine.jar)
    pub is_enabled: bool, // True if the file doesn't end with .disabled
    pub path: PathBuf,    // Full path to the file in custom_mods directory
}

impl Profile {
    /// Returns whether this profile should actually use a shared Minecraft folder.
    /// This method takes into account both the profile setting and special group logic.
    pub fn should_use_shared_minecraft_folder(&self) -> bool {
        // For isolated groups (server, modpacks), always return false regardless of the setting
        if let Some(group) = &self.group {
            if ProfileManager::is_isolated_group(group) {
                return false;
            }
        }
        
        // Profile has no group, don't use shared folder (use original path logic)
        self.use_shared_minecraft_folder
    }
}

// Profile Manager
pub struct ProfileManager {
    profiles: Arc<RwLock<HashMap<Uuid, Profile>>>,
    profiles_path: PathBuf,
    save_lock: Mutex<()>,
    backup_config: BackupConfig,
    store: crate::state::profile_store::ProfileStore,
    db: crate::state::db::DbHandle,
    transient: RwLock<HashSet<Uuid>>,
    persisted: Mutex<HashMap<Uuid, u64>>,
    loading: Mutex<()>,
}

const PRE_MIGRATION_SNAPSHOT: &str = "pre-profiles";

impl ProfileManager {
    pub fn new(profiles_path: PathBuf, db: crate::state::db::DbHandle) -> Result<Self> {
        info!(
            "ProfileManager: Initializing with path: {:?} (profiles loading deferred)",
            profiles_path
        );

        // Configure backup settings - more aggressive for profiles due to critical nature
        let backup_config = BackupConfig {
            max_backups_per_file: 10, // Keep more backups for profiles
            max_backup_age_seconds: 90 * 24 * 60 * 60, // 90 days for profiles
            min_backup_interval_seconds: 60, // TEMP: Increased to 5 minutes to prevent spam during testing
        };

        Ok(Self {
            profiles: Arc::new(RwLock::new(HashMap::new())), // Start with empty profiles
            profiles_path,
            save_lock: Mutex::new(()),
            backup_config,
            store: crate::state::profile_store::ProfileStore::new(db.clone()),
            db,
            transient: RwLock::new(HashSet::new()),
            persisted: Mutex::new(HashMap::new()),
            loading: Mutex::new(()),
        })
    }

    fn migrated_json_path(&self) -> PathBuf {
        self.profiles_path.with_extension("json.migrated")
    }

    async fn read_migration_source(&self, source: &std::path::Path) -> Result<String> {
        let mut current = source.to_path_buf();

        for attempt in 0..2 {
            let raw = fs::read_to_string(&current).await?;
            if serde_json::from_str::<Vec<serde_json::Value>>(&raw).is_ok() {
                return Ok(raw);
            }

            error!(
                "ProfileManager: {:?} is not a readable profile list, quarantining it",
                current
            );
            let quarantine = current.with_extension(format!(
                "corrupted.{}",
                Utc::now().format("%Y%m%d_%H%M%S")
            ));
            if fs::copy(&current, &quarantine).await.is_ok() {
                info!("ProfileManager: corrupted file saved as {:?}", quarantine);
            }

            if attempt == 1 {
                break;
            }

            match backup_utils::restore_from_backup(&self.profiles_path, Some("profiles")).await {
                Ok(restored) => {
                    info!("ProfileManager: restored profiles from backup {:?}", restored);
                    current = self.profiles_path.clone();
                }
                Err(e) => {
                    error!("ProfileManager: no usable backup to restore: {}", e);
                    break;
                }
            }
        }

        warn!("ProfileManager: importing an empty profile list");
        Ok("[]".to_string())
    }

    async fn migrate_if_needed(&self) -> Result<()> {
        if self.store.is_migrated().await? {
            return Ok(());
        }

        if self.store.counts().await?.0 > 0 {
            warn!(
                "The database already holds profiles but was never marked as migrated; adopting them instead of importing over them"
            );
            self.store.mark_migrated().await?;
            return Ok(());
        }

        let source = if self.profiles_path.exists() {
            self.profiles_path.clone()
        } else if self.migrated_json_path().exists() {
            info!("The database is empty; re-importing from profiles.json.migrated");
            self.migrated_json_path()
        } else {
            info!("No profiles.json to import; starting the database empty");
            self.store.import_from_json("[]").await?;
            return Ok(());
        };

        if source == self.profiles_path {
            if let Err(e) =
                backup_utils::create_backup(&source, Some("profiles"), &self.backup_config).await
            {
                warn!("Could not back up profiles.json before importing: {}", e);
            }
        }

        let raw = self.read_migration_source(&source).await?;
        let outcome = self.store.import_from_json(&raw).await?;

        if source == self.profiles_path {
            if let Err(e) = fs::rename(&self.profiles_path, self.migrated_json_path()).await {
                warn!("Could not rename profiles.json after importing: {}", e);
            }
        }

        info!(
            "Imported {} profiles and {} mods into the database ({} quarantined)",
            outcome.imported, outcome.mods, outcome.unparsed
        );
        Ok(())
    }

    async fn save_profile(&self, id: Uuid) -> Result<()> {
        let _guard = self.save_lock.lock().await;

        let transient = self.transient.read().await.clone();
        let profile = {
            let profiles = self.profiles.read().await;
            match profiles.get(&id) {
                Some(profile) if should_persist(profile, &transient) => profile.clone(),
                _ => return Ok(()),
            }
        };

        let hash = content_hash(&profile)?;
        if self.persisted.lock().await.get(&id) == Some(&hash) {
            return Ok(());
        }

        self.store.upsert_many(std::slice::from_ref(&profile)).await?;
        self.persisted.lock().await.insert(id, hash);
        Ok(())
    }

    // Renamed from load_profiles to avoid conflict, made internal
    async fn load_profiles_internal(&self, path: &PathBuf) -> Result<HashMap<Uuid, Profile>> {
        let mut attempt_count = 0;
        let max_attempts = 2; // Allow one retry after restoration

        loop {
            attempt_count += 1;

            if !path.exists() {
                if attempt_count == 1 {
                    info!("ProfileManager: Profiles file doesn't exist, checking for backups to restore");
                    // Try to restore from backup if file doesn't exist
                    match backup_utils::restore_from_backup(path, Some("profiles")).await {
                        Ok(restored_path) => {
                            info!("ProfileManager: Successfully restored profiles from backup: {:?}", restored_path);
                            continue; // Try loading again
                        }
                        Err(e) => {
                            warn!("ProfileManager: No backup available to restore: {}", e);
                            return Ok(HashMap::new());
                        }
                    }
                } else {
                    return Ok(HashMap::new());
                }
            }

            match fs::read_to_string(path).await {
                Ok(data) => {
                    match serde_json::from_str::<Vec<Profile>>(&data) {
                        Ok(profiles) => {
                            info!("ProfileManager: Successfully loaded {} profiles from file", profiles.len());
                            return Ok(profiles.into_iter().map(|p| (p.id, p)).collect());
                        }
                        Err(e) => {
                            if attempt_count < max_attempts {
                                error!("ProfileManager: Failed to parse profiles JSON: {}. Attempting recovery from backup.", e);

                                // Backup the corrupted file
                                let corrupted_path = path.with_extension(format!(
                                    "corrupted.{}",
                                    Utc::now().format("%Y%m%d_%H%M%S")
                                ));
                                if let Err(backup_err) = fs::copy(path, &corrupted_path).await {
                                    error!("ProfileManager: Failed to backup corrupted file: {}", backup_err);
                                } else {
                                    info!("ProfileManager: Corrupted profiles file saved as: {:?}", corrupted_path);
                                }

                                // Try to restore from backup
                                match backup_utils::restore_from_backup(path, Some("profiles")).await {
                                    Ok(restored_path) => {
                                        info!("ProfileManager: Successfully restored profiles from backup: {:?}", restored_path);
                                        continue; // Try loading again
                                    }
                                    Err(restore_err) => {
                                        error!("ProfileManager: Failed to restore from backup: {}. Starting with empty profiles.", restore_err);
                                        return Ok(HashMap::new());
                                    }
                                }
                            } else {
                                error!("ProfileManager: Failed to parse profiles JSON after {} attempts: {}. Starting with empty profiles.", max_attempts, e);
                                return Ok(HashMap::new());
                            }
                        }
                    }
                }
                Err(e) => {
                    if attempt_count < max_attempts {
                        error!("ProfileManager: Failed to read profiles file: {}. Attempting recovery from backup.", e);

                        // Try to restore from backup
                        match backup_utils::restore_from_backup(path, Some("profiles")).await {
                            Ok(restored_path) => {
                                info!("ProfileManager: Successfully restored profiles from backup: {:?}", restored_path);
                                continue; // Try loading again
                            }
                            Err(restore_err) => {
                                error!("ProfileManager: Failed to restore from backup: {}. Starting with empty profiles.", restore_err);
                                return Ok(HashMap::new());
                            }
                        }
                    } else {
                        error!("ProfileManager: Failed to read profiles file after {} attempts: {}. Starting with empty profiles.", max_attempts, e);
                        return Ok(HashMap::new());
                    }
                }
            }
        }
    }

    async fn save_profiles(&self) -> Result<()> {
        let ids: Vec<Uuid> = self.profiles.read().await.keys().copied().collect();
        for id in ids {
            self.save_profile(id).await?;
        }
        Ok(())
    }

    // CRUD Operationen
    pub async fn create_profile(&self, profile: Profile) -> Result<Uuid> {
        // The 'profile.path' field is expected to be a relative path/name for the profile directory
        // e.g., "My Profile Name" or "some_group/My Profile Name"
        info!(
            "Attempting to create profile named '{}' with relative path identifier: {:?}",
            profile.name, profile.path
        );

        // Calculate the absolute path for the new profile's instance directory
        let new_profile_instance_path = self.calculate_instance_path_for_profile(&profile)?;

        info!(
            "Calculated absolute profile instance directory: {:?}",
            new_profile_instance_path
        );

        // Create the specific instance directory for this new profile.
        // This will also create any necessary parent directories, including the one
        // where profiles.json (self.profiles_path) will be stored, due to the nature of create_dir_all.
        info!(
            "Creating profile instance directory at: {:?}",
            new_profile_instance_path
        );
        fs::create_dir_all(&new_profile_instance_path).await?; // Use the calculated full path

        let id = profile.id;
        {
            let mut profiles = self.profiles.write().await;
            // The 'profile' object with its relative 'path' is stored.
            // Other functions will use calculate_instance_path_for_profile to resolve it.
            profiles.insert(id, profile);
        }
        info!("Saving profiles metadata to: {:?}", self.profiles_path);
        self.save_profiles().await?;
        Ok(id)
    }

    pub async fn get_profile(&self, id: Uuid) -> Result<Profile> {
        let profiles = self.profiles.read().await;
        if let Some(profile) = profiles.get(&id).cloned() {
            Ok(profile)
        } else {
            Err(AppError::ProfileNotFound(id))
        }
    }

    pub async fn update_profile(&self, id: Uuid, profile: Profile) -> Result<()> {
        {
            let mut profiles = self.profiles.write().await;
            profiles.insert(id, profile);
        }
        self.save_profiles().await?;
        Ok(())
    }

    /// Helper function to check if any other profile uses the same path
    /// This is used before deleting a profile directory to ensure we don't delete
    /// files that are still needed by other profiles
    async fn has_other_profile_with_same_path<F>(&self, exclude_id: Uuid, target_path: &PathBuf, path_calculator: F) -> bool
    where
        F: Fn(&Profile) -> PathBuf,
    {
        let profiles = self.profiles.read().await;

        for (&profile_id, profile) in profiles.iter() {
            // Skip the profile we're about to delete
            if profile_id == exclude_id {
                continue;
            }

            // Calculate the path for this profile and compare
            let other_path = path_calculator(profile);
            if other_path == *target_path {
                info!(
                    "Found another profile '{}' (ID: {}) using the same path: {:?}",
                    profile.name, profile_id, target_path
                );
                return true;
            }
        }

        false
    }



    pub async fn delete_profile(&self, id: Uuid) -> Result<()> {
        let profile_to_delete: Option<Profile>;

        // Scope to release the read lock quickly
        {
            let profiles = self.profiles.read().await;
            profile_to_delete = profiles.get(&id).cloned(); // Clone the profile data if it exists
        }

        // If the profile exists, determine its path using the helper function
        let profile_dir_path = if let Some(profile) = &profile_to_delete {
            match self.calculate_instance_path_for_profile(&profile) {
                Ok(path) => {
                    info!(
                        "Profile '{}' marked for deletion. Directory path: {:?}",
                        profile.name, path
                    );
                    Some(path)
                }
                Err(e) => {
                    // Should not happen if profile object is valid, but handle defensively
                    error!("Failed to calculate instance path for profile '{}': {}. Aborting directory deletion.", profile.name, e);
                    // Return an error, as we can't be sure about the path
                    return Err(AppError::Other(format!(
                        "Could not calculate profile path: {}",
                        e
                    )));
                }
            }
        } else {
            // Profile not found in map, nothing to delete on filesystem
            info!("Profile with ID {} not found for deletion.", id);
            return Err(AppError::ProfileNotFound(id)); // Return error if profile doesn't exist
        };

        // Check if other profiles use the same path before attempting directory deletion
        let should_delete_directory = if let Some(path) = &profile_dir_path {
            if self.has_other_profile_with_same_path(id, path, |profile| {
                self.calculate_instance_path_for_profile(profile).unwrap_or_default()
            }).await {
                info!(
                    "Another profile is using the same directory path {:?}. Skipping directory deletion.",
                    path
                );
                false
            } else {
                info!(
                    "No other profile uses the directory path {:?}. Safe to delete.",
                    path
                );
                true
            }
        } else {
            false
        };

        // Attempt to delete the directory only if no other profile uses it
        if should_delete_directory {
            if let Some(ref path) = profile_dir_path {
                if path.exists() {
                    info!("Moving profile directory to trash: {:?}", path);
                    match crate::utils::trash_utils::move_path_to_trash(path, Some("profiles")).await {
                        Ok(wrapper) => info!("Profile directory moved to trash wrapper: {:?}", wrapper),
                        Err(e) => {
                            error!("Failed to move profile directory {:?} to trash: {}", path, e);
                            return Err(e);
                        }
                    }
                } else {
                    info!(
                        "Profile directory {:?} does not exist. Skipping directory deletion.",
                        path
                    );
                }
            }
        }

        // Additionally, always try to delete the individual profile path (build_path_from_profile_path)
        // This covers cases where the profile might have files in both group and individual directories
        if let Some(profile) = &profile_to_delete {
            let individual_path = Self::build_path_from_profile_path(profile);

            // Only delete if it's different from the main path
            if Some(&individual_path) != profile_dir_path.as_ref() {
                // Check if other profiles use the same individual path before attempting deletion
                let should_delete_individual_directory = if self.has_other_profile_with_same_path(id, &individual_path, |profile| {
                    Self::build_path_from_profile_path(profile)
                }).await {
                    info!(
                        "Another profile is using the same individual directory path {:?}. Skipping individual directory deletion.",
                        individual_path
                    );
                    false
                } else {
                    info!(
                        "No other profile uses the individual directory path {:?}. Safe to delete.",
                        individual_path
                    );
                    true
                };

                if should_delete_individual_directory {
                    if individual_path.exists() {
                        info!("Moving individual profile directory to trash: {:?}", individual_path);
                        match crate::utils::trash_utils::move_path_to_trash(&individual_path, Some("profiles")).await {
                            Ok(wrapper) => info!("Individual profile directory moved to trash wrapper: {:?}", wrapper),
                            Err(e) => {
                                error!("Failed to move individual profile directory {:?} to trash: {}", individual_path, e);
                                // Don't return error here, as the main profile deletion was successful
                                warn!("Continuing despite individual path deletion failure.");
                            }
                        }
                    } else {
                        info!(
                            "Individual profile directory {:?} does not exist. Skipping deletion.",
                            individual_path
                        );
                    }
                }
            } else {
                info!("Individual path is the same as main path, skipping separate deletion.");
            }
        }

        // Remove the profile from the in-memory map
        {
            let mut profiles = self.profiles.write().await;
            if profiles.remove(&id).is_none() {
                // This case should ideally not happen if we found it earlier, but log just in case
                warn!(
                    "Profile {} was not found in the map during final removal step.",
                    id
                );
            }
        }

        self.store.delete_profile(id).await?;
        self.persisted.lock().await.remove(&id);
        info!(
            "Successfully removed profile entry {} from configuration.",
            id
        );

        Ok(())
    }

    // Add a new mod to a specific profile
    pub async fn add_mod(&self, profile_id: Uuid, mod_info: Mod) -> Result<()> {
        info!(
            "Adding mod '{}' (Source: {:?}) to profile {}",
            mod_info
                .display_name
                .as_deref()
                .unwrap_or(&mod_info.id.to_string()),
            mod_info.source,
            profile_id
        );

        let mut profiles = self.profiles.write().await;

        if let Some(profile) = profiles.get_mut(&profile_id) {
            if !profile
                .mods
                .iter()
                .any(|existing_mod| existing_mod.source == mod_info.source)
            {
                profile.mods.push(mod_info);
                drop(profiles);
                self.save_profiles().await?;
                info!("Successfully added mod to profile {}", profile_id);
                Ok(())
            } else {
                info!(
                    "Mod with the same source already exists in profile {}",
                    profile_id
                );
                Err(AppError::Other(format!(
                    "Mod already exists in profile {}",
                    profile_id
                )))
            }
        } else {
            Err(AppError::ProfileNotFound(profile_id))
        }
    }

    // Add a mod specifically sourced from Modrinth - Internal function with dependency logic
    // Use BoxFuture for recursion
    fn add_modrinth_mod_internal<'a>(
        &'a self,
        profile_id: Uuid,
        project_id: String,
        version_id: String,
        file_name: String,
        download_url: String,
        file_hash_sha1: Option<String>,
        // Optional details for better Mod struct population
        mod_name: Option<String>,
        version_number: Option<String>,
        // Loaders and game versions associated with *this specific version* being added
        loaders: Option<Vec<String>>,
        game_versions: Option<Vec<String>>,
        // Flag to control dependency fetching
        add_dependencies: bool,
        // Internal parameter to prevent infinite loops
        visited_mods: HashSet<(String, String)>,
    ) -> BoxFuture<'a, Result<()>> {
        // Return type is BoxFuture
        Box::pin(async move {
            let display_name_log = mod_name.as_deref().unwrap_or(&project_id);
            let version_log = version_number.as_deref().unwrap_or(&version_id);
            info!(
                "Processing Modrinth mod {} (Version {}) for profile {}. Add dependencies: {}",
                display_name_log, version_log, profile_id, add_dependencies
            );

            let mod_key = (project_id.clone(), version_id.clone());
            if visited_mods.contains(&mod_key) {
                info!(
                    "Skipping already processed mod/dependency: {} ({})",
                    display_name_log, version_log
                );
                return Ok(());
            }
            let mut visited_mods_clone = visited_mods.clone();
            visited_mods_clone.insert(mod_key);

            let source = ModSource::Modrinth {
                project_id: project_id.clone(),
                version_id: version_id.clone(),
                file_name: file_name.clone(),
                download_url: download_url.clone(),
                file_hash_sha1: file_hash_sha1.clone(),
            };

            let mut needs_save = false;
            {
                let mut profiles = self.profiles.write().await;
                if let Some(profile) = profiles.get_mut(&profile_id) {
                    if !profile.mods.iter().any(|m| m.source == source) {
                        info!(
                            "Adding mod {} ({}) to profile {}",
                            display_name_log, version_log, profile_id
                        );

                        let new_mod = Mod {
                            id: Uuid::new_v4(),
                            source: source.clone(),
                            enabled: true,
                            display_name: mod_name.clone().or_else(|| Some(file_name.clone())),
                            version: version_number.clone(),
                            game_versions: game_versions.clone(),
                            file_name_override: None,
                            associated_loader: loaders
                                .clone()
                                .and_then(|l| l.first().and_then(|s| ModLoader::from_str(s).ok())),
                            modpack_origin: None, // Manually added mod
                            updates_enabled: true, // Updates enabled by default
                        };
                        profile.mods.push(new_mod);
                        needs_save = true;
                    } else {
                        info!(
                            "Mod {} ({}) already exists in profile {}. Skipping addition.",
                            display_name_log, version_log, profile_id
                        );
                    }
                } else {
                    return Err(AppError::ProfileNotFound(profile_id));
                }
            }

            if needs_save {
                self.save_profiles().await?;
                info!(
                    "Profile saved after adding mod {} ({})",
                    display_name_log, version_log
                );
            }

            if add_dependencies {
                info!(
                    "Fetching dependencies for {} ({})",
                    display_name_log, version_log
                );

                let profile_details = self.get_profile(profile_id).await?;
                let profile_loader_str = profile_details.loader.as_str().to_string();
                let profile_game_version = profile_details.game_version.clone();

                match modrinth::get_mod_versions(project_id.clone(), None, None).await {
                    Ok(versions) => {
                        if let Some(version_info) =
                            versions.into_iter().find(|v| v.id == version_id)
                        {
                            info!(
                                "Found {} dependencies for {} ({})",
                                version_info.dependencies.len(),
                                display_name_log,
                                version_log
                            );

                            for dependency in version_info.dependencies {
                                if dependency.dependency_type == ModrinthDependencyType::Required {
                                    info!("Processing required dependency: Project={:?}, Version={:?}", dependency.project_id, dependency.version_id);

                                    if let Some(dep_project_id) = dependency.project_id {
                                        info!("Attempting to find compatible version for dependency project '{}'", dep_project_id);

                                        let target_version_id = dependency.version_id;

                                        // Fetch dependency versions compatible with the profile's loader, but *without* filtering by game version yet.
                                        // Game version filtering will happen below based on the *parent mod's* requirements.
                                        match modrinth::get_mod_versions(
                                            dep_project_id.clone(), 
                                            Some(vec![profile_loader_str.clone()]), 
                                            None // <-- Removed game_version filter here
                                        ).await {
                                            Ok(dep_versions) => {
                                                let mut best_dep_version: Option<&ModrinthVersion> = None;

                                                // If a specific dependency version was requested, try to find that first.
                                                if let Some(tv_id) = &target_version_id { // Borrow tv_id
                                                    best_dep_version = dep_versions.iter().find(|v| &v.id == tv_id);
                                                    if best_dep_version.is_none() {
                                                        warn!("Requested dependency version '{}' not found or not compatible with profile's loader for project '{}'. Trying to find best alternative.", tv_id, dep_project_id);
                                                    }
                                                }
                                                
                                                // If no specific version requested or found, find the best compatible version.
                                                if best_dep_version.is_none() {
                                                    
                                                    // Determine the target game versions for filtering: use the PARENT mod's versions (from version_info) if available (non-empty), else fallback to profile's.
                                                    let target_game_versions_for_dep: Vec<String> = if !version_info.game_versions.is_empty() {
                                                        // Use the parent mod's game versions if the list is not empty
                                                        version_info.game_versions.clone()
                                                    } else {
                                                         // Otherwise, fallback to the profile's game version
                                                         warn!("Parent mod {} ({}) did not provide specific game versions in its fetched data (version_info) or list was empty. Falling back to profile game version '{}' for dependency '{}' lookup.", display_name_log, version_log, profile_game_version, dep_project_id);
                                                        vec![profile_game_version.clone()]
                                                    };
                                                    
                                                     // Attempt 1: Find the latest version supporting any of the *target* game versions.
                                                     best_dep_version = dep_versions.iter()
                                                         .filter(|dep_v| {
                                                            // Check if the dependency version supports AT LEAST ONE of the target game versions
                                                            target_game_versions_for_dep.iter().any(|target_gv| dep_v.game_versions.contains(target_gv))
                                                         })
                                                         .max_by_key(|v| &v.date_published);

                                                     // Attempt 2: If no match for target game versions, fall back to the overall latest compatible version (loader match only).
                                                     if best_dep_version.is_none() {
                                                         warn!("Could not find dependency version matching target game versions {:?} for project '{}'. Falling back to latest version compatible with loader '{}'.", target_game_versions_for_dep, dep_project_id, profile_loader_str);
                                                         best_dep_version = dep_versions.iter()
                                                             .max_by_key(|v| &v.date_published);
                                                     }
                                                 }

                                                 if let Some(selected_dep_version) = best_dep_version {
                                                     info!("Selected version '{}' ({}) for dependency '{}'", selected_dep_version.name, selected_dep_version.id, dep_project_id);
                                                     
                                                     if let Some(primary_file) = selected_dep_version.files.iter().find(|f| f.primary) {
                                                         match self.add_modrinth_mod_internal(
                                                             profile_id,
                                                             selected_dep_version.project_id.clone(),
                                                             selected_dep_version.id.clone(),
                                                             primary_file.filename.clone(),
                                                             primary_file.url.clone(),
                                                             primary_file.hashes.sha1.clone(),
                                                             Some(selected_dep_version.name.clone()),
                                                             Some(selected_dep_version.version_number.clone()),
                                                             Some(selected_dep_version.loaders.clone()),
                                                             Some(selected_dep_version.game_versions.clone()),
                                                             true,
                                                             visited_mods_clone.clone(),
                                                         ).await {
                                                             Ok(_) => info!("Successfully processed dependency '{}'", dep_project_id),
                                                             Err(e) => error!("Failed processing dependency '{}': {}", dep_project_id, e),
                                                         }
                                                     } else {
                                                          error!("Could not find primary file for dependency version {} ({})", selected_dep_version.name, selected_dep_version.id);
                                                     }
                                                 } else {
                                                     warn!("Could not find a compatible version for dependency project '{}' matching loader '{}' and game version '{}'. Dependency may be missing.", dep_project_id, profile_loader_str, profile_game_version);
                                                 }
                                             },
                                             Err(e) => error!("Failed to fetch versions for dependency project '{}': {}", dep_project_id, e),
                                         }
                                    } else {
                                        if let Some(dep_version_id_only) = dependency.version_id {
                                            warn!("Dependency has only version_id ('{}'). Attempting to fetch details directly.", dep_version_id_only);
                                            match modrinth::get_version_details(
                                                dep_version_id_only.clone(),
                                            )
                                            .await
                                            {
                                                Ok(dep_version_details) => {
                                                    info!("Successfully fetched details for version '{}': Project='{}'", dep_version_id_only, dep_version_details.project_id);
                                                    if let Some(primary_file) = dep_version_details
                                                        .files
                                                        .iter()
                                                        .find(|f| f.primary)
                                                    {
                                                        match self.add_modrinth_mod_internal(
                                                             profile_id,
                                                             dep_version_details.project_id.clone(),
                                                             dep_version_details.id.clone(),
                                                             primary_file.filename.clone(),
                                                             primary_file.url.clone(),
                                                             primary_file.hashes.sha1.clone(),
                                                             Some(dep_version_details.name.clone()),
                                                             Some(dep_version_details.version_number.clone()),
                                                             Some(dep_version_details.loaders.clone()),
                                                             Some(dep_version_details.game_versions.clone()),
                                                             true,
                                                             visited_mods_clone.clone(),
                                                         ).await {
                                                             Ok(_) => info!("Successfully processed dependency by version_id '{}'", dep_version_id_only),
                                                             Err(e) => error!("Failed processing dependency by version_id '{}': {}", dep_version_id_only, e),
                                                         }
                                                    } else {
                                                        error!("Could not find primary file for dependency version fetched by ID '{}'", dep_version_id_only);
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("Failed to fetch details for dependency version_id '{}': {}. Cannot add dependency.", dep_version_id_only, e);
                                                }
                                            }
                                        } else {
                                            error!("Required dependency is missing project_id and version_id. Cannot resolve. File: {:?}", dependency.file_name);
                                        }
                                    }
                                } else {
                                    // Optional/Incompatible/Embedded dependencies are ignored for now
                                    // info!("Ignoring non-required dependency type: {:?}", dependency.dependency_type);
                                }
                            }
                        } else {
                            warn!("Could not find details for version ID '{}' of project '{}' on Modrinth after fetching versions.", version_id, project_id);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to fetch versions for project '{}' to resolve dependencies: {}",
                            project_id, e
                        );
                    }
                }
            } else {
                info!(
                    "Skipping dependency check for {} ({}) as requested.",
                    display_name_log, version_log
                );
            }

            Ok(())
        })
    }

    async fn insert_mod_from_payload(
        &self,
        payload: &crate::commands::content_command::InstallContentPayload,
    ) -> Result<()> {
        use crate::integrations::unified_mod::ModPlatform;

        let display_name_log = payload.content_name.as_deref().unwrap_or(&payload.project_id);
        let platform_name = match payload.source {
            ModPlatform::Modrinth => "Modrinth",
            ModPlatform::CurseForge => "CurseForge",
        };

        let source = match payload.source {
            ModPlatform::Modrinth => ModSource::Modrinth {
                project_id: payload.project_id.clone(),
                version_id: payload.version_id.clone(),
                file_name: payload.file_name.clone(),
                download_url: payload.download_url.clone(),
                file_hash_sha1: payload.file_hash_sha1.clone(),
            },
            ModPlatform::CurseForge => ModSource::CurseForge {
                project_id: payload.project_id.clone(),
                file_id: payload.version_id.clone(), // For CurseForge, version_id is actually file_id
                file_name: payload.file_name.clone(),
                download_url: payload.download_url.clone(),
                file_hash_sha1: payload.file_hash_sha1.clone(),
                file_fingerprint: payload.file_fingerprint,
            },
        };

        let mut profiles = self.profiles.write().await;

        if let Some(profile) = profiles.get_mut(&payload.profile_id) {
            if !profile.mods.iter().any(|m| m.source == source) {
                info!(
                    "Adding mod {} to profile {}",
                    display_name_log, payload.profile_id
                );

                let new_mod = Mod {
                    id: Uuid::new_v4(),
                    source: source.clone(),
                    enabled: true,
                    display_name: payload.content_name.clone(),
                    version: payload.version_number.clone(),
                    game_versions: payload.game_versions.clone(),
                    file_name_override: None,
                    associated_loader: payload.loaders
                        .clone()
                        .and_then(|l| l.first().and_then(|s| ModLoader::from_str(s).ok())),
                    modpack_origin: None, // Manually added mod
                    updates_enabled: true, // Updates enabled by default
                };
                profile.mods.push(new_mod);
                drop(profiles);
                self.save_profiles().await?;
                info!(
                    "Successfully added {} mod {} to profile {}",
                    platform_name, display_name_log, payload.profile_id
                );
            } else {
                info!(
                    "{} mod {} already exists in profile {}. Skipping addition.",
                    platform_name, display_name_log, payload.profile_id
                );
            }
        } else {
            return Err(AppError::ProfileNotFound(payload.profile_id));
        }

        Ok(())
    }

    // Public wrapper function to add a mod (supports both Modrinth and CurseForge)
    pub async fn add_mod_from_payload(
        &self,
        payload: &crate::commands::content_command::InstallContentPayload,
        add_dependencies: bool,
    ) -> Result<()> {
        use crate::integrations::unified_mod::ModPlatform;

        let display_name_log = payload.content_name.as_deref().unwrap_or(&payload.project_id);
        let platform_name = match payload.source {
            ModPlatform::Modrinth => "Modrinth",
            ModPlatform::CurseForge => "CurseForge",
        };

        info!(
            "Adding {} mod {} to profile {} (dependencies: {})",
            platform_name, display_name_log, payload.profile_id, add_dependencies
        );

        self.insert_mod_from_payload(payload).await?;

        if add_dependencies {
            self.install_dependencies_for_mod(payload, display_name_log, platform_name)
                .await?;
        }

        Ok(())
    }

    // Helper method to install dependencies for a mod
    async fn install_dependencies_for_mod(
        &self,
        payload: &crate::commands::content_command::InstallContentPayload,
        display_name_log: &str,
        platform_name: &str,
    ) -> Result<()> {
        use crate::integrations::unified_mod::{ModPlatform, UnifiedModVersionsParams};

        info!(
            "Installing dependencies for {} mod {} (version: {})",
            platform_name, display_name_log, payload.version_number.as_deref().unwrap_or("unknown")
        );

        // Get version details to find dependencies
        let versions_params = UnifiedModVersionsParams {
            source: payload.source.clone(),
            project_id: payload.project_id.clone(),
            loaders: payload.loaders.clone(),
            game_versions: payload.game_versions.clone(),
            limit: Some(1), // We only need the specific version
            offset: None,
        };

        let versions_response = match crate::integrations::unified_mod::get_mod_versions_unified(versions_params).await {
            Ok(response) => response,
            Err(e) => {
                warn!("Failed to get version details for dependencies: {}", e);
                return Ok(()); // Don't fail the whole operation if dependencies can't be fetched
            }
        };

        if let Some(target_version) = versions_response.versions.into_iter().find(|v| v.id == payload.version_id) {
            info!(
                "Found {} dependencies for {} mod {}",
                target_version.dependencies.len(),
                platform_name,
                display_name_log
            );

            if !target_version.dependencies.is_empty() {
                self.install_missing_dependencies(
                    payload.profile_id,
                    &target_version.dependencies,
                    &payload.source,
                    &target_version.date_published,
                )
                .await?;
            }
        } else {
            warn!("Could not find version {} for dependency resolution", payload.version_id);
        }

        Ok(())
    }

    // Helper method to install CurseForge dependencies
    async fn install_curseforge_dependencies(
        &self,
        profile_id: Uuid,
        file: &crate::integrations::curseforge::CurseForgeFile,
        _parent_mod_name: &str,
    ) -> Result<()> {
        use crate::integrations::curseforge::CurseForgeFileRelationType;

        let profile = self.get_profile(profile_id).await?;
        let profile_loader_str = profile.loader.as_str().to_string();
        let profile_game_version = profile.game_version.clone();

        for dependency in &file.dependencies {
            // Only install required dependencies
            if let Some(relation_type) = CurseForgeFileRelationType::from_u32(dependency.relationType) {
                if relation_type.should_install() {
                    info!("Processing CurseForge dependency: ModId={}, RelationType={}", dependency.modId, relation_type.as_str());

                    // Get dependency mod information
                    match crate::integrations::curseforge::get_mod_info(dependency.modId).await {
                        Ok(dep_mod_info) => {
                            // Get compatible files for this dependency
                            match crate::integrations::curseforge::get_mod_files(
                                dependency.modId,
                                Some(profile_game_version.clone()),
                                None, // We'll filter loaders in the unified conversion
                                None,
                                None,
                                Some(50), // Get first 50 files
                            ).await {
                                Ok(dep_files_response) => {
                                    if let Some(best_file) = dep_files_response.data.into_iter().max_by_key(|f| f.fileDate.clone()) {
                                        // Check if this file supports the required loader
                                        let file_loaders = crate::integrations::unified_mod::extract_loaders_from_game_versions(&best_file.gameVersions);

                                        // Check if the file is compatible with the profile's loader
                                        let is_compatible = if profile_loader_str == "vanilla" {
                                            // Vanilla is compatible with everything
                                            true
                                        } else {
                                            file_loaders.contains(&profile_loader_str)
                                        };

                                        if is_compatible {
                                            // Create dependency payload
                                            let dep_payload = crate::commands::content_command::InstallContentPayload {
                                                profile_id,
                                                project_id: dependency.modId.to_string(),
                                                version_id: best_file.id.to_string(),
                                                file_name: best_file.fileName.clone(),
                                                download_url: best_file.downloadUrl.clone(),
                                                file_hash_sha1: best_file.hashes.iter()
                                                    .find(|h| h.algo == 1) // SHA1 = 1
                                                    .map(|h| h.value.clone()),
                                                file_fingerprint: Some(best_file.fileFingerprint),
                                                content_name: Some(best_file.displayName.clone()),
                                                version_number: Some(best_file.fileName.clone()),
                                                content_type: crate::utils::profile_utils::ContentType::Mod,
                                                loaders: Some(file_loaders),
                                                game_versions: Some(best_file.gameVersions.clone()),
                                                source: crate::integrations::unified_mod::ModPlatform::CurseForge,
                                            };

                                            // Recursively install dependency (without further dependencies to avoid loops)
                                            match Box::pin(self.add_mod_from_payload(&dep_payload, false)).await {
                                                Ok(_) => info!("Successfully installed CurseForge dependency '{}'", dep_mod_info.name),
                                                Err(e) => error!("Failed to install CurseForge dependency '{}': {}", dep_mod_info.name, e),
                                            }
                                        } else {
                                            warn!("CurseForge dependency '{}' (ID: {}) is not compatible with profile loader '{}'", dep_mod_info.name, dependency.modId, profile_loader_str);
                                        }
                                    } else {
                                        warn!("No compatible files found for CurseForge dependency mod ID {}", dependency.modId);
                                    }
                                }
                                Err(e) => error!("Failed to get files for CurseForge dependency '{}': {}", dependency.modId, e),
                            }
                        }
                        Err(e) => error!("Failed to get mod info for CurseForge dependency '{}': {}", dependency.modId, e),
                    }
                }
            }
        }

        Ok(())
    }

    // Helper method to install Modrinth dependencies
    async fn install_modrinth_dependencies(
        &self,
        profile_id: Uuid,
        version: &crate::integrations::modrinth::ModrinthVersion,
        _parent_mod_name: &str,
    ) -> Result<()> {
        use crate::integrations::modrinth::ModrinthDependencyType;

        let profile = self.get_profile(profile_id).await?;
        let profile_loader_str = profile.loader.as_str().to_string();
        let profile_game_version = profile.game_version.clone();

        for dependency in &version.dependencies {
            if dependency.dependency_type == ModrinthDependencyType::Required {
                info!("Processing required Modrinth dependency: Project={:?}, Version={:?}", dependency.project_id, dependency.version_id);

                if let Some(dep_project_id) = &dependency.project_id {
                    // Get compatible versions for the dependency
                    match crate::integrations::modrinth::get_mod_versions(
                        dep_project_id.clone(),
                        Some(vec![profile_loader_str.clone()]),
                        Some(vec![profile_game_version.clone()]),
                    ).await {
                        Ok(dep_versions) => {
                            if let Some(best_version) = dep_versions.iter().max_by_key(|v| &v.date_published) {
                                if let Some(primary_file) = best_version.files.iter().find(|f| f.primary) {
                                    // Create dependency payload
                                    let dep_payload = crate::commands::content_command::InstallContentPayload {
                                        profile_id,
                                        project_id: dep_project_id.clone(),
                                        version_id: best_version.id.clone(),
                                        file_name: primary_file.filename.clone(),
                                        download_url: primary_file.url.clone(),
                                        file_hash_sha1: primary_file.hashes.sha1.clone(),
                                        file_fingerprint: None, // Not used for Modrinth
                                        content_name: Some(best_version.name.clone()),
                                        version_number: Some(best_version.version_number.clone()),
                                        content_type: crate::utils::profile_utils::ContentType::Mod,
                                        loaders: Some(best_version.loaders.clone()),
                                        game_versions: Some(best_version.game_versions.clone()),
                                        source: crate::integrations::unified_mod::ModPlatform::Modrinth,
                                    };

                                    // Recursively install dependency (without further dependencies to avoid loops)
                                    match Box::pin(self.add_mod_from_payload(&dep_payload, false)).await {
                                        Ok(_) => info!("Successfully installed dependency '{}'", dep_project_id),
                                        Err(e) => error!("Failed to install dependency '{}': {}", dep_project_id, e),
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Failed to get versions for dependency '{}': {}", dep_project_id, e),
                    }
                }
            }
        }

        Ok(())
    }

    // Public wrapper function to add a Modrinth mod and its dependencies
    pub async fn add_modrinth_mod(
        &self,
        profile_id: Uuid,
        project_id: String,
        version_id: String,
        file_name: String,
        download_url: String,
        file_hash_sha1: Option<String>,
        // Optional details for better Mod struct population
        mod_name: Option<String>,
        version_number: Option<String>,
        loaders: Option<Vec<String>>,
        game_versions: Option<Vec<String>>,
        add_dependencies: bool, // Allow caller to decide
    ) -> Result<()> {
           // Always use the same behavior for all profiles (add to profile mods + optional deps)
           // if profile.is_standard_version {
           //     let mods_dir = self.get_profile_mods_path(&profile)?;
           //     tokio::fs::create_dir_all(&mods_dir).await?;
           //
           //     let target_path = mods_dir.join(&file_name);
           //     let tmp_path = target_path.with_extension("jar.nrc_tmp");
           //
           //     let mut config = crate::utils::download_utils::DownloadConfig::new().with_streaming(true);
           //     if let Some(sha1) = &file_hash_sha1 { config = config.with_sha1(sha1); }
           //     crate::utils::download_utils::DownloadUtils::download_file(
           //         &download_url,
           //         &tmp_path,
           //         config,
           //     ).await?;
           //     // Atomic move
           //     tokio::fs::rename(&tmp_path, &target_path).await?;
           //
           //     // Optionally install required dependencies if requested
           //     if add_dependencies {
           //         // Fetch version details to read dependencies
           //         if let Ok(ver_details) = modrinth::get_version_details(version_id.clone()).await {
           //             for dep in ver_details.dependencies.iter().filter(|d| d.dependency_type == ModrinthDependencyType::Required) {
           //                 if let Some(dep_project_id) = &dep.project_id {
           //                     // Find a compatible version by loader/profile game version
           //                     if let Ok(dep_versions) = modrinth::get_mod_versions(dep_project_id.clone(), Some(vec![profile.loader.as_str().to_string()]), Some(vec![profile.game_version.clone()])).await {
           //                         if let Some(best) = dep_versions.iter().max_by_key(|v| &v.date_published) {
           //                             if let Some(primary) = best.files.iter().find(|f| f.primary) {
           //                                 let dep_tmp = mods_dir.join(&primary.filename).with_extension("jar.nrc_tmp");
           //                                 let dep_target = mods_dir.join(&primary.filename);
           //                                 let mut cfg = crate::utils::download_utils::DownloadConfig::new().with_streaming(true);
           //                                 if let Some(s) = &primary.hashes.sha1 { cfg = cfg.with_sha1(s); }
           //                                 let _ = crate::utils::download_utils::DownloadUtils::download_file(&primary.url, &dep_tmp, cfg).await;
           //                                 let _ = tokio::fs::rename(&dep_tmp, &dep_target).await;
           //                             }
           //                         }
           //                     }
           //                 }
           //             }
           //         }
           //     }
           //     Ok(())
           // } else {
               // Use the same behavior for all profiles (add to profile mods + optional deps)
               self.add_modrinth_mod_internal(
                   profile_id,
                   project_id,
                   version_id,
                   file_name,
                   download_url,
                   file_hash_sha1,
                   mod_name,
                   version_number,
                   loaders,
                   game_versions,
                   add_dependencies,
                   HashSet::new(),
               )
               .await
           // }
    }

    // Set the enabled status of a specific mod within a profile
    pub async fn set_mod_enabled(
        &self,
        profile_id: Uuid,
        mod_id: Uuid,
        enabled: bool,
    ) -> Result<()> {
        info!(
            "Setting mod {} enabled status to {} for profile {}",
            mod_id, enabled, profile_id
        );

        let mut profiles = self.profiles.write().await;

        if let Some(profile) = profiles.get_mut(&profile_id) {
            if let Some(mod_to_update) = profile.mods.iter_mut().find(|m| m.id == mod_id) {
                if mod_to_update.enabled != enabled {
                    mod_to_update.enabled = enabled;
                    drop(profiles);
                    self.save_profile(profile_id).await?;
                    info!(
                        "Successfully updated mod {} enabled status in profile {}",
                        mod_id, profile_id
                    );
                } else {
                    info!(
                        "Mod {} enabled status already {}. No change needed.",
                        mod_id, enabled
                    );
                }
                Ok(())
            } else {
                Err(AppError::Other(format!(
                    "Mod with ID {} not found in profile {}",
                    mod_id, profile_id
                )))
            }
        } else {
            Err(AppError::ProfileNotFound(profile_id))
        }
    }

    /// Sets the updates_enabled status for a specific mod in a profile
    pub async fn set_mod_updates_enabled(
        &self,
        profile_id: Uuid,
        mod_id: Uuid,
        updates_enabled: bool,
    ) -> Result<()> {
        info!(
            "Setting mod {} updates_enabled status to {} for profile {}",
            mod_id, updates_enabled, profile_id
        );

        let mut profiles = self.profiles.write().await;

        if let Some(profile) = profiles.get_mut(&profile_id) {
            if let Some(mod_to_update) = profile.mods.iter_mut().find(|m| m.id == mod_id) {
                if mod_to_update.updates_enabled != updates_enabled {
                    mod_to_update.updates_enabled = updates_enabled;
                    drop(profiles);
                    self.save_profile(profile_id).await?;
                    info!(
                        "Successfully updated mod {} updates_enabled status in profile {}",
                        mod_id, profile_id
                    );
                } else {
                    info!(
                        "Mod {} updates_enabled status already {}. No change needed.",
                        mod_id, updates_enabled
                    );
                }
                Ok(())
            } else {
                Err(AppError::Other(format!(
                    "Mod with ID {} not found in profile {}",
                    mod_id, profile_id
                )))
            }
        } else {
            Err(AppError::ProfileNotFound(profile_id))
        }
    }

    // Remove a specific mod from a profile
    pub async fn delete_mod(&self, profile_id: Uuid, mod_id: Uuid) -> Result<()> {
        info!("Deleting mod {} from profile {}", mod_id, profile_id);

        let mut profiles = self.profiles.write().await;

        if let Some(profile) = profiles.get_mut(&profile_id) {
            let initial_len = profile.mods.len();
            profile.mods.retain(|m| m.id != mod_id);
            let final_len = profile.mods.len();

            if final_len < initial_len {
                drop(profiles);
                self.save_profiles().await?;
                info!(
                    "Successfully deleted mod {} from profile {}",
                    mod_id, profile_id
                );
                Ok(())
            } else {
                Err(AppError::Other(format!(
                    "Mod with ID {} not found in profile {}",
                    mod_id, profile_id
                )))
            }
        } else {
            Err(AppError::ProfileNotFound(profile_id))
        }
    }

    // Set the enabled/disabled status of a specific mod within a Norisk Pack for a profile's specific context
    // Removed: set_norisk_mod_status - No pre-installed modpacks

    // Utility Funktionen
    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.ensure_profiles_loaded().await?;
        let profiles = self.profiles.read().await;
        Ok(profiles.values().cloned().collect())
    }

    /// Lightweight list for library UI: mods stripped, mod count returned separately.
    pub async fn list_profiles_without_mods(&self) -> Result<Vec<(Profile, usize)>> {
        self.ensure_profiles_loaded().await?;
        let profiles = self.profiles.read().await;
        Ok(profiles
            .values()
            .map(|p| {
                let mod_count = p.mods.len();
                let slim = Profile {
                    mods: Vec::new(),
                    ..p.clone()
                };
                (slim, mod_count)
            })
            .collect())
    }

    pub async fn search_profiles(&self, query: &str) -> Result<Vec<Profile>> {
        let query = query.to_lowercase();
        let profiles = self.profiles.read().await;
        Ok(profiles
            .values()
            .filter(|p| p.name.to_lowercase().contains(&query))
            .cloned()
            .collect())
    }

        /// Updates the version of a specific CurseForge mod instance within a profile,
    /// after checking for the presence of required dependencies (by project ID).
    /// Automatically adds missing dependencies.
    pub async fn update_profile_curseforge_mod_version(
        &self,
        profile_id: Uuid,
        mod_id: Uuid,
        new_version_details: &crate::integrations::curseforge::CurseForgeFile,
    ) -> Result<()> {
        info!(
            "Attempting to update CurseForge mod instance {} in profile {} to version '{}' (ID: {})",
            mod_id, profile_id, new_version_details.displayName, new_version_details.id
        );

        let mut profiles = self.profiles.write().await;

        let profile = profiles.get_mut(&profile_id).ok_or_else(|| {
            error!(
                "Profile {} not found during CurseForge mod update attempt.",
                profile_id
            );
            AppError::ProfileNotFound(profile_id)
        })?;

        info!(
            "Checking required dependencies for new CurseForge version {}...",
            new_version_details.id
        );
        let existing_project_ids: HashSet<String> = profile
            .mods
            .iter()
            .filter_map(|m| match &m.source {
                ModSource::CurseForge { project_id, .. } => Some(project_id.clone()),
                _ => None,
            })
            .collect();

        // Track missing dependencies to install them later
        let mut missing_deps = Vec::new();

        for dependency in &new_version_details.dependencies {
            // Only process required dependencies
            if let Some(relation_type) = crate::integrations::curseforge::CurseForgeFileRelationType::from_u32(dependency.relationType) {
                if relation_type.should_install() {
                    if !existing_project_ids.contains(&dependency.modId.to_string()) {
                        info!(
                            "Required dependency project '{}' is missing in profile {}. Will install it automatically.",
                            dependency.modId, profile_id
                        );
                        missing_deps.push(dependency.modId);
                    } else {
                        info!(
                            "Required dependency project '{}' found in profile.",
                            dependency.modId
                        );
                    }
                }
            }
        }

        // Now update the mod
        let mod_to_update_index = profile.mods.iter().position(|m| m.id == mod_id);

        if let Some(index) = mod_to_update_index {
            let mod_to_update = &mut profile.mods[index];

            if let ModSource::CurseForge {
                project_id: old_project_id,
                ..
            } = &mod_to_update.source
            {
                if old_project_id != &new_version_details.modId.to_string() {
                    error!(
                        "Project ID mismatch when updating CurseForge mod {}! Expected '{}', got '{}'. Aborting update.",
                         mod_id, old_project_id, new_version_details.modId
                    );
                    return Err(AppError::Other(format!(
                        "Project ID mismatch for CurseForge mod {}",
                        mod_id
                    )));
                }

                info!(
                    "Updating CurseForge mod instance {} from version {} to {} using file '{}'",
                    mod_id,
                    mod_to_update.version.as_deref().unwrap_or("?"),
                    new_version_details.displayName,
                    new_version_details.fileName
                );

                mod_to_update.source = ModSource::CurseForge {
                    project_id: new_version_details.modId.to_string(),
                    file_id: new_version_details.id.to_string(),
                    file_name: new_version_details.fileName.clone(),
                    download_url: new_version_details.downloadUrl.clone(),
                    file_hash_sha1: new_version_details.hashes.iter()
                        .find(|h| h.algo == 1) // SHA1 = 1
                        .map(|h| h.value.clone()),
                    file_fingerprint: Some(new_version_details.fileFingerprint),
                };

                mod_to_update.version = Some(new_version_details.displayName.clone());
                mod_to_update.game_versions = Some(new_version_details.gameVersions.clone());
                // For CurseForge, we don't have explicit loader info in the file, so we keep the existing one
                // or try to determine it from game versions
                if mod_to_update.associated_loader.is_none() {
                    mod_to_update.associated_loader = crate::integrations::unified_mod::extract_loaders_from_game_versions(&new_version_details.gameVersions)
                        .first()
                        .and_then(|s| ModLoader::from_str(s).ok());
                }

                info!("CurseForge mod instance {} updated successfully in memory.", mod_id);
            } else {
                error!(
                    "Mod instance {} in profile {} is not a CurseForge mod.",
                    mod_id, profile_id
                );
                return Err(AppError::Other(format!(
                    "Mod {} is not a CurseForge mod",
                    mod_id
                )));
            }
        } else {
            error!(
                "Mod instance with ID {} not found in profile {} during update.",
                mod_id, profile_id
            );
            return Err(AppError::ModNotFoundInProfile { profile_id, mod_id });
        }

        // Save changes to the profile first
        drop(profiles);
        self.save_profiles().await?;
        info!(
            "Profile {} saved after updating CurseForge mod {}.",
            profile_id, mod_id
        );

        // Now install any missing dependencies
        if !missing_deps.is_empty() {
            let display_name_log = new_version_details.displayName.as_str();
            info!("Installing {} missing CurseForge dependencies", missing_deps.len());
            match self.install_curseforge_dependencies(profile_id, &new_version_details, display_name_log).await {
                Ok(_) => info!("Successfully installed CurseForge dependencies for '{}'", display_name_log),
                Err(e) => error!("Failed to install some CurseForge dependencies for '{}': {}", display_name_log, e),
            }
        } else {
            info!("No missing CurseForge dependencies to install for '{}'", new_version_details.displayName);
        }

        Ok(())
    }

    /// Updates the version of a specific Modrinth mod instance within a profile,
    /// after checking for the presence of required dependencies (by project ID).
    /// Automatically adds missing dependencies.
    pub async fn update_profile_modrinth_mod_version(
        &self,
        profile_id: Uuid,
        mod_id: Uuid,
        new_version_details: &ModrinthVersion,
    ) -> Result<()> {
        info!(
            "Attempting to update Modrinth mod instance {} in profile {} to version '{}' ({})",
            mod_id, profile_id, new_version_details.name, new_version_details.id
        );

        let mut profiles = self.profiles.write().await;

        let profile = profiles.get_mut(&profile_id).ok_or_else(|| {
            error!(
                "Profile {} not found during mod update attempt.",
                profile_id
            );
            AppError::ProfileNotFound(profile_id)
        })?;

        info!(
            "Checking required dependencies for new version {}...",
            new_version_details.id
        );
        let existing_project_ids: HashSet<String> = profile
            .mods
            .iter()
            .filter_map(|m| match &m.source {
                ModSource::Modrinth { project_id, .. } => Some(project_id.clone()),
                _ => None,
            })
            .collect();

        // Track missing dependencies to install them later
        let mut missing_deps = Vec::new();

        for dependency in &new_version_details.dependencies {
            if dependency.dependency_type == ModrinthDependencyType::Required {
                if let Some(dep_project_id) = &dependency.project_id {
                    if !existing_project_ids.contains(dep_project_id) {
                        info!(
                            "Required dependency project '{}' is missing in profile {}. Will install it automatically.",
                            dep_project_id, profile_id
                        );
                        missing_deps.push((dep_project_id.clone(), dependency.version_id.clone()));
                    } else {
                        info!(
                            "Required dependency project '{}' found in profile.",
                            dep_project_id
                        );
                    }
                } else {
                    warn!(
                        "Required dependency found without a project_id in version {}: {:?}",
                        new_version_details.id, dependency
                    );
                }
            }
        }

        // Now update the mod
        let mod_to_update_index = profile.mods.iter().position(|m| m.id == mod_id);

        if let Some(index) = mod_to_update_index {
            let mod_to_update = &mut profile.mods[index];

            if let ModSource::Modrinth {
                project_id: old_project_id,
                ..
            } = &mod_to_update.source
            {
                if old_project_id != &new_version_details.project_id {
                    error!(
                        "Project ID mismatch when updating mod {}! Expected '{}', got '{}'. Aborting update.",
                         mod_id, old_project_id, new_version_details.project_id
                    );
                    return Err(AppError::Other(format!(
                        "Project ID mismatch for mod {}",
                        mod_id
                    )));
                }

                match new_version_details.files.iter().find(|f| f.primary) {
                    Some(primary_file) => {
                        info!(
                            "Updating mod instance {} from version {} to {} using file '{}'",
                            mod_id,
                            mod_to_update.version.as_deref().unwrap_or("?"),
                            new_version_details.version_number,
                            primary_file.filename
                        );

                        mod_to_update.source = ModSource::Modrinth {
                            project_id: new_version_details.project_id.clone(),
                            version_id: new_version_details.id.clone(),
                            file_name: primary_file.filename.clone(),
                            download_url: primary_file.url.clone(),
                            file_hash_sha1: primary_file.hashes.sha1.clone(),
                        };

                        mod_to_update.version = Some(new_version_details.version_number.clone());
                        mod_to_update.game_versions =
                            Some(new_version_details.game_versions.clone());
                        mod_to_update.associated_loader = new_version_details
                            .loaders
                            .first()
                            .and_then(|s| ModLoader::from_str(s).ok());

                        info!("Mod instance {} updated successfully in memory.", mod_id);
                    }
                    None => {
                        error!(
                            "No primary file found for Modrinth version {} (ID: {})",
                            new_version_details.name, new_version_details.id
                        );
                        return Err(AppError::ModrinthPrimaryFileNotFound {
                            version_id: new_version_details.id.clone(),
                        });
                    }
                }
            } else {
                error!(
                    "Mod instance {} in profile {} is not a Modrinth mod.",
                    mod_id, profile_id
                );
                return Err(AppError::Other(format!(
                    "Mod {} is not a Modrinth mod",
                    mod_id
                )));
            }
        } else {
            error!(
                "Mod instance with ID {} not found in profile {} during update.",
                mod_id, profile_id
            );
            return Err(AppError::ModNotFoundInProfile { profile_id, mod_id });
        }

        // Save changes to the profile first
        drop(profiles);
        self.save_profiles().await?;
        info!(
            "Profile {} saved after updating mod {}.",
            profile_id, mod_id
        );

        // Now install any missing dependencies
        let mut installed_deps = 0;
        let mut failed_deps = 0;

        for (dep_project_id, dep_version_id_opt) in missing_deps {
            info!("Installing missing dependency: {}", dep_project_id);

            // Get the profile's game version and loader for compatibility check
            let profile = self.get_profile(profile_id).await?;
            let profile_loader = profile.loader.as_str().to_string();

            // First, try to find the specific version if one was specified
            if let Some(version_id) = dep_version_id_opt {
                match modrinth::get_version_details(version_id.clone()).await {
                    Ok(dep_version) => {
                        if let Some(primary_file) = dep_version.files.iter().find(|f| f.primary) {
                            match self
                                .add_modrinth_mod(
                                    profile_id,
                                    dep_version.project_id.clone(),
                                    dep_version.id.clone(),
                                    primary_file.filename.clone(),
                                    primary_file.url.clone(),
                                    primary_file.hashes.sha1.clone(),
                                    Some(dep_version.name.clone()),
                                    Some(dep_version.version_number.clone()),
                                    Some(dep_version.loaders.clone()),
                                    Some(dep_version.game_versions.clone()),
                                    false, // don't recursively add dependencies here
                                )
                                .await
                            {
                                Ok(_) => {
                                    info!("Successfully added dependency: {}", dep_project_id);
                                    installed_deps += 1;
                                }
                                Err(e) => {
                                    error!("Failed to add dependency {}: {}", dep_project_id, e);
                                    failed_deps += 1;
                                }
                            }
                            continue;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to fetch version details for dependency {} ({}): {}. Trying to find compatible version.", 
                            dep_project_id, version_id, e);
                    }
                }
            }

            // If specific version not found or no version specified, find compatible version
            match modrinth::get_mod_versions(
                dep_project_id.clone(),
                Some(vec![profile_loader.clone()]),
                Some(vec![profile.game_version.clone()]),
            )
            .await
            {
                Ok(versions) => {
                    if let Some(best_version) = versions.iter().max_by_key(|v| &v.date_published) {
                        if let Some(primary_file) = best_version.files.iter().find(|f| f.primary) {
                            match self
                                .add_modrinth_mod(
                                    profile_id,
                                    best_version.project_id.clone(),
                                    best_version.id.clone(),
                                    primary_file.filename.clone(),
                                    primary_file.url.clone(),
                                    primary_file.hashes.sha1.clone(),
                                    Some(best_version.name.clone()),
                                    Some(best_version.version_number.clone()),
                                    Some(best_version.loaders.clone()),
                                    Some(best_version.game_versions.clone()),
                                    false, // don't recursively add dependencies here
                                )
                                .await
                            {
                                Ok(_) => {
                                    info!("Successfully added dependency: {}", dep_project_id);
                                    installed_deps += 1;
                                }
                                Err(e) => {
                                    error!("Failed to add dependency {}: {}", dep_project_id, e);
                                    failed_deps += 1;
                                }
                            }
                        } else {
                            error!("No primary file found for dependency version");
                            failed_deps += 1;
                        }
                    } else {
                        error!(
                            "No compatible version found for dependency {}",
                            dep_project_id
                        );
                        failed_deps += 1;
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to fetch versions for dependency {}: {}",
                        dep_project_id, e
                    );
                    failed_deps += 1;
                }
            }
        }

        info!(
            "Dependency installation complete: {} installed, {} failed",
            installed_deps, failed_deps
        );

        Ok(())
    }

    /// Returns the instance path for a given profile ID by looking it up.
    pub async fn get_profile_instance_path(&self, profile_id: Uuid) -> Result<PathBuf> {
        //log::debug!("Attempting to get instance path for profile {}", profile_id);
        let profiles_map = self.profiles.read().await;
        match profiles_map.get(&profile_id) {
            Some(profile) => {
                log::trace!(
                    "Found instance path {:?} for profile {}",
                    &profile.path,
                    profile_id
                );
                // Reuse the logic by calling the new method
                self.calculate_instance_path_for_profile(profile)
            }
            None => {
                // Profile not found - standard versions no longer available
                log::warn!("Profile {} not found.", profile_id);
                Err(AppError::ProfileNotFound(profile_id))
            }
        }
    }

    /// Helper function to check if a group should NOT use shared Minecraft folder
    fn is_isolated_group(group_name: &str) -> bool {
        let normalized = group_name.to_lowercase();
        normalized == "server" || normalized == "modpacks"
    }


    /// Sanitizes a group name for safe filesystem usage
    fn sanitize_group_name(group_name: &str) -> String {
        sanitize_filename::sanitize(group_name.to_lowercase())
    }

    /// Builds the default path using profile.path segments
    pub fn build_path_from_profile_path(profile: &Profile) -> PathBuf {
        let mut path = default_profile_path();
        
        // Explicitly split profile.path by '/' and push each segment
        for segment in profile.path.split('/') {
            if !segment.is_empty() {
                path.push(segment);
            }
        }
        path
    }

    /// Calculates the group directory for a profile when using shared folder logic.
    /// Returns the directory path based on the profile's group and Minecraft version.
    pub fn calculate_group_directory(&self, profile: &Profile) -> Result<PathBuf> {
        if let Some(group) = &profile.group {
                // Other custom groups go to "groups/{sanitized_group_name}"
                let sanitized_group = Self::sanitize_group_name(group);
                Ok(default_profile_path().join("groups").join(sanitized_group))
        } else {
            // No group, use the original logic with profile.path
            Ok(Self::build_path_from_profile_path(profile))
        }
    }

    /// Calculates the instance path for a given Profile object based on its properties.
    /// This method does NOT check if the profile exists in the manager.
    pub fn calculate_instance_path_for_profile(&self, profile: &Profile) -> Result<PathBuf> {
        log::trace!(
            "Calculating instance path for profile '{}' (Raw profile.path: '{}', Version: {}, Group: {:?})",
            profile.name,
            profile.path, // Log the raw profile.path string
            profile.game_version,
            profile.group
        );

        // Determine final path based on shared folder logic and group
        let final_path = if profile.should_use_shared_minecraft_folder() {
            // Profile should use shared folder - use group directory logic
            log::trace!("Profile '{}' should use shared Minecraft folder, using group directory", profile.name);
            self.calculate_group_directory(profile)?
        } else {
            // Profile should NOT use shared folder (isolated) - use original logic with profile.path
            log::trace!("Profile '{}' should not use shared Minecraft folder, using isolated path logic", profile.name);
            Self::build_path_from_profile_path(profile)
        };

        log::trace!(
            "Constructed final path for profile '{}': {:?}",
            profile.name,
            final_path
        );
        Ok(final_path)
    }

    /// Returns the path to the mods directory for a given profile.
    pub fn get_profile_mods_path(&self, profile: &Profile) -> Result<PathBuf> {
        let instance_path = self.calculate_instance_path_for_profile(profile)?;
        let mods_path = instance_path.join("mods");
        log::debug!(
            "Calculated mods path for profile '{}' (Loader: {:?}, Game Version: {}, Standard: {}, Uses Shared: {}): {:?}",
            profile.name,
            profile.loader,
            profile.game_version,
            profile.is_standard_version,
            profile.should_use_shared_minecraft_folder(),
            mods_path
        );
        Ok(mods_path)
    }

    /// Returns the path to the custom_mods directory for a given profile ID.
    /// The directory is located next to the .minecraft directory within the instance folder.
    pub async fn get_profile_custom_mods_path(&self, profile_id: Uuid) -> Result<PathBuf> {
        log::debug!(
            "Attempting to get custom_mods path for profile {}",
            profile_id
        );
        let minecraft_dir_path = self.get_profile_instance_path(profile_id).await?;

        let custom_mods_dir = minecraft_dir_path.join("custom_mods");
        log::trace!(
            "Determined custom_mods path {:?} for profile {}",
            custom_mods_dir,
            profile_id
        );
        Ok(custom_mods_dir)
    }

    /// Lists relevant custom mods found in the profile's `custom_mods` directory.
    /// Only includes files ending in `.jar` or `.jar.disabled`.
    pub async fn list_custom_mods(&self, profile: &Profile) -> Result<Vec<CustomModInfo>> {
        let custom_mods_path = self.get_profile_custom_mods_path(profile.id).await?;
        let mut custom_mods = Vec::new();

        if !custom_mods_path.exists() {
            log::debug!(
                "Custom mods directory {:?} does not exist for profile {}. Returning empty list.",
                custom_mods_path,
                profile.id
            );
            // Attempt to create it for next time?
            if let Err(e) = tokio::fs::create_dir_all(&custom_mods_path).await {
                log::warn!(
                    "Failed to create custom_mods directory {:?}: {}",
                    custom_mods_path,
                    e
                );
            }
            return Ok(custom_mods); // Return empty list if dir doesn't exist initially
        }

        let mut dir_entries = tokio::fs::read_dir(&custom_mods_path).await.map_err(|e| {
            log::error!(
                "Failed to read custom_mods directory {:?}: {}",
                custom_mods_path,
                e
            );
            AppError::Io(e)
        })?;

        while let Some(entry_result) = dir_entries.next_entry().await.map_err(|e| {
            log::error!(
                "Failed to read entry in custom_mods directory {:?}: {}",
                custom_mods_path,
                e
            );
            AppError::Io(e)
        })? {
            let path = entry_result.path();
            if path.is_file() {
                if let Some(filename_str) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip hidden files
                    if filename_str.starts_with(".") {
                        log::trace!("Skipping hidden file in custom_mods: {:?}", path);
                        continue;
                    }

                    let is_enabled = !filename_str.ends_with(".disabled");
                    let base_filename_opt = if is_enabled {
                        if filename_str.ends_with(".jar") {
                            Some(filename_str.to_string())
                        } else {
                            None // Skip if enabled but not a .jar
                        }
                    } else {
                        // If disabled, check if the base name ends with .jar
                        if let Some(base) = filename_str.strip_suffix(".disabled") {
                            if base.ends_with(".jar") {
                                Some(base.to_string())
                            } else {
                                None // Skip if disabled but base is not .jar
                            }
                        } else {
                            None // Should not happen if ends_with(".disabled") is true
                        }
                    };

                    if let Some(base_filename) = base_filename_opt {
                        custom_mods.push(CustomModInfo {
                            filename: base_filename,
                            is_enabled,
                            path: path.clone(),
                        });
                    } else {
                        log::trace!(
                            "Skipping file in custom_mods (not .jar or .jar.disabled): {:?}",
                            path
                        );
                    }
                }
            }
        }

        log::info!(
            "Found {} relevant custom mod file(s) in {:?}",
            custom_mods.len(),
            custom_mods_path
        );
        Ok(custom_mods)
    }

    /// Sets the enabled/disabled state of a custom mod by renaming it.
    /// Accepts the base filename (e.g., "OptiFine.jar") and the desired enabled state.
    /// Returns Ok(()) if the state is successfully set or already correct.
    pub async fn set_custom_mod_enabled(
        &self,
        profile_id: Uuid,
        filename: String,
        set_enabled: bool,
    ) -> Result<()> {
        // Changed return type to Result<()>
        let custom_mods_path = self.get_profile_custom_mods_path(profile_id).await?;

        // Ensure the filename itself doesn't end with .disabled - we expect the base name.
        if filename.ends_with(".disabled") {
            log::warn!("set_custom_mod_enabled called with filename ending in .disabled: '{}'. Please provide the base filename.", filename);
            return Err(AppError::Other(format!(
                "Invalid filename provided to set_custom_mod_enabled: {}",
                filename
            )));
        }

        let enabled_path = custom_mods_path.join(&filename);
        let disabled_filename = format!("{}.disabled", filename);
        let disabled_path = custom_mods_path.join(&disabled_filename);

        let current_enabled = enabled_path.exists();
        let currently_exists_as_disabled = disabled_path.exists();

        if !current_enabled && !currently_exists_as_disabled {
            // Neither file exists
            log::error!(
                "Could not find custom mod file '{}' or '{}' in {:?}",
                filename,
                disabled_filename,
                custom_mods_path
            );
            return Err(AppError::Other(format!(
                "Custom mod file not found: {} in {:?}",
                filename, custom_mods_path
            )));
        }

        // Check if the state is already the desired one
        if current_enabled == set_enabled {
            log::info!(
                "Custom mod '{}' is already {}. No action needed.",
                filename,
                if set_enabled { "enabled" } else { "disabled" }
            );
            return Ok(());
        }

        // Perform the rename if the state needs changing
        if set_enabled {
            // --> Enable it: Rename file.disabled to file
            log::info!(
                "Enabling custom mod: Renaming {:?} to {:?}",
                disabled_path,
                enabled_path
            );
            tokio::fs::rename(&disabled_path, &enabled_path)
                .await
                .map_err(|e| {
                    log::error!(
                        "Failed to rename custom mod {:?} to {:?}: {}",
                        disabled_path,
                        enabled_path,
                        e
                    );
                    AppError::Io(e)
                })?;
        } else {
            // --> Disable it: Rename file to file.disabled
            log::info!(
                "Disabling custom mod: Renaming {:?} to {:?}",
                enabled_path,
                disabled_path
            );
            tokio::fs::rename(&enabled_path, &disabled_path)
                .await
                .map_err(|e| {
                    log::error!(
                        "Failed to rename custom mod {:?} to {:?}: {}",
                        enabled_path,
                        disabled_path,
                        e
                    );
                    AppError::Io(e)
                })?;
        }

        log::info!(
            "Successfully set custom mod '{}' state to: {}",
            filename,
            if set_enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Imports local .jar files selected by the user into the specified profile.
    /// It tries to identify mods via Modrinth hash lookup and adds them as Modrinth mods.
    /// If a mod is not found on Modrinth or an error occurs during lookup,
    /// it falls back to copying the file into the profile's custom_mods directory.
    pub async fn import_local_mods_to_profile(
        &self,
        profile_id: Uuid,
        paths_enums: Vec<FilePath>,
    ) -> Result<()> {
        info!(
            "Processing {} selected files for import into profile {}",
            paths_enums.len(),
            profile_id
        );

        // --- Collect Hashes and Paths ---
        let mut hashes_to_check: Vec<String> = Vec::new();
        let mut path_map: HashMap<String, PathBuf> = HashMap::new(); // Map: sha1 -> PathBuf
        let mut path_conversion_errors = 0;

        for file_path_enum in paths_enums {
            let src_path_buf = match file_path_enum.into_path() {
                Ok(path) => path,
                Err(e) => {
                    error!("Failed to convert selected file path: {}", e);
                    path_conversion_errors += 1;
                    continue;
                }
            };

            // Calculate hash using the async util function
            match hash_utils::calculate_sha1(&src_path_buf).await {
                Ok(hash) => {
                    // Avoid checking the same hash multiple times if user selects same file twice
                    if !path_map.contains_key(&hash) {
                        hashes_to_check.push(hash.clone());
                        path_map.insert(hash, src_path_buf);
                    } else {
                        warn!(
                            "Skipping duplicate file selection: {:?}",
                            src_path_buf.file_name().unwrap_or_default()
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to calculate SHA1 for {:?}: {}", src_path_buf, e);
                    path_conversion_errors += 1;
                }
            }
        }

        if hashes_to_check.is_empty() {
            info!(
                "No valid files found to process after hashing/path conversion for profile {}.",
                profile_id
            );
            // Still return Ok, as no critical error occurred, just nothing to import
            return Ok(());
        }

        info!(
            "Attempting to look up {} unique hashes on Modrinth for profile {}...",
            hashes_to_check.len(),
            profile_id
        );

        // --- Modrinth Bulk Lookup ---
        // Use qualified path if modrinth module is imported directly
        let versions_map_result =
            crate::integrations::modrinth::get_versions_by_hashes(hashes_to_check, "sha1").await;

        // --- Process Results ---
        // Use normal mods directory for direct file placement
        let profile = self.get_profile(profile_id).await?;
        let mods_dir = if profile.loader == ModLoader::Fabric {
            self.get_profile_mods_path(&profile)?
        } else {
            self.get_profile_custom_mods_path(profile_id).await?
        };
        // Ensure mods_dir exists ONCE
        fs::create_dir_all(&mods_dir)
            .await
            .map_err(AppError::Io)?;

        let mut modrinth_added_count: u64 = 0;
        let mut custom_added_count: u64 = 0;
        let mut skipped_count: u64 = 0; // For already existing custom mods
        let mut error_count: u64 = path_conversion_errors;

        match versions_map_result {
            Ok(versions_map) => {
                info!(
                    "Successfully received results for {} hashes from Modrinth for profile {}.",
                    versions_map.len(),
                    profile_id
                );
                for (hash, src_path_buf) in path_map {
                    // Iterate through the originally collected paths/hashes
                    if let Some(modrinth_version) = versions_map.get(&hash) {
                        // Found on Modrinth
                        log::debug!(
                            "Processing Modrinth match for hash {} for profile {}: {:?}",
                            hash,
                            profile_id,
                            src_path_buf.file_name().unwrap_or_default()
                        );
                        if let Some(primary_file) =
                            modrinth_version.files.iter().find(|f| f.primary)
                        {
                            match self
                                .add_modrinth_mod(
                                    // Use self
                                    profile_id,
                                    modrinth_version.project_id.clone(),
                                    modrinth_version.id.clone(),
                                    primary_file.filename.clone(),
                                    primary_file.url.clone(),
                                    primary_file.hashes.sha1.clone(),
                                    Some(modrinth_version.name.clone()),
                                    Some(modrinth_version.version_number.clone()),
                                    Some(modrinth_version.loaders.clone()),
                                    Some(modrinth_version.game_versions.clone()),
                                    false, // add_dependencies = true
                                )
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Successfully added '{}' as Modrinth mod to profile {}.",
                                        primary_file.filename, profile_id
                                    );
                                    modrinth_added_count += 1;
                                }
                                Err(e) => {
                                    // Log error, count it, but continue processing other files
                                    error!("Failed to add identified Modrinth mod '{}' to profile {}: {}", primary_file.filename, profile_id, e);
                                    error_count += 1;
                                }
                            }
                        } else {
                            // Log error, count it, and fallback
                            error!("Modrinth version {} found for hash {}, but no primary file found. Falling back to custom mod import for profile {} - {:?}.", modrinth_version.id, hash, profile_id, src_path_buf.file_name().unwrap_or_default());
                            error_count += 1; // Count as error because Modrinth add failed essentially
                            path_utils::copy_as_custom_mod(
                                &src_path_buf,
                                &mods_dir,
                                profile_id,
                                &mut custom_added_count,
                                &mut skipped_count,
                            )
                            .await;
                        }
                    } else {
                        // Not found in Modrinth results -> Treat as custom mod
                        log::info!("Mod {:?} (hash: {}) not found on Modrinth for profile {}. Importing as custom mod.", src_path_buf.file_name().unwrap_or_default(), hash, profile_id);
                        path_utils::copy_as_custom_mod(
                            &src_path_buf,
                            &mods_dir,
                            profile_id,
                            &mut custom_added_count,
                            &mut skipped_count,
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to perform bulk hash lookup on Modrinth for profile {}: {}. Falling back to importing all as custom mods.", profile_id, e);
                error_count += path_map.len() as u64; // Count all as errors for Modrinth lookup
                                                      // Fallback: Try adding all as custom mods
                for (_hash, src_path_buf) in path_map {
                    path_utils::copy_as_custom_mod(
                        &src_path_buf,
                        &mods_dir,
                        profile_id,
                        &mut custom_added_count,
                        &mut skipped_count,
                    )
                    .await;
                }
            }
        }

        log::info!(
            "Import process finished for profile {}. Added as Modrinth: {}, Added as Custom: {}, Skipped (exists/other): {}, Hashing/Path/Lookup Errors: {}",
            profile_id, modrinth_added_count, custom_added_count, skipped_count, error_count
        );

        // No critical error occurred during the process itself, return Ok.
        // Individual file errors are logged and counted.
        Ok(())
    }

    /// Ensures profiles are loaded from disk if not already loaded, performing migrations if needed.
    /// This method is used to avoid race conditions where profile operations are called before profiles are loaded.
    async fn load_profiles_once(&self) -> Result<()> {
        {
            let profiles_guard = self.profiles.read().await;
            if profiles_guard.is_empty() {
                info!("ProfileManager: Profiles not loaded yet, loading them now...");
                drop(profiles_guard);

                let mut loaded_profiles = self.store.load_all().await?;
                let migration_count =
                    crate::utils::migration_utils::migrate_profiles(&mut loaded_profiles);

                {
                    let mut profiles_write_guard = self.profiles.write().await;
                    *profiles_write_guard = loaded_profiles;
                }

                if migration_count > 0 {
                    info!(
                        "ProfileManager: Persisting profiles after {} migration change(s)...",
                        migration_count
                    );
                    self.save_profiles().await?;
                }

                info!("ProfileManager: Profiles loaded successfully.");
            }
        }
        Ok(())
    }

    async fn ensure_profiles_loaded(&self) -> Result<()> {
        let _guard = self.loading.lock().await;
        self.load_profiles_once().await
    }

    /// Creates an editable copy of a standard profile for user customization
    async fn create_editable_copy_from_standard(&self, standard_profile: &Profile) -> Result<Uuid> {
        let mut editable_copy = standard_profile.clone();
           
        // Link back to original standard profile
        editable_copy.source_standard_profile_id = Some(standard_profile.id);
        
        // Update timestamps
        editable_copy.created = chrono::Utc::now();
        editable_copy.last_played = None;
        
        // Reset state to not installed for user copy
        editable_copy.state = ProfileState::NotInstalled;

        
        // Clear all mods from standard profile - users should add their own
        editable_copy.mods = Vec::new();

        // Create the profile using existing create_profile method
        let new_id = self.create_profile(editable_copy).await?;
        
        Ok(new_id)
    }

    /// Updates an existing copy with forced fields from the standard profile
    /// Returns true if any changes were made, false otherwise
    async fn update_copy_with_forced_fields(&self, copy_id: Uuid, standard_profile: &Profile) -> Result<bool> {
        let mut profiles = self.profiles.write().await;
        
        if let Some(copy) = profiles.get_mut(&copy_id) {
            let mut changed = false;
            
            // Force update name if different
            if copy.name != standard_profile.name {
                info!("Updating name for copy {}: '{}' -> '{}'", copy_id, copy.name, standard_profile.name);
                copy.name = standard_profile.name.clone();
                changed = true;
            }
            
            // Force update group if different
            if copy.group != standard_profile.group {
                info!("Updating group for copy {}: {:?} -> {:?}", copy_id, copy.group, standard_profile.group);
                copy.group = standard_profile.group.clone();
                changed = true;
            }
            
            // Force update game version if different
            if copy.game_version != standard_profile.game_version {
                info!("Updating game version for copy {}: '{}' -> '{}'", copy_id, copy.game_version, standard_profile.game_version);
                copy.game_version = standard_profile.game_version.clone();
                changed = true;
            }
            
            // Force update loader if different
            if copy.loader != standard_profile.loader {
                info!("Updating loader for copy {}: {:?} -> {:?}", copy_id, copy.loader, standard_profile.loader);
                copy.loader = standard_profile.loader.clone();
                changed = true;
            }
            
            // Force update loader version if different
            if copy.loader_version != standard_profile.loader_version {
                info!("Updating loader version for copy {}: {:?} -> {:?}", copy_id, copy.loader_version, standard_profile.loader_version);
                copy.loader_version = standard_profile.loader_version.clone();
                changed = true;
            }
            
            // Force update description if different
            if copy.description != standard_profile.description {
                info!("Updating description for copy {}", copy_id);
                copy.description = standard_profile.description.clone();
                changed = true;
            }
            
            
            // Force update banner if different
            if copy.banner != standard_profile.banner {
                info!("Updating banner for copy {}", copy_id);
                copy.banner = standard_profile.banner.clone();
                changed = true;
            }

              // Force update banner if different
            if copy.background != standard_profile.background {
                info!("Updating background for copy {}", copy_id);
                copy.background = standard_profile.background.clone();
                changed = true;
            }
            
            // Force update is_standard_version if different
            if copy.is_standard_version != standard_profile.is_standard_version {
                info!("Updating is_standard_version for copy {}: {} -> {}", copy_id, copy.is_standard_version, standard_profile.is_standard_version);
                copy.is_standard_version = standard_profile.is_standard_version;
                changed = true;
            }
            
            // Force update path if different
            if copy.path != standard_profile.path {
                info!("Updating path for copy {}: '{}' -> '{}'", copy_id, copy.path, standard_profile.path);
                copy.path = standard_profile.path.clone();
                changed = true;
            }
            
            
            if changed {
                drop(profiles);
                self.save_profiles().await?;
                info!("Saved forced field updates for copy {}", copy_id);
            }
            
            Ok(changed)
        } else {
            Err(AppError::ProfileNotFound(copy_id))
        }
    }

    /// Updates a mod in a profile using SwitchContentVersionPayload
    /// This method handles the unified version update process
    pub async fn update_mod_with_switch_content_version_payload(
        &self,
        profile_id: Uuid,
        payload: &crate::commands::content_command::SwitchContentVersionPayload,
    ) -> Result<()> {
        info!(
            "Updating mod in profile {} using unified version switch",
            profile_id
        );

        let mut profiles = self.profiles.write().await;

        let profile = profiles.get_mut(&profile_id).ok_or_else(|| {
            error!(
                "Profile {} not found during unified mod update attempt.",
                profile_id
            );
            AppError::ProfileNotFound(profile_id)
        })?;

        let current_item = payload.current_item_details.as_ref().ok_or_else(|| {
            AppError::InvalidInput("Missing current_item_details in payload.".to_string())
        })?;

        // Find the mod to update
        let mod_to_update_index = profile.mods.iter().position(|m| {
            match &m.source {
                ModSource::Modrinth { project_id, .. } => {
                    // Check if ID matches or project_id from modrinth_info matches
                    current_item.id.as_ref() == Some(&m.id.to_string()) ||
                    (current_item.modrinth_info.as_ref().map(|info| &info.project_id) == Some(project_id))
                },
                ModSource::CurseForge { project_id, .. } => {
                    // Check if ID matches or project_id from curseforge_info matches
                    current_item.id.as_ref() == Some(&m.id.to_string()) ||
                    (current_item.curseforge_info.as_ref().map(|info| &info.project_id) == Some(project_id))
                },
                _ => false,
            }
        });

        if let Some(index) = mod_to_update_index {
            let mod_to_update = &mut profile.mods[index];

            // Update the mod source based on the platform from unified version
            match payload.new_version_details.source {
                crate::integrations::unified_mod::ModPlatform::Modrinth => {
                    // Find primary file
                    let primary_file = payload.new_version_details.files.iter()
                        .find(|f| f.primary)
                        .or_else(|| payload.new_version_details.files.first())
                        .ok_or_else(|| AppError::InvalidInput("No primary file found in unified version".to_string()))?;

                    mod_to_update.source = ModSource::Modrinth {
                        project_id: payload.new_version_details.project_id.clone(),
                        version_id: payload.new_version_details.id.clone(),
                        file_name: primary_file.filename.clone(),
                        download_url: primary_file.url.clone(),
                        file_hash_sha1: primary_file.hashes.get("sha1").cloned(),
                    };

                    mod_to_update.version = Some(payload.new_version_details.version_number.clone());
                    mod_to_update.game_versions = Some(payload.new_version_details.game_versions.clone());
                },
                crate::integrations::unified_mod::ModPlatform::CurseForge => {
                    // Find primary file
                    let primary_file = payload.new_version_details.files.iter()
                        .find(|f| f.primary)
                        .or_else(|| payload.new_version_details.files.first())
                        .ok_or_else(|| AppError::InvalidInput("No primary file found in unified version".to_string()))?;

                    mod_to_update.source = ModSource::CurseForge {
                        project_id: payload.new_version_details.project_id.clone(),
                        file_id: payload.new_version_details.id.clone(),
                        file_name: primary_file.filename.clone(),
                        download_url: primary_file.url.clone(),
                        file_hash_sha1: primary_file.hashes.get("sha1").cloned(),
                        file_fingerprint: primary_file.fingerprint,
                    };

                    mod_to_update.version = Some(payload.new_version_details.version_number.clone());
                    mod_to_update.game_versions = Some(payload.new_version_details.game_versions.clone());
                },
            }

            // Update display name if available
            if mod_to_update.display_name.is_none() {
                mod_to_update.display_name = Some(payload.new_version_details.name.clone());
            }

            info!("Successfully updated mod {} in profile {}", mod_to_update.id, profile_id);
        } else {
            error!(
                "Mod not found in profile {} for update with unified version",
                profile_id
            );
            return Err(AppError::ModNotFoundInProfile {
                profile_id,
                mod_id: current_item.id.as_ref()
                    .and_then(|id_str| Uuid::parse_str(id_str).ok())
                    .unwrap_or(Uuid::nil()),
            });
        }

        drop(profiles);
        self.save_profiles().await?;

        info!(
            "Profile {} saved after updating mod with unified version.",
            profile_id
        );

        // Install missing dependencies if any
        if !payload.new_version_details.dependencies.is_empty() {
            info!("Processing {} dependencies for updated mod", payload.new_version_details.dependencies.len());
            if let Err(e) = self.install_missing_dependencies(
                profile_id,
                &payload.new_version_details.dependencies,
                &payload.new_version_details.source,
                &payload.new_version_details.date_published,
            )
            .await
            {
                error!("Failed to install dependencies: {}", e);
                // Don't fail the entire operation if dependency installation fails
            }
        }

        Ok(())
    }

    /// Checks if a dependency mod is already installed in the profile
    fn is_dependency_installed(
        &self,
        profile: &Profile,
        dependency_project_id: &str,
    ) -> bool {
        profile.mods.iter().any(|mod_entry| {
            match &mod_entry.source {
                ModSource::Modrinth { project_id, .. } => {
                    project_id == dependency_project_id && mod_entry.enabled
                },
                ModSource::CurseForge { project_id, .. } => {
                    project_id == dependency_project_id && mod_entry.enabled
                },
                _ => false,
            }
        })
    }

    fn installed_dependency(
        &self,
        profile: &Profile,
        dependency_project_id: &str,
    ) -> Option<(Uuid, bool)> {
        profile.mods.iter().find_map(|mod_entry| {
            let project_id = match &mod_entry.source {
                ModSource::Modrinth { project_id, .. } => project_id.as_str(),
                ModSource::CurseForge { project_id, .. } => project_id.as_str(),
                _ => return None,
            };
            if project_id == dependency_project_id {
                Some((mod_entry.id, mod_entry.enabled))
            } else {
                None
            }
        })
    }

    /// Installs missing dependencies for a mod (batch-resolved tree).
    async fn install_missing_dependencies(
        &self,
        profile_id: Uuid,
        dependencies: &[crate::integrations::unified_mod::UnifiedDependency],
        platform: &crate::integrations::unified_mod::ModPlatform,
        parent_date: &str,
    ) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        self.install_missing_dependencies_seen(
            profile_id,
            dependencies,
            platform,
            parent_date,
            &mut seen,
        )
        .await
    }

    async fn install_missing_dependencies_seen(
        &self,
        profile_id: Uuid,
        dependencies: &[crate::integrations::unified_mod::UnifiedDependency],
        platform: &crate::integrations::unified_mod::ModPlatform,
        parent_date: &str,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        use crate::integrations::mod_dependencies::{
            resolve_required_dependencies_seen, DependencyTarget, DEPENDENCY_DEPTH,
        };

        let profile = self.get_profile(profile_id).await?;
        let target = DependencyTarget {
            loader: profile.loader.as_str().to_string(),
            game_version: profile.game_version.clone(),
        };

        let resolved = resolve_required_dependencies_seen(
            platform,
            dependencies,
            parent_date,
            &target,
            DEPENDENCY_DEPTH,
            seen,
        )
        .await;

        for dependency in resolved {
            if let Some((existing_id, enabled)) =
                self.installed_dependency(&profile, &dependency.project_id)
            {
                if enabled {
                    info!(
                        "Dependency {} already installed, skipping",
                        dependency.project_id
                    );
                } else {
                    info!(
                        "Dependency {} is installed but disabled, re-enabling",
                        dependency.project_id
                    );
                    if let Err(e) = self.set_mod_enabled(profile_id, existing_id, true).await {
                        error!(
                            "Failed to re-enable dependency '{}': {}",
                            dependency.project_id, e
                        );
                    }
                }
                continue;
            }

            let version = &dependency.version;
            let file = version
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| version.files.first());

            let dep_payload = crate::commands::content_command::InstallContentPayload {
                profile_id,
                project_id: dependency.project_id.clone(),
                version_id: version.id.clone(),
                file_name: file
                    .map(|f| f.filename.clone())
                    .unwrap_or_else(|| format!("{}.jar", dependency.project_id)),
                download_url: file.map(|f| f.url.clone()).unwrap_or_default(),
                file_hash_sha1: file.and_then(|f| f.hashes.get("sha1").cloned()),
                file_fingerprint: file.and_then(|f| f.fingerprint),
                content_name: Some(dependency.display_name()),
                version_number: Some(version.version_number.clone()),
                content_type: crate::utils::profile_utils::ContentType::Mod,
                loaders: Some(version.loaders.clone()),
                game_versions: Some(version.game_versions.clone()),
                source: platform.clone(),
            };

            if let Err(e) = self.insert_mod_from_payload(&dep_payload).await {
                error!(
                    "Failed to install dependency '{}': {}",
                    dependency.project_id, e
                );
            }
        }

        Ok(())
    }

    /// Deletes a custom mod file (either .jar or .jar.disabled) from the profile's custom_mods directory.
    pub async fn delete_custom_mod_file(&self, profile_id: Uuid, filename: &str) -> Result<()> {
        info!(
            "Attempting to delete custom mod file '{}' for profile {}",
            filename, profile_id
        );

        // Note: Validation that filename doesn't end with .disabled should happen in the caller (command)

        let custom_mods_dir = self.get_profile_custom_mods_path(profile_id).await?;

        let enabled_path = custom_mods_dir.join(filename); // filename is the base name
        let disabled_filename = format!("{}.disabled", filename);
        let disabled_path = custom_mods_dir.join(&disabled_filename);

        let file_to_delete = if enabled_path.exists() {
            Some(enabled_path)
        } else if disabled_path.exists() {
            Some(disabled_path)
        } else {
            None
        };

        if let Some(path_to_delete) = file_to_delete {
            log::debug!("Deleting custom mod file at path: {:?}", path_to_delete);
            fs::remove_file(&path_to_delete).await.map_err(|e| {
                log::error!(
                    "Failed to delete custom mod file {:?}: {}",
                    path_to_delete,
                    e
                );
                AppError::Io(e)
            })?; // Propagate IO error
            info!(
                "Successfully deleted custom mod file corresponding to '{}' for profile {}.",
                filename, profile_id
            );
            Ok(())
        } else {
            log::warn!(
                "Custom mod file '{}' not found (neither enabled nor disabled) in profile {}.",
                filename,
                profile_id
            );
            // Return specific error indicating file not found
            Err(AppError::Profile(format!(
                "Custom mod {} in profile {}",
                filename, profile_id
            )))
        }
    }
}

#[async_trait]
impl PostInitializationHandler for ProfileManager {
    async fn on_state_ready(&self, _app_handle: Arc<tauri::AppHandle>) -> Result<()> {
        info!("ProfileManager: on_state_ready called. Loading profiles...");
        // PRIORITY 0: Create backup BEFORE ANYTHING else (including loading)
        info!("ProfileManager: Creating pre-load backup of profiles.json...");
        if self.profiles_path.exists() {
            match backup_utils::create_backup(&self.profiles_path, Some("profiles"), &self.backup_config).await {
                Ok(backup_path) => {
                    info!("ProfileManager: Pre-load backup created: {:?}", backup_path);
                }
                Err(e) => {
                    warn!("ProfileManager: Failed to create pre-load backup: {}", e);
                    // Continue anyway - don't fail the whole operation
                }
            }
        } else {
            info!("ProfileManager: profiles.json doesn't exist yet - no backup needed at this stage");
        }

        {
            let _guard = self.loading.lock().await;
            self.migrate_if_needed().await?;
            self.load_profiles_once().await?;
        }

        if let Ok((profiles, mods)) = self.store.counts().await {
            info!(
                "ProfileManager: serving {} profiles and {} mods from the database",
                profiles, mods
            );
        }

        info!("ProfileManager: Successfully loaded profiles in on_state_ready.");

        // Fire-and-forget: purge trashed items and old backups after init
        let backup_config_clone = self.backup_config.clone();
        let profiles_path_clone = self.profiles_path.clone();
        tauri::async_runtime::spawn(async move {
            let seconds_30_days = 30 * 24 * 60 * 60;

            // Clean up trash
            if let Err(e) = crate::utils::trash_utils::purge_expired(seconds_30_days).await {
                log::warn!("Trash purge after init failed: {}", e);
            }

            // Clean up old backups for profiles category using our specific config
            if let Err(e) = crate::utils::backup_utils::cleanup_old_backups(
                &profiles_path_clone,
                Some("profiles"),
                &backup_config_clone,
            ).await {
                log::warn!("Profile backup cleanup after init failed: {}", e);
            }
        });

        Ok(())
    }
}

/// Helper function to determine the definitive filename for a mod defined within a Profile.
pub fn should_persist(profile: &Profile, transient: &HashSet<Uuid>) -> bool {
    if transient.contains(&profile.id) {
        return false;
    }
    !(profile.is_standard_version && profile.source_standard_profile_id.is_none())
}

fn content_hash(profile: &Profile) -> Result<u64> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    crate::state::profile_store::canonical_value(profile)?
        .to_string()
        .hash(&mut hasher);
    Ok(hasher.finish())
}

/// Helper function to determine the definitive filename for a mod defined within a Profile.
pub fn get_profile_mod_filename(source: &ModSource) -> crate::error::Result<String> {
    match source {
        ModSource::Modrinth { file_name, .. } => Ok(file_name.clone()),
        ModSource::CurseForge { file_name, .. } => Ok(file_name.clone()),
        ModSource::Local { file_name } => Ok(file_name.clone()),
        ModSource::Url { file_name, url } => file_name.clone().ok_or_else(|| {
            crate::error::AppError::Other(format!("Filename missing for URL mod source: {}", url))
        }),
        ModSource::Maven { coordinates, .. } => Err(crate::error::AppError::Other(format!(
            "Cannot determine filename for profile Maven mod source: {}",
            coordinates
        ))),
        ModSource::Embedded { name } => Err(crate::error::AppError::Other(format!(
            "Cannot get filename for embedded mod source: {}",
            name
        ))),
    }
}

pub fn default_profile_path() -> PathBuf {
    // Check cache first (same system as meta_dir)
    if let Ok(guard) = crate::config::CUSTOM_GAME_DIR_CACHE.read() {
        if let Some(cached_value) = guard.as_ref() {
            if let Some(custom_dir) = cached_value {
                return custom_dir.join("profiles");
            }
        }
    }
    
    // Fallback to standard logic
    LAUNCHER_DIRECTORY.data_dir().join("profiles")
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            java_path: None,
            use_custom_java_path: false,
            use_overwrite_loader_version: false,
            overwrite_loader_version: None,
            memory: MemorySettings::default(),
            resolution: None,
            fullscreen: false,
            extra_game_args: Vec::new(),
            custom_jvm_args: None, // Standardmäßig keine benutzerdefinierten JVM-Args
            quick_play_path: None,
        }
    }
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            min: 1024, // 1GB
            max: 2048, // 2GB
        }
    }
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: 854,
            height: 480,
        }
    }
}
