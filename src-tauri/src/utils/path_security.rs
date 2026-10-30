use crate::config::{standard_meta_dir, ProjectDirsExt, LAUNCHER_DIRECTORY};
use crate::error::{AppError, Result};
use crate::state::profile_state::default_profile_path;
use std::path::{Component, Path, PathBuf};

/// Launcher-owned temp directory (not the system-wide temp folder).
pub fn launcher_temp_dir() -> PathBuf {
    LAUNCHER_DIRECTORY.data_dir().join("temp")
}

/// Returns filesystem roots the launcher is allowed to read/write via IPC path commands.
pub fn allowed_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        LAUNCHER_DIRECTORY.root_dir(),
        LAUNCHER_DIRECTORY.data_dir().to_path_buf(),
        standard_meta_dir(),
        LAUNCHER_DIRECTORY.meta_dir(),
        LAUNCHER_DIRECTORY.file_sync_hub_dir(),
        default_profile_path(),
        launcher_temp_dir(),
    ];

    if let Ok(guard) = crate::config::CUSTOM_GAME_DIR_CACHE.read() {
        if let Some(Some(custom_dir)) = guard.as_ref() {
            if let Ok(canonical) = std::fs::canonicalize(custom_dir) {
                if is_safe_custom_game_directory(&canonical) {
                    roots.push(canonical.clone());
                    roots.push(canonical.join("profiles"));
                }
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

/// Joins a relative path (may use `/` or `\`) onto `base`, validating each segment.
pub fn join_relative_path(base: impl AsRef<Path>, relative: &str) -> Result<PathBuf> {
    let normalized = relative.replace('\\', "/");
    let mut full_path = base.as_ref().to_path_buf();
    for segment in normalized.split('/').filter(|s| !s.is_empty()) {
        let segment = validate_relative_segment(segment)?;
        full_path = full_path.join(segment);
    }
    Ok(full_path)
}

/// Rejects relative path segments that could escape a base directory.
pub fn validate_relative_segment(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(AppError::Other("Path segment cannot be empty".to_string()));
    }

    if name.contains('\0') {
        return Err(AppError::Other("Path segment contains invalid characters".to_string()));
    }

    let path = Path::new(name);
    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::Other(format!(
                    "Invalid path segment: {}",
                    name
                )));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    if name.contains('/') || name.contains('\\') {
        return Err(AppError::Other(format!(
            "Path segment must not contain separators: {}",
            name
        )));
    }

    Ok(name.to_string())
}

fn normalize_for_comparison(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).map_err(AppError::Io);
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::Other("Path has no file name".to_string()))?;

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let canonical_parent = match parent {
        Some(parent_path) if parent_path.exists() => std::fs::canonicalize(parent_path)?,
        Some(_) => {
            return Err(AppError::Other(format!(
                "Parent path does not exist: {}",
                path.display()
            )));
        }
        None => {
            return Err(AppError::Other(format!(
                "Cannot resolve relative path: {}",
                path.display()
            )));
        }
    };

    Ok(canonical_parent.join(file_name))
}

fn is_under_root(candidate: &Path, root: &Path) -> bool {
    let Ok(canonical_root) = std::fs::canonicalize(root) else {
        return false;
    };

    candidate.starts_with(&canonical_root)
        || candidate
            .strip_prefix("\\\\?\\")
            .map(|stripped| stripped.starts_with(&canonical_root))
            .unwrap_or(false)
}

/// Validates that `path` resolves under one of the allowed launcher roots.
pub fn validate_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let normalized = normalize_for_comparison(path)?;

    let allowed = allowed_roots();
    if allowed.iter().any(|root| is_under_root(&normalized, root)) {
        return Ok(normalized);
    }

    Err(AppError::Other(format!(
        "Path is outside allowed launcher directories: {}",
        path.display()
    )))
}

/// Validates that `path` resolves under `base` (typically a profile instance directory).
pub fn validate_path_under_base(path: impl AsRef<Path>, base: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let base = base.as_ref();
    let normalized = normalize_for_comparison(path)?;

    let canonical_base = if base.exists() {
        std::fs::canonicalize(base)?
    } else {
        return Err(AppError::Other(format!(
            "Base path does not exist: {}",
            base.display()
        )));
    };

    if is_under_root(&normalized, &canonical_base) {
        Ok(normalized)
    } else {
        Err(AppError::Other(format!(
            "Path is outside allowed directory: {}",
            path.display()
        )))
    }
}

