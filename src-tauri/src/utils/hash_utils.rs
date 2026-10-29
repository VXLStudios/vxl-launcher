use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::io;
use std::path::{Path, PathBuf};

const HASH_READ_BUFFER: usize = 64 * 1024;

/// Asynchronously calculates the SHA1 hash of a file (blocking pool for large reads).
pub async fn calculate_sha1(path: &PathBuf) -> Result<String, io::Error> {
    hash_file_blocking::<Sha1>(path.clone()).await
}

async fn hash_file_blocking<D>(path: PathBuf) -> Result<String, io::Error>
where
    D: Digest + Send + 'static,
{
    tokio::task::spawn_blocking(move || hash_file_sync::<D>(&path))
        .await
        .map_err(|e| io::Error::other(format!("hash task failed: {e}")))?
}

fn hash_file_sync<D: Digest>(path: &Path) -> Result<String, io::Error> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = D::new();
    let mut buffer = vec![0u8; HASH_READ_BUFFER];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Calculates the SHA1 hash of a byte slice.
pub fn calculate_sha1_from_bytes(bytes: &[u8]) -> String {
    hex_lower(&Sha1::digest(bytes))
}

/// Alias for calculate_sha1 to match the naming convention used in download_utils
pub async fn calculate_sha1_from_file<P: AsRef<Path>>(path: P) -> Result<String, io::Error> {
    calculate_sha1(&path.as_ref().to_path_buf()).await
}

/// Asynchronously calculates the SHA256 hash of a file.
pub async fn calculate_sha256_from_file<P: AsRef<Path>>(path: P) -> Result<String, io::Error> {
    hash_file_blocking::<Sha256>(path.as_ref().to_path_buf()).await
}

/// Calculates the SHA256 hash of a byte slice.
pub fn calculate_sha256_from_bytes(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}
