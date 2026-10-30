use std::path::PathBuf;

use crate::error::{AppError, Result};
use crate::minecraft::dto::forge_install_profile::ForgeInstallProfile;
use crate::minecraft::dto::neo_forge_install_profile::NeoForgeInstallProfile;
use crate::minecraft::launch::forge_patcher::ForgePatcher;
use crate::state::event_state::EventType;
use uuid::Uuid;

pub struct NeoForgePatcher {
    inner: ForgePatcher,
}

fn neo_install_profile_as_forge(profile: &NeoForgeInstallProfile) -> Result<ForgeInstallProfile> {
    let value = serde_json::to_value(profile)
        .map_err(|e| AppError::NeoForgeError(format!("Failed to encode install profile: {}", e)))?;
    serde_json::from_value(value).map_err(|e| {
        AppError::NeoForgeError(format!("Failed to decode install profile for patching: {}", e))
    })
}

impl NeoForgePatcher {
    pub fn new(java_path: PathBuf, minecraft_version: &str) -> Self {
        Self {
            inner: ForgePatcher::new(java_path, minecraft_version)
                .with_processor_event_type(EventType::InstallingNeoForge),
        }
    }

    pub fn with_event_id(mut self, event_id: Uuid) -> Self {
        self.inner = self.inner.with_event_id(event_id);
        self
    }

    pub fn with_profile_id(mut self, profile_id: Uuid) -> Self {
        self.inner = self.inner.with_profile_id(profile_id);
        self
    }

    pub async fn apply_processors(
        &self,
        install_profile: &NeoForgeInstallProfile,
        minecraft_version: &str,
        is_client: bool,
        installer_path: &PathBuf,
    ) -> Result<()> {
        let forge_profile = neo_install_profile_as_forge(install_profile)?;
        self.inner
            .apply_processors(
                &forge_profile,
                minecraft_version,
                is_client,
                installer_path,
            )
            .await
            .map_err(|e| match e {
                AppError::ForgeError(message) => AppError::NeoForgeError(message),
                other => other,
            })
    }
}
