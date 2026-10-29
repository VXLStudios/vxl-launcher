use crate::config::{ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::error::{AppError, Result};
use async_zip::error::ZipError;
use async_zip::tokio::read::seek::ZipFileReader;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::AsyncReadExt;
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use log::debug;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::fs::File;

pub async fn write_atomic<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref().to_path_buf();
    let contents = contents.as_ref().to_vec();

    tokio::task::spawn_blocking(move || write_atomic_sync(&path, &contents))
        .await
        .map_err(|e| AppError::Other(format!("Atomic write task failed: {e}")))?
}

pub fn write_atomic_sync(path: &Path, contents: &[u8]) -> Result<()> {
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };

    let mut tmp = tempfile::NamedTempFile::new_in(&dir).map_err(AppError::Io)?;
    tmp.write_all(contents).map_err(AppError::Io)?;
    tmp.as_file().sync_all().map_err(AppError::Io)?;
    tmp.persist(path).map_err(|e| AppError::Io(e.error))?;
    Ok(())
}

/// Helper function to read a specific zip entry by index and encode it as Base64
async fn read_zip_entry_as_base64(
    zip: &mut ZipFileReader<tokio::io::BufReader<tokio::fs::File>>,
    entry_index: usize,
    entry_filename_for_error: &str,
) -> Result<String> {
    let mut entry_reader = zip.reader_with_entry(entry_index).await.map_err(|e| {
        AppError::ArchiveReadError(format!(
            "Failed to create reader for entry {}: {}",
            entry_filename_for_error, e
        ))
    })?;

    let mut buffer = Vec::new();
    entry_reader.read_to_end(&mut buffer).await.map_err(|e| {
        AppError::ArchiveReadError(format!(
            "Failed to read content of {}: {}",
            entry_filename_for_error, e
        ))
    })?;

    let base64_string = STANDARD.encode(&buffer);
    Ok(base64_string)
}

/// Finds the first `.png` file within a zip or jar archive and returns its content as a Base64 encoded string.
/// Prioritizes pack.png, then icon.png in root, then other root PNGs, then any subdirectory PNG.
///
/// # Arguments
///
/// * `archive_path` - The path to the `.zip` or `.jar` file.
///
/// # Returns
///
/// A `Result` containing the Base64 encoded string of the first PNG found, or an `AppError`.
pub async fn find_first_png_in_archive_as_base64(archive_path: &Path) -> Result<String> {
    if !archive_path.exists() {
        debug!("Archive not found at path: {}", archive_path.display());
        return Err(AppError::FileNotFound(archive_path.to_path_buf()));
    }
    debug!(
        "Attempting to find PNG in archive: {}",
        archive_path.display()
    );

    let file = File::open(archive_path)
        .await
        .map_err(|e| AppError::Io(e))?;
    let mut reader = tokio::io::BufReader::new(file);

    let mut zip = ZipFileReader::with_tokio(reader).await.map_err(|e| {
        AppError::ArchiveReadError(format!(
            "Failed to read archive {}: {}",
            archive_path.display(),
            e
        ))
    })?;

    let entries = zip.file().entries().to_vec(); // Clone to allow mutable borrow of zip later

    let mut root_pack_png_idx: Option<usize> = None;
    let mut root_icon_png_idx: Option<usize> = None;
    let mut first_other_root_png_idx: Option<usize> = None;
    let mut first_subdir_png_idx: Option<usize> = None;

    for index in 0..entries.len() {
        let entry = match entries.get(index) {
            Some(e) => e,
            None => continue, // Should not happen if iterating up to entries.len()
        };

        let filename = match entry.filename().as_str() {
            Ok(s) => s,
            Err(_) => {
                debug!("Skipping entry with non-UTF8 filename at index {}", index);
                continue; // Skip non-UTF8 filenames
            }
        };

        if !filename.to_lowercase().ends_with(".png") {
            continue; // Not a PNG file
        }

        let is_root_file = !filename.contains('/');

        if is_root_file {
            if filename.eq_ignore_ascii_case("pack.png") {
                if root_pack_png_idx.is_none() {
                    debug!("Found potential root 'pack.png': {}", filename);
                    root_pack_png_idx = Some(index);
                }
            } else if filename.eq_ignore_ascii_case("icon.png") {
                if root_icon_png_idx.is_none() {
                    debug!("Found potential root 'icon.png': {}", filename);
                    root_icon_png_idx = Some(index);
                }
            } else {
                if first_other_root_png_idx.is_none() {
                    debug!("Found potential other root PNG: {}", filename);
                    first_other_root_png_idx = Some(index);
                }
            }
        } else {
            // Is a subdirectory file
            if first_subdir_png_idx.is_none() {
                debug!("Found potential subdirectory PNG: {}", filename);
                first_subdir_png_idx = Some(index);
            }
        }
    }

    // Determine which index to use based on priority
    let final_idx_to_read: Option<usize> = root_pack_png_idx
        .or(root_icon_png_idx)
        .or(first_other_root_png_idx)
        .or(first_subdir_png_idx);

    if let Some(index_to_read) = final_idx_to_read {
        let entry_to_read = entries.get(index_to_read).unwrap(); // Safe due to previous checks
        let filename_for_error = entry_to_read
            .filename()
            .as_str()
            .unwrap_or("[unknown filename]")
            .to_string(); // Fallback for error

        debug!(
            "Selected PNG to read: {} (index {})",
            filename_for_error, index_to_read
        );
        return read_zip_entry_as_base64(&mut zip, index_to_read, &filename_for_error).await;
    }

    debug!("No PNG found in archive: {}", archive_path.display());
    Err(AppError::PngNotFoundInArchive(archive_path.to_path_buf()))
}

