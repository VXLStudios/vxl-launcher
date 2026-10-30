use crate::error::Result;
use crate::minecraft::api::loader_maven_metadata::{
    get_cached_maven_metadata, LoaderMavenApiConfig,
};
use crate::minecraft::dto::neo_forge_maven_meta::NeoForgeMavenMetadata;

const NEO_FORGE_MAVEN_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

const NEO_FORGE_MAVEN_CONFIG: LoaderMavenApiConfig = LoaderMavenApiConfig {
    metadata_url: NEO_FORGE_MAVEN_METADATA_URL,
    cache_subdir: "neoforge_cache",
    cache_filename: "neoforge_metadata.xml",
    loader_label: "NeoForge",
};

pub struct NeoForgeApi;

impl NeoForgeApi {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_all_versions(&self) -> Result<NeoForgeMavenMetadata> {
        get_cached_maven_metadata(&NEO_FORGE_MAVEN_CONFIG).await
    }
}
