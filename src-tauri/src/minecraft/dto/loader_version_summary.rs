use crate::minecraft::dto::fabric_meta::FabricVersionInfo;
use crate::minecraft::dto::quilt_meta::QuiltVersionInfo;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LoaderVersionSummary {
    pub loader: LoaderVersionSummaryLoader,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoaderVersionSummaryLoader {
    pub version: String,
    pub stable: bool,
    pub build: i32,
}

impl From<FabricVersionInfo> for LoaderVersionSummary {
    fn from(value: FabricVersionInfo) -> Self {
        Self {
            loader: LoaderVersionSummaryLoader {
                version: value.loader.version,
                stable: value.loader.stable,
                build: value.loader.build,
            },
        }
    }
}

impl From<QuiltVersionInfo> for LoaderVersionSummary {
    fn from(value: QuiltVersionInfo) -> Self {
        Self {
            loader: LoaderVersionSummaryLoader {
                version: value.loader.version,
                stable: value.loader.stable,
                build: value.loader.build,
            },
        }
    }
}
