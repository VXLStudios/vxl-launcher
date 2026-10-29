use crate::config::{ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::error::Result;
use crate::minecraft::dto::piston_meta::DownloadInfo;
use crate::utils::download_utils::{DownloadConfig, DownloadUtils};
use log::info;
use std::path::PathBuf;
use tokio::fs;

const VERSIONS_DIR: &str = "versions";

pub struct MinecraftClientDownloadService {
    base_path: PathBuf,
    verify_hashes: bool,
}

impl MinecraftClientDownloadService {
    pub fn new() -> Self {
        let base_path = LAUNCHER_DIRECTORY.meta_dir().join(VERSIONS_DIR);
        Self {
            base_path,
            verify_hashes: false,
        }
    }

    pub fn with_verify_hashes(mut self, verify_hashes: bool) -> Self {
        self.verify_hashes = verify_hashes;
        self
    }

    pub async fn download_client(
        &self,
        client_info: &DownloadInfo,
        version_id: &str,
    ) -> Result<()> {
        let version_dir = self.base_path.join(version_id);
        let target_path = version_dir.join(format!("{}.jar", version_id));

        fs::create_dir_all(&version_dir).await?;

        info!("Downloading client jar for version: {}", version_id);

        let mut config = DownloadConfig::new()
            .with_size(client_info.size as u64)
            .with_hash_existing_files(self.verify_hashes)
            .with_streaming(true)
            .with_retries(3);

        if self.verify_hashes && !client_info.sha1.is_empty() {
            config = config.with_sha1(client_info.sha1.clone());
        }

        DownloadUtils::download_file(&client_info.url, &target_path, config).await?;

        info!("Downloaded client jar to: {}", target_path.display());
        Ok(())
    }
}
