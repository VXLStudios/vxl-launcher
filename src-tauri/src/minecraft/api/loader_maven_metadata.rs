use crate::config::{HTTP_CLIENT, ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::error::{AppError, Result};
use log::{debug, error, info};
use quick_xml::de::from_str;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use tokio::fs as tokio_fs;

pub struct LoaderMavenApiConfig {
    pub metadata_url: &'static str,
    pub cache_subdir: &'static str,
    pub cache_filename: &'static str,
    pub loader_label: &'static str,
}

async fn fetch_and_cache_metadata<T: DeserializeOwned + Send>(
    base_url: &str,
    cache_path: &Path,
    loader_label: &str,
) -> Result<T> {
    debug!("Fetching {} metadata from: {}", loader_label, base_url);

    let response = HTTP_CLIENT.get(base_url).send().await.map_err(|e| {
        AppError::ForgeError(format!("Failed to fetch {} versions: {}", loader_label, e))
    })?;

    if !response.status().is_success() {
        return Err(AppError::ForgeError(format!(
            "Failed to fetch {} versions: Status {}",
            loader_label,
            response.status()
        )));
    }

    let xml_content = response.text().await.map_err(|e| {
        AppError::ForgeError(format!("Failed to read {} response: {}", loader_label, e))
    })?;

    let metadata: T = from_str(&xml_content).map_err(|e| {
        AppError::ForgeError(format!("Failed to parse {} metadata: {}", loader_label, e))
    })?;

    if let Err(e) = tokio_fs::write(cache_path, &xml_content).await {
        error!("Failed to write {} cache: {}", loader_label, e);
    } else {
        debug!("Cached {} metadata: {:?}", loader_label, cache_path);
    }

    info!("Successfully fetched {} metadata", loader_label);
    Ok(metadata)
}

async fn background_update<T: DeserializeOwned + Send + 'static>(
    base_url: String,
    cache_path: PathBuf,
    loader_label: &'static str,
) {
    debug!("[BG] Updating {} metadata", loader_label);
    if let Err(e) =
        fetch_and_cache_metadata::<T>(&base_url, &cache_path, loader_label).await
    {
        error!("[BG] Failed to update {} cache: {}", loader_label, e);
    }
}

pub async fn get_cached_maven_metadata<T: DeserializeOwned + Send + 'static>(
    config: &LoaderMavenApiConfig,
) -> Result<T> {
    let cache_dir = LAUNCHER_DIRECTORY.meta_dir().join(config.cache_subdir);
    if !cache_dir.exists() {
        std::fs::create_dir_all(&cache_dir).unwrap_or_else(|e| {
            error!("Failed to create {} cache directory: {}", config.loader_label, e);
        });
    }

    let cache_path = cache_dir.join(config.cache_filename);
    let base_url = config.metadata_url.to_string();

    if cache_path.exists() {
        debug!(
            "Cache hit for {} metadata: {:?}",
            config.loader_label, cache_path
        );

        match tokio_fs::read_to_string(&cache_path).await {
            Ok(cached_xml) => match from_str::<T>(&cached_xml) {
                Ok(cached_metadata) => {
                    let base_url_clone = base_url.clone();
                    let cache_path_clone = cache_path.clone();
                    let label = config.loader_label;
                    tokio::spawn(async move {
                        background_update::<T>(base_url_clone, cache_path_clone, label).await;
                    });
                    return Ok(cached_metadata);
                }
                Err(e) => {
                    error!(
                        "Failed to parse cached {} metadata: {}",
                        config.loader_label, e
                    );
                }
            },
            Err(e) => {
                error!("Failed to read {} cache: {}", config.loader_label, e);
            }
        }
    }

    debug!("Cache miss for {} metadata, fetching...", config.loader_label);
    fetch_and_cache_metadata(&base_url, &cache_path, config.loader_label).await
}
