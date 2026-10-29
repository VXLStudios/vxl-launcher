use crate::config::HTTP_CLIENT;
use crate::error::CommandError;
use futures::future::join_all;
use log::warn;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tauri::command;

const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MojangServiceStatus {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MojangServiceHealth {
    pub id: String,
    pub label: String,
    pub desc: String,
    pub status: MojangServiceStatus,
    #[serde(default)]
    pub latency: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojangStatusResponse {
    pub services: Vec<MojangServiceHealth>,
    pub checked_at: String,
}

struct ServiceProbe {
    id: &'static str,
    label: &'static str,
    desc: &'static str,
    url: &'static str,
}

const SERVICE_PROBES: &[ServiceProbe] = &[
    ServiceProbe {
        id: "microsoft",
        label: "Microsoft Auth",
        desc: "Microsoft login",
        url: "https://login.microsoftonline.com/consumers/v2.0/.well-known/openid-configuration",
    },
    ServiceProbe {
        id: "api",
        label: "Mojang API",
        desc: "Player & name lookup",
        url: "https://api.mojang.com/users/profiles/minecraft/Notch",
    },
    ServiceProbe {
        id: "session",
        label: "Session Server",
        desc: "Multiplayer authentication",
        url: "https://sessionserver.mojang.com/session/minecraft/profile/853c80ef3c3749fdaa49938b674adae6",
    },
    ServiceProbe {
        id: "skins",
        label: "Skin Server",
        desc: "Skins and capes",
        url: "http://textures.minecraft.net/texture/7fd9ba42a7c81eeea22f1524271ae85a8e045ce0af5a6ae16c6406ae917e68b5",
    },
    ServiceProbe {
        id: "voxel",
        label: "Voxel API",
        desc: "Voxel Studios API",
        url: "https://api.voxelstudios.co.uk/health",
    },
];

async fn probe_service(probe: &ServiceProbe) -> MojangServiceHealth {
    let started = Instant::now();
    let result = HTTP_CLIENT
        .get(probe.url)
        .timeout(PROBE_TIMEOUT)
        .send()
        .await;

    let latency = Some(started.elapsed().as_millis() as u64);
    let status = match &result {
        Ok(response) if !response.status().is_server_error() => MojangServiceStatus::Up,
        Ok(response) => {
            warn!(
                "Mojang status probe {} returned {}",
                probe.id,
                response.status()
            );
            MojangServiceStatus::Down
        }
        Err(error) => {
            warn!("Mojang status probe {} failed: {}", probe.id, error);
            MojangServiceStatus::Down
        }
    };

    MojangServiceHealth {
        id: probe.id.to_string(),
        label: probe.label.to_string(),
        desc: probe.desc.to_string(),
        status,
        latency,
    }
}

/// Check Microsoft login, Mojang, the skin host, and the Voxel API directly.
#[command]
pub async fn fetch_mojang_service_status_command() -> Result<MojangStatusResponse, CommandError> {
    let services = join_all(SERVICE_PROBES.iter().map(probe_service)).await;

    Ok(MojangStatusResponse {
        services,
        checked_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    })
}
