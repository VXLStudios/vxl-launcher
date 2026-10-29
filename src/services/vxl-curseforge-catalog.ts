import type { VxlCurseForgeCatalogMod } from './vxl-api-client';

export function isVxlCurseForgeModpack(mod: VxlCurseForgeCatalogMod): boolean {
  return mod.contentType === 'modpack' || mod.classId === 4471;
}

export function isVxlCurseForgeMod(mod: VxlCurseForgeCatalogMod): boolean {
  return mod.contentType === 'mod' && mod.classId !== 4471 && mod.classId !== 12;
}

export function isVxlCurseForgeResourcePack(mod: VxlCurseForgeCatalogMod): boolean {
  return mod.classId === 12 || mod.contentType === 'resourcepack';
}

export function vxlCurseForgeProjectType(
  mod: VxlCurseForgeCatalogMod,
): 'mod' | 'modpack' | 'resourcepack' {
  if (isVxlCurseForgeModpack(mod)) return 'modpack';
  if (isVxlCurseForgeResourcePack(mod)) return 'resourcepack';
  return 'mod';
}

export function latestVxlCurseForgeGameVersion(mod: VxlCurseForgeCatalogMod): string | null {
  return mod.gameVersions?.find((v) => v.trim()) ?? null;
}
