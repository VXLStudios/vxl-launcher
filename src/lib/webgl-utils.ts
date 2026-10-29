/** Probe whether WebGL is available (software or hardware). */
export function isWebGLAvailable(): boolean {
  if (typeof document === "undefined") return false;
  try {
    const canvas = document.createElement("canvas");
    return !!(
      canvas.getContext("webgl2") ||
      canvas.getContext("webgl") ||
      canvas.getContext("experimental-webgl")
    );
  } catch {
    return false;
  }
}

/** 2D fallback when WebGL snapshot rendering is unavailable. */
export function renderFlatSkin(textureUrl: string): string {
  return textureUrl;
}
