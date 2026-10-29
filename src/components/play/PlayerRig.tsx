"use client";

import React, { useEffect, useMemo, useRef, useState } from "react";
import { SkinView } from "vxl-skin-renderer/react";
import type { OutlineProfile } from "vxl-skin-renderer/core";
import type { Attachable } from "vxl-skin-renderer/core";

import { SkinViewer } from "../launcher/SkinViewer";
import { useActiveSkinTexture } from "../../hooks/useActiveSkinTexture";
import { useEquippedCosmetics } from "../../hooks/useEquippedCosmetics";
import { useIdleEmote } from "../../hooks/useIdleEmote";
import { useSkinPreview } from "../../hooks/useSkinPreview";
import { useWindowFocus } from "../../hooks/useWindowFocus";
import { cosmeticToAttachable } from "../../lib/cosmetics/to-attachable";
import { useDeferredRendererSkin } from "../../hooks/useDeferredRendererSkin";
import { useMinecraftAuthStore } from "../../store/minecraft-auth-store";
import { useQualitySettingsStore } from "../../store/quality-settings-store";
import { useThemeStore } from "../../store/useThemeStore";
import { useVanillaCapeStore } from "../../store/useVanillaCapeStore";
import { useStreamerModeStore } from "../../store/streamer-mode-store";
import { isWebGLAvailable } from "../../lib/webgl-utils";

const NO_ACCOUNT_SKIN_URL = "/skins/steve.png";
const RIG_SHADOW = "drop-shadow(5px 10px 5px rgba(0,0,0,0.75))";
const NO_ATTACHABLES: Attachable[] = [];

export const PLAYER_RIG_WIDTH = 225;
export const PLAYER_RIG_HEIGHT = 450;

const CANVAS_WIDTH = 1040;
const CANVAS_HEIGHT = 860;

const canvasStyle: React.CSSProperties = {
  position: "absolute",
  left: "50%",
  top: "50%",
  transform: "translate(-50%, -50%)",
  width: `${CANVAS_WIDTH}px`,
  height: `${CANVAS_HEIGHT}px`,
  maxWidth: "none",
  pointerEvents: "none",
  filter: RIG_SHADOW,
};

interface PlayerRigProps {
  playerName: string | null | undefined;
  outline?: Partial<OutlineProfile>;
  className?: string;
}

function useMinLoading(active: boolean, minMs: number): boolean {
  const [done, setDone] = useState(false);
  const startRef = useRef(Date.now());
  useEffect(() => {
    if (active) return;
    const remaining = Math.max(0, minMs - (Date.now() - startRef.current));
    const id = setTimeout(() => setDone(true), remaining);
    return () => clearTimeout(id);
  }, [active, minMs]);
  return active || !done;
}

export function PlayerRig({ playerName, outline, className }: PlayerRigProps) {
  const accentColor = useThemeStore((state) => state.accentColor);
  const activeAccount = useMinecraftAuthStore((state) => state.activeAccount);
  const isLoadingAccounts = useMinecraftAuthStore((state) => state.isLoading);
  const fetchOwnedCapes = useVanillaCapeStore((state) => state.fetchOwnedCapes);
  const { textureUrl, variant, loading: skinLoading } = useActiveSkinTexture();
  const { cosmetics: equippedCosmetics } = useEquippedCosmetics(activeAccount?.id);
  const idleEmote = useIdleEmote();
  const skinRenderer3dSetting = useQualitySettingsStore((s) => s.skinRenderer3d);
  const skinRenderer3d = skinRenderer3dSetting && isWebGLAvailable();
  const isWindowFocused = useWindowFocus();
  const streamerMode = useStreamerModeStore((state) => state.enabled);
  const hasAccount = !!activeAccount;
  const loading = useMinLoading(hasAccount && skinLoading, 450);

  useEffect(() => {
    if (!activeAccount?.id) return;
    void fetchOwnedCapes({ accountId: activeAccount.id });
  }, [activeAccount?.id, fetchOwnedCapes]);

  const attachables = useMemo(
    () => equippedCosmetics.map(cosmeticToAttachable),
    [equippedCosmetics],
  );

  const ownNametag = useMemo(() => {
    const resolvedName =
      playerName ||
      activeAccount?.minecraft_username ||
      activeAccount?.username;
    if (!resolvedName) return null;
    return {
      text: resolvedName.toString(),
      iconUrl: null,
      iconPlus: false,
    };
  }, [playerName, activeAccount?.minecraft_username, activeAccount?.username]);

  const rigSkinUrl = hasAccount ? textureUrl : NO_ACCOUNT_SKIN_URL;
  const rigVariant = hasAccount ? variant : "slim";
  const rigAttachables = hasAccount ? attachables : NO_ATTACHABLES;
  const attachablesSyncKey = useMemo(
    () =>
      rigAttachables
        .map(
          (entry) =>
            `${entry.meta.id ?? ""}:${entry.assets.texture}:${entry.assets.geo ?? ""}`,
        )
        .join("|"),
    [rigAttachables],
  );
  const deferredSkin = useDeferredRendererSkin(
    rigSkinUrl,
    `${attachablesSyncKey}`,
  );
  const rigNametag = useMemo(() => {
    if (streamerMode) return null;
    // Avoid flashing "No account" while auth is still loading — concurrent
    // async nametag rebuilds can leave duplicate labels on screen.
    if (isLoadingAccounts) return null;
    if (!hasAccount) return { text: "No account", iconUrl: null, iconPlus: false };
    return ownNametag;
  }, [streamerMode, hasAccount, isLoadingAccounts, ownNametag]);

  const skinViewKey = activeAccount?.id ?? "no-account";

  const renderProfile = useMemo(
    () => ({
      renderer: {
        fpsCap: isWindowFocused ? 60 : 1,
        maxDpr: 1.5,
      },
      loading: {
        skeletonColor: accentColor.value,
      },
    }),
    [accentColor.value, isWindowFocused],
  );

  const { url: stillUrl } = useSkinPreview(!skinRenderer3d && !loading, {
    textureUrl: rigSkinUrl,
    variant: rigVariant,
    attachables: rigAttachables,
    nametag: rigNametag,
    fit: false,
    crop: false,
    width: CANVAS_WIDTH,
    height: CANVAS_HEIGHT,
    dpr: 1.5,
    fallbackUrl: null,
  });

  return (
    <div
      className={className}
      style={{
        width: `${PLAYER_RIG_WIDTH}px`,
        height: `${PLAYER_RIG_HEIGHT}px`,
      }}
    >
      {skinRenderer3d ? (
        <SkinView
          key={skinViewKey}
          profile={renderProfile}
          skin={deferredSkin}
          model="auto"
          attachables={rigAttachables}
          emote={idleEmote.urls}
          onEmoteEnd={!idleEmote.loop ? idleEmote.onEnd : undefined}
          dragAxis="yaw"
          outline={outline}
          nametag={rigNametag}
          loading={loading}
          className="bg-transparent"
          style={canvasStyle}
        />
      ) : stillUrl ? (
        <SkinViewer
          skinUrl={stillUrl}
          playerName={playerName?.toString()}
          width={CANVAS_WIDTH}
          height={CANVAS_HEIGHT}
          className="bg-transparent"
          style={canvasStyle}
        />
      ) : null}
    </div>
  );
}
