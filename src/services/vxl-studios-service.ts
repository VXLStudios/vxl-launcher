import type { ModrinthProject } from "../types/modrinth";
import type { CurseForgeMod } from "../types/curseforge";
import {
  fetchVxlCurseForgeCatalogMods,
  fetchVxlModrinthCatalog,
  type VxlCurseForgeCatalogMod,
} from "./vxl-api-client";

const MODRINTH_API_V2_BASE = "https://api.modrinth.com/v2";
const CURSEFORGE_API_BASE = "https://api.curseforge.com/v1";

export class VXLStudiosService {
  /** VXL Studios Modrinth projects (cached on api.voxelstudios.co.uk). */
  static async getVXLStudiosModrinthProjects(): Promise<ModrinthProject[]> {
    try {
      const catalog = await fetchVxlModrinthCatalog();
      console.log(
        "[VXLStudiosService] Modrinth catalog from Studios API:",
        catalog.count,
        "projects",
      );

      return catalog.projects.map((entry) => {
        const raw = entry.project as Record<string, unknown>;
        const projectTypes = raw.project_types as string[] | undefined;
        const versions = (entry.versions ?? []) as ModrinthProject["versions"];
        return {
          ...(raw as unknown as ModrinthProject),
          project_type: (projectTypes?.[0] ??
            raw.project_type ??
            "mod") as ModrinthProject["project_type"],
          title: String(raw.name ?? raw.title ?? ""),
          description: String(raw.summary ?? raw.description ?? ""),
          versions: versions ?? [],
        };
      });
    } catch (error) {
      console.error("[VXLStudiosService] Error fetching Modrinth projects:", error);
      throw error;
    }
  }

  /** VXL Studios CurseForge catalog (cached on api.voxelstudios.co.uk). */
  static async getVXLStudiosCurseForgeCatalog(
    game: "minecraft" | "hytale" = "minecraft",
  ): Promise<VxlCurseForgeCatalogMod[]> {
    try {
      const mods = await fetchVxlCurseForgeCatalogMods(game);
      console.log(
        "[VXLStudiosService] CurseForge catalog from Studios API:",
        mods.length,
        "projects for",
        game,
      );
      return mods;
    } catch (error) {
      console.error("[VXLStudiosService] Error fetching CurseForge catalog:", error);
      throw error;
    }
  }

  /**
   * Get a specific Modrinth project by ID
   */
  static async getModrinthProjectById(projectId: string): Promise<ModrinthProject> {
    try {
      const response = await fetch(
        `${MODRINTH_API_V2_BASE}/project/${projectId}`
      );

      if (!response.ok) {
        throw new Error(`Failed to fetch project: ${response.statusText}`);
      }

      const project = await response.json();
      return project as ModrinthProject;
    } catch (error) {
      console.error("[VXLStudiosService] Error fetching Modrinth project:", error);
      throw error;
    }
  }

  /**
   * Get a specific CurseForge project by ID
   */
  static async getCurseForgeProjectById(projectId: number): Promise<CurseForgeMod> {
    try {
      const response = await fetch(
        `${CURSEFORGE_API_BASE}/mods/${projectId}`
      );

      if (!response.ok) {
        throw new Error(`Failed to fetch CurseForge project: ${response.statusText}`);
      }

      const data = await response.json();
      return data.data as CurseForgeMod;
    } catch (error) {
      console.error("[VXLStudiosService] Error fetching CurseForge project:", error);
      throw error;
    }
  }
}
