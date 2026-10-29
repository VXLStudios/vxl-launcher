import { invoke } from '@tauri-apps/api/core';

const VXL_API_BASE = 'https://api.voxelstudios.co.uk';

function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') return false;
  const w = window as Window & {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
    __TAURI_METADATA__?: unknown;
  };
  return Boolean(w.__TAURI__ || w.__TAURI_INTERNALS__ || w.__TAURI_METADATA__);
}

function normalizeApiPath(path: string): string {
  if (path.startsWith('http://') || path.startsWith('https://')) {
    const url = new URL(path);
    return `${url.pathname}${url.search}`;
  }
  return path.startsWith('/') ? path : `/${path}`;
}

/** Fetch JSON from api.voxelstudios.co.uk (uses Tauri HTTP in the desktop app to avoid CORS). */
export async function fetchVxlApiJson<T>(path: string, init?: RequestInit): Promise<T> {
  const apiPath = normalizeApiPath(path);

  if (isTauriRuntime()) {
    return invoke<T>('fetch_vxl_api_json', { path: apiPath });
  }

  const response = await fetch(`${VXL_API_BASE}${apiPath}`, {
    ...init,
    signal: init?.signal ?? AbortSignal.timeout(15_000),
  });

  if (!response.ok) {
    throw new Error(`VXL API request failed: ${response.status} ${response.statusText}`);
  }

  return response.json() as Promise<T>;
}

/** CurseForge catalog entry from api.voxelstudios.co.uk/v1/curseforge */
export interface VxlCurseForgeCatalogMod {
  id: number;
  name: string;
  slug: string;
  summary: string;
  description?: string;
  downloadCount: number;
  iconUrl?: string | null;
  websiteUrl: string;
  game: string;
  platform: string;
  classId: number;
  projectType: string;
  contentType: string;
  gameVersions?: string[];
  loaders?: string[];
}

export interface VxlCurseForgeCatalogResponse {
  mods: VxlCurseForgeCatalogMod[];
  count: number;
  cache?: {
    fetchedAt?: string;
    expiresAt?: string;
  };
}

export async function fetchVxlCurseForgeCatalog(): Promise<VxlCurseForgeCatalogResponse> {
  return fetchVxlApiJson<VxlCurseForgeCatalogResponse>('/v1/curseforge');
}

/** Full CurseForge catalog for a game (Studios API cache). */
export async function fetchVxlCurseForgeCatalogMods(
  game: 'minecraft' | 'hytale' = 'minecraft',
): Promise<VxlCurseForgeCatalogMod[]> {
  const data = await fetchVxlCurseForgeCatalog();
  return data.mods.filter((m) => m.game === game);
}

/** Mod ids for a game from the unified catalog. */
export async function fetchVxlCurseForgeProjectIds(
  game: 'minecraft' | 'hytale' = 'minecraft',
): Promise<number[]> {
  const mods = await fetchVxlCurseForgeCatalogMods(game);
  return mods.map((m) => m.id);
}

/** Cached VXL Studios Modrinth org catalog from the Studios API. */
export interface VxlModrinthCatalogEntry {
  project: Record<string, unknown>;
  versions: unknown[];
}

export interface VxlModrinthCatalogResponse {
  projects: VxlModrinthCatalogEntry[];
  count: number;
  organizationId: string;
}

export async function fetchVxlModrinthCatalog(): Promise<VxlModrinthCatalogResponse> {
  return fetchVxlApiJson<VxlModrinthCatalogResponse>('/v1/modrinth');
}