fn archive_icon_cache_dir() -> PathBuf {
    LAUNCHER_DIRECTORY.meta_dir().join("mod_icon_cache")
}

async fn archive_icon_cache_key(archive_path: &Path) -> Result<String> {
    let metadata = tokio::fs::metadata(archive_path)
        .await
        .map_err(AppError::Io)?;
    let modified = metadata
        .modified()
        .map_err(AppError::Io)?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::Other("Invalid file modified time".to_string()))?
        .as_secs();
    let size = metadata.len();
    let mut hasher = Sha256::new();
    hasher.update(archive_path.to_string_lossy().as_bytes());
    hasher.update(modified.to_le_bytes());
    hasher.update(size.to_le_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// Returns `Ok(Some(icon))` on cache hit, `Ok(None)` when cached as having no icon,
/// and `Err(AppError::FileNotFound)` when there is no cache entry yet.
pub async fn lookup_cached_archive_icon(archive_path: &Path) -> Result<Option<String>> {
    let cache_key = archive_icon_cache_key(archive_path).await?;
    let cache_dir = archive_icon_cache_dir();
    let icon_path = cache_dir.join(format!("{cache_key}.b64"));
    let miss_path = cache_dir.join(format!("{cache_key}.none"));

    if miss_path.exists() {
        return Ok(None);
    }

    if icon_path.exists() {
        let cached = tokio::fs::read_to_string(&icon_path)
            .await
            .map_err(AppError::Io)?;
        return Ok(Some(cached));
    }

    Err(AppError::FileNotFound(archive_path.to_path_buf()))
}

pub async fn store_cached_archive_icon(
    archive_path: &Path,
    icon: Option<&str>,
) -> Result<()> {
    let cache_key = archive_icon_cache_key(archive_path).await?;
    let cache_dir = archive_icon_cache_dir();
    if !cache_dir.exists() {
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .map_err(AppError::Io)?;
    }

    let icon_path = cache_dir.join(format!("{cache_key}.b64"));
    let miss_path = cache_dir.join(format!("{cache_key}.none"));

    match icon {
        Some(base64_icon) => {
            tokio::fs::write(&icon_path, base64_icon)
                .await
                .map_err(AppError::Io)?;
            let _ = tokio::fs::remove_file(&miss_path).await;
        }
        None => {
            tokio::fs::write(&miss_path, b"")
                .await
                .map_err(AppError::Io)?;
            let _ = tokio::fs::remove_file(&icon_path).await;
        }
    }

    Ok(())
}

pub async fn find_first_png_in_archive_as_base64_cached(archive_path: &Path) -> Result<String> {
    match lookup_cached_archive_icon(archive_path).await {
        Ok(Some(cached_icon)) => return Ok(cached_icon),
        Ok(None) => {
            return Err(AppError::PngNotFoundInArchive(archive_path.to_path_buf()));
        }
        Err(AppError::FileNotFound(_)) => {}
        Err(error) => return Err(error),
    }

    match find_first_png_in_archive_as_base64(archive_path).await {
        Ok(icon) => {
            let _ = store_cached_archive_icon(archive_path, Some(&icon)).await;
            Ok(icon)
        }
        Err(AppError::PngNotFoundInArchive(_)) => {
            let _ = store_cached_archive_icon(archive_path, None).await;
            Err(AppError::PngNotFoundInArchive(archive_path.to_path_buf()))
        }
        Err(error) => Err(error),
    }
}


/// Reads the content of a file into a string, replacing invalid UTF-8 sequences.
///
/// If the file doesn't exist, returns `Ok("".to_string())`.
///
/// # Arguments
///
/// * `file_path` - The path to the file to read.
///
/// # Returns
///
/// A `Result` containing the file content as a `String`, or an `AppError` if reading fails.
pub async fn read_file_content_lossy(file_path: &Path) -> Result<String> {
    // Check if the file exists
    if !file_path.exists() {
        log::warn!(
            "File not found at {}, returning empty content.",
            file_path.display()
        );
        return Ok("".to_string()); // Return empty string if file not found
    }

    // Read file content as bytes first to handle potential invalid UTF-8
    match tokio::fs::read(file_path).await {
        Ok(bytes) => {
            // Convert bytes to string, replacing invalid sequences
            let content = String::from_utf8_lossy(&bytes).to_string();
            log::info!(
                "Successfully read {} bytes (lossy converted) from file {}",
                bytes.len(),
                file_path.display()
            );
            Ok(content)
        }
        Err(e) => {
            log::error!("Failed to read file content {}: {}", file_path.display(), e);
            Err(AppError::Io(e))
        }
    }
}

/// Reads the content of a log file (`.log` or `.log.gz`) into a string.
/// Supports plain text and gzip compressed files.
///
/// If the file doesn't exist, returns `Ok("".to_string())`.
/// Invalid UTF-8 sequences are replaced lossily.
///
/// # Arguments
///
/// * `log_path` - The path to the log file.
///
/// # Returns
///
/// A `Result` containing the log content as a `String`, or an `AppError` if reading or decompression fails.
pub async fn read_log_file_content(log_path: &Path) -> Result<String> {
    // Check if the file exists
    if !log_path.exists() {
        log::warn!(
            "Log file not found at {}, returning empty content.",
            log_path.display()
        );
        return Ok("".to_string()); // Return empty string if file not found
    }

    let filename = log_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if filename.ends_with(".log.gz") {
        // Handle gzipped file
        log::debug!("Reading gzipped log file: {}", log_path.display());
        match tokio::fs::File::open(log_path).await {
            Ok(file) => {
                let buf_reader = tokio::io::BufReader::new(file);
                let mut decoder = async_compression::tokio::bufread::GzipDecoder::new(buf_reader);
                let mut decompressed_bytes = Vec::new();
                match tokio::io::copy(&mut decoder, &mut decompressed_bytes).await {
                    Ok(bytes_copied) => {
                        let raw_content = String::from_utf8_lossy(&decompressed_bytes).to_string();
                        // Mask sensitive information before returning
                        let safe_content = crate::utils::security_utils::mask_sensitive_data(&raw_content);
                        log::info!(
                            "Successfully read and decompressed {} bytes from gzipped log file {}",
                            bytes_copied,
                            log_path.display()
                        );
                        Ok(safe_content)
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to decompress gzipped log file {}: {}",
                            log_path.display(),
                            e
                        );
                        Err(AppError::Io(e))
                    }
                }
            }
            Err(e) => {
                log::error!(
                    "Failed to open gzipped log file {}: {}",
                    log_path.display(),
                    e
                );
                Err(AppError::Io(e))
            }
        }
    } else if filename.ends_with(".log") {
        // Handle plain text file using existing function, then mask sensitive data
        log::debug!("Reading plain text log file: {}", log_path.display());
        match read_file_content_lossy(log_path).await {
            Ok(raw_content) => {
                // Mask sensitive information before returning
                let safe_content = crate::utils::security_utils::mask_sensitive_data(&raw_content);
                Ok(safe_content)
            }
            Err(e) => Err(e),
        }
    } else {
        // Handle unsupported file type
        log::warn!(
            "Unsupported log file type at {}, returning empty content.",
            log_path.display()
        );
        Ok("".to_string()) // Or return an error if preferred
    }
}