/// Joins a validated relative segment onto `base`.
pub fn join_validated_segment(base: impl AsRef<Path>, segment: &str) -> Result<PathBuf> {
    let segment = validate_relative_segment(segment)?;
    let joined = base.as_ref().join(segment);
    validate_path_under_base(&joined, base)
}

/// Rejects filesystem roots and other paths that would make IPC path checks too broad.
pub fn is_safe_custom_game_directory(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }

    #[cfg(windows)]
    {
        let components: Vec<_> = path.components().collect();
        if components.len() <= 1 {
            return false;
        }
    }

    #[cfg(unix)]
    {
        if path.parent().is_none() || path == Path::new("/") {
            return false;
        }
    }

    true
}

/// Validates a user-supplied custom game directory before it is stored in config.
pub fn validate_custom_game_directory(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(AppError::Other(
            "Custom game directory must be an absolute path".to_string(),
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(AppError::Io)?;
    if !is_safe_custom_game_directory(&canonical) {
        return Err(AppError::Other(
            "Custom game directory cannot be a filesystem root".to_string(),
        ));
    }
    Ok(canonical)
}

/// Keeps only the final path segment and validates it (for download file names from IPC).
pub fn resolve_single_file_name(file_name: &str) -> Result<String> {
    let path = Path::new(file_name);
    let base = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Other("Invalid file name".to_string()))?;
    validate_relative_segment(base)
}

/// Joins a validated file name onto `target_dir` and ensures the result stays under it.
pub fn join_content_file_path(target_dir: impl AsRef<Path>, file_name: &str) -> Result<PathBuf> {
    let segment = resolve_single_file_name(file_name)?;
    let joined = target_dir.as_ref().join(&segment);
    validate_path_under_base(&joined, target_dir)
}

/// Joins a zip/tar entry onto `base` after zip-slip validation.
pub fn join_archive_entry(base: impl AsRef<Path>, entry_name: &str) -> Result<PathBuf> {
    validate_zip_entry_path(entry_name)?;
    let joined = base.as_ref().join(entry_name);
    validate_path_under_base(&joined, base)
}

/// Validates zip archive entry paths to prevent zip-slip attacks.
pub fn validate_zip_entry_path(entry_name: &str) -> Result<()> {
    if entry_name.contains('\0') {
        return Err(AppError::Other("Invalid zip entry name".to_string()));
    }

    let path = Path::new(entry_name);
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)) {
            return Err(AppError::Other(format!(
                "Unsafe zip entry path: {}",
                entry_name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn rejects_parent_dir_segments() {
        assert!(validate_relative_segment("..").is_err());
        assert!(validate_relative_segment("../secret").is_err());
        assert!(validate_relative_segment("foo/../bar").is_err());
    }

    #[test]
    fn accepts_simple_segment_names() {
        assert_eq!(validate_relative_segment("options.txt").unwrap(), "options.txt");
        assert_eq!(validate_relative_segment("mods").unwrap(), "mods");
    }

    #[test]
    fn rejects_separator_in_segment() {
        assert!(validate_relative_segment("mods/cheats.jar").is_err());
        assert!(validate_relative_segment("mods\\cheats.jar").is_err());
    }

    #[test]
    fn validate_path_allows_under_temp_root() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, b"ok").unwrap();

        // temp_dir is in allowed_roots
        let result = validate_path(&file);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_path_rejects_outside_roots() {
        if cfg!(windows) {
            let outside = PathBuf::from("C:\\Windows\\System32\\drivers\\etc\\hosts");
            if outside.exists() {
                assert!(validate_path(&outside).is_err());
            }
        } else if Path::new("/etc/passwd").exists() {
            assert!(validate_path("/etc/passwd").is_err());
        }
    }

    #[test]
    fn join_validated_segment_stays_under_base() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("profile");
        fs::create_dir_all(&base).unwrap();

        let joined = join_validated_segment(&base, "options.txt").unwrap();
        let canonical_base = std::fs::canonicalize(&base).unwrap();
        assert!(joined.starts_with(&canonical_base));
        assert!(validate_relative_segment("../escape").is_err());
    }

    #[test]
    fn join_relative_path_accepts_backslash_segments() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("profile");
        let nested = base.join("vxl");
        fs::create_dir_all(&nested).unwrap();
        let icon = nested.join("icon.webp");
        fs::write(&icon, b"ok").unwrap();

        let joined = join_relative_path(&base, "vxl\\icon.webp").unwrap();
        assert_eq!(joined, icon);
    }
}
