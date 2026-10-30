/// NeoForge artifacts live under the Maven `releases` repository, not the repo root.
pub const NEOFORGE_MAVEN_RELEASES_BASE: &str = "https://maven.neoforged.net/releases";

pub fn neoforge_maven_download_url(relative_maven_path: &str) -> String {
    format!("{}/{}", NEOFORGE_MAVEN_RELEASES_BASE, relative_maven_path)
}
