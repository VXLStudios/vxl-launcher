use crate::error::Result;
use crate::minecraft::api::loader_maven_metadata::{
    get_cached_maven_metadata, LoaderMavenApiConfig,
};
use crate::minecraft::dto::forge_maven_meta::ForgeMavenMetadata;

const FORGE_MAVEN_METADATA_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";

const FORGE_MAVEN_CONFIG: LoaderMavenApiConfig = LoaderMavenApiConfig {
    metadata_url: FORGE_MAVEN_METADATA_URL,
    cache_subdir: "forge_cache",
    cache_filename: "forge_metadata.xml",
    loader_label: "Forge",
};

pub struct ForgeApi;

impl ForgeApi {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_all_versions(&self) -> Result<ForgeMavenMetadata> {
        get_cached_maven_metadata(&FORGE_MAVEN_CONFIG).await
    }
}