/// Response struct for cape resizing operations
#[derive(Debug)]
pub struct CapeResizeResult {
    /// The image bytes (resized or original)
    pub image_bytes: Vec<u8>,
    /// Whether the image was resized
    pub was_resized: bool,
    /// Original dimensions if resized (None if already correct size)
    pub original_dimensions: Option<(u32, u32)>,
}

/// Resizes a cape image to 512x256 pixels if it doesn't already have that size.
/// Uses Nearest Neighbor filter for pixel-perfect scaling suitable for capes.
///
/// # Arguments
///
/// * `image_path` - The path to the image file to resize
///
/// # Returns
///
/// A `Result` containing a `CapeResizeResult` with image bytes and resize info, or an `AppError` if processing fails.
pub async fn resize_cape_to_512x256(image_path: &Path) -> Result<CapeResizeResult> {
    debug!("Attempting to load image for resizing: {}", image_path.display());

    // Load the image from file
    let img = image::open(image_path).map_err(|e| {
        debug!("Failed to open image {}: {}", image_path.display(), e);
        AppError::ImageProcessingError(format!("Failed to open image: {}", e))
    })?;

    let width = img.width();
    let height = img.height();
    debug!("Original image dimensions: {}x{}", width, height);

    // Check if already the correct size
    if width == 512 && height == 256 {
        debug!("Image already has correct dimensions 512x256, returning original");
        // Convert back to bytes without resizing
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .map_err(|e| {
                AppError::ImageProcessingError(format!("Failed to encode image to PNG: {}", e))
            })?;
        return Ok(CapeResizeResult {
            image_bytes: bytes,
            was_resized: false,
            original_dimensions: None,
        });
    }

    debug!("Resizing image from {}x{} to 512x256", width, height);

    // Resize the image to 512x256 using Nearest Neighbor for pixel-perfect scaling
    let resized_img = img.resize_exact(512, 256, FilterType::Nearest);

    debug!("Successfully resized image to 512x256");

    // Convert the resized image back to PNG bytes
    let mut bytes: Vec<u8> = Vec::new();
    resized_img
        .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|e| {
            AppError::ImageProcessingError(format!("Failed to encode resized image to PNG: {}", e))
        })?;

    debug!(
        "Successfully encoded resized image to PNG ({} bytes)",
        bytes.len()
    );

    Ok(CapeResizeResult {
        image_bytes: bytes,
        was_resized: true,
        original_dimensions: Some((width, height)),
    })
}
