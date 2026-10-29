export type MojangServiceStatus = "up" | "down";

export interface MojangServiceHealth {
  id: string;
  label: string;
  desc: string;
  status: MojangServiceStatus;
  latency?: number | null;
}

export interface MojangStatusResponse {
  services: MojangServiceHealth[];
  checkedAt: string;
}

export const MONITORED_MOJANG_SERVICES = [
  { id: "microsoft", label: "Microsoft Auth" },
  { id: "api", label: "Mojang API" },
  { id: "session", label: "Session Server" },
  { id: "skins", label: "Skin Server" },
  { id: "voxel", label: "Voxel API" },
] as const;

export type MonitoredMojangServiceId = (typeof MONITORED_MOJANG_SERVICES)[number]["id"];

export type MonitoredServiceCheckStatus = "loading" | "up" | "down" | "unknown";

export interface MonitoredMojangServiceState {
  id: MonitoredMojangServiceId;
  label: string;
  status: MonitoredServiceCheckStatus;
}
