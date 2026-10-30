use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct NeoForgeMavenMetadata {
    #[serde(rename = "versioning")]
    pub versioning: Versioning,
}

#[derive(Debug, Deserialize)]
pub struct Versioning {
    #[serde(rename = "latest")]
    pub latest: String,
    #[serde(rename = "release")]
    pub release: String,
    #[serde(rename = "versions")]
    pub versions: Versions,
}

#[derive(Debug, Deserialize)]
pub struct Versions {
    #[serde(rename = "version")]
    pub versions: Vec<String>,
}

impl NeoForgeMavenMetadata {
    pub fn get_latest_version(&self) -> &str {
        &self.versioning.latest
    }

    pub fn get_release_version(&self) -> &str {
        &self.versioning.release
    }

    pub fn get_all_versions(&self) -> &[String] {
        &self.versioning.versions.versions
    }

    pub fn get_versions_for_minecraft(&self, minecraft_version: &str) -> Vec<String> {
        let mut versions: Vec<String> = self.versioning
            .versions
            .versions
            .iter()
            .filter(|v| {
                if let Some(parsed_mc_version) = Self::parse_neoforge_version_to_minecraft(v) {
                    parsed_mc_version == minecraft_version
                } else {
                    false
                }
            })
            .cloned()
            .collect();
        
        // Reverse to get newest first (Maven metadata is chronological, oldest to newest)
        versions.reverse();
        versions
    }

    pub fn get_latest_version_for_minecraft(&self, minecraft_version: &str) -> Option<String> {
        // After reverse, first element is the newest
        self.get_versions_for_minecraft(minecraft_version)
            .into_iter()
            .next()
    }

    pub fn parse_neoforge_version_to_minecraft(neoforge_version: &str) -> Option<String> {
        let version_core = neoforge_version.split('-').next().unwrap_or(neoforge_version);
        let parts: Vec<&str> = version_core.split('.').collect();

        if parts.len() < 2 {
            return None;
        }

        let p1: u32 = parts[0].parse().ok()?;

        if p1 == 0 {
            return Some(parts[1].to_string());
        }

        let p2: u32 = parts[1].parse().ok()?;

        // Minecraft 26.x snapshots use unprefixed game versions (26.3); NeoForge uses 26.3.0.x builds.
        if p1 >= 26 {
            Some(format!("{}.{}", p1, p2))
        } else if p2 > 0 {
            Some(format!("1.{}.{}", p1, p2))
        } else {
            Some(format!("1.{}", p1))
        }
    }

    pub fn print_parsed_versions(&self) {
        log::info!("NeoForge Version -> Parsed Minecraft Version:");
        for neoforge_version in &self.versioning.versions.versions {
            let parsed_mc_version = Self::parse_neoforge_version_to_minecraft(neoforge_version);
            log::info!(
                "  {} -> {}",
                neoforge_version,
                parsed_mc_version.as_deref().unwrap_or("Parse Failed")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NeoForgeMavenMetadata;

    #[test]
    fn maps_legacy_neoforge_versions_to_1_x_y() {
        assert_eq!(
            NeoForgeMavenMetadata::parse_neoforge_version_to_minecraft("21.11.38-beta"),
            Some("1.21.11".to_string())
        );
    }

    #[test]
    fn maps_snapshot_neoforge_versions_to_unprefixed_mc() {
        assert_eq!(
            NeoForgeMavenMetadata::parse_neoforge_version_to_minecraft("26.3.0.99"),
            Some("26.3".to_string())
        );
        assert_eq!(
            NeoForgeMavenMetadata::parse_neoforge_version_to_minecraft("26.3.0.14-beta"),
            Some("26.3".to_string())
        );
    }

    #[test]
    fn filters_maven_versions_for_minecraft_26_3() {
        let metadata: NeoForgeMavenMetadata = quick_xml::de::from_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <metadata>
                  <versioning>
                    <latest>26.3.0.99</latest>
                    <release>26.3.0.99</release>
                    <versions>
                      <version>21.11.0-beta</version>
                      <version>26.3.0.14-beta</version>
                      <version>26.3.0.99</version>
                    </versions>
                  </versioning>
                </metadata>"#,
        )
        .expect("parse metadata");

        let for_26_3 = metadata.get_versions_for_minecraft("26.3");
        assert_eq!(for_26_3, vec!["26.3.0.99", "26.3.0.14-beta"]);
    }
}
