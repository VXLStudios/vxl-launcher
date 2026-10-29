"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "@iconify/react";
import { invoke } from "@tauri-apps/api/core";
import { useLocation } from "react-router-dom";
import { cn } from "../../lib/utils";
import { useThemeStore } from "../../store/useThemeStore";
import { Card, IconButton, LoadingState } from "../ui-v2";
import { listItemAccentHover } from "../ui-v2/tokens";
import { useStreamerModeStore } from "../../store/streamer-mode-store";

interface ServerInfo {
  name?: string;
  address?: string;
  icon_base64?: string;
  profileId?: string;
  profileName?: string;
  gameVersion?: string;
  loader?: string;
}

interface ServerPingInfo {
  description?: string;
  favicon_base64?: string;
  motd_plain?: string;
  version_name?: string;
  players_online?: number;
  players_max?: number;
  ping?: number;
}

function parseMotdFromApi(description: unknown): string {
  if (!description) return "";

  const extractText = (obj: unknown): string => {
    if (!obj || typeof obj !== "object") return "";
    const record = obj as { text?: string; extra?: unknown[] };
    let text = record.text || "";
    if (Array.isArray(record.extra)) {
      record.extra.forEach((extra) => {
        text += extractText(extra);
      });
    }
    return text;
  };

  return extractText(description);
}

function extractMotd(data: Record<string, unknown>): string | null {
  if (data.description) {
    const rawMotd =
      typeof data.description === "string"
        ? data.description
        : parseMotdFromApi(data.description);
    if (rawMotd.trim()) return rawMotd;
  }

  const motd = data.motd as
    | string
    | { raw?: string; html?: string[] }
    | undefined;

  if (typeof motd === "string" && motd.trim()) return motd;
  if (motd && typeof motd === "object") {
    if (motd.raw?.trim()) return motd.raw;
    if (Array.isArray(motd.html)) return motd.html.join(" ");
  }

  return null;
}

function stripMotdFormatting(text: string): string {
  return text
    .replace(/§[0-9a-fk-or]/gi, "")
    .replace(/§x(§[0-9a-f]){6}/gi, "")
    .replace(/&?#([0-9a-fA-F]{6})/g, "")
    .replace(/<\/?[^>]+>/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function hasValidPlayerCount(details?: ServerPingInfo): details is ServerPingInfo & {
  players_online: number;
  players_max: number;
} {
  return (
    typeof details?.players_online === "number" &&
    typeof details?.players_max === "number" &&
    details.players_max > 0
  );
}

interface ServerSectionProps {
  className?: string;
}

function ServerListItem({
  server,
  details,
  isConnecting,
  onConnect,
}: {
  server: ServerInfo;
  details?: ServerPingInfo;
  isConnecting: boolean;
  onConnect: (server: ServerInfo) => void;
}) {
  const streamerMode = useStreamerModeStore((state) => state.enabled);
  const faviconSrc = details?.favicon_base64
    ? `data:image/png;base64,${details.favicon_base64}`
    : server.icon_base64
      ? `data:image/png;base64,${server.icon_base64}`
      : null;

  const subtitle = streamerMode
    ? details?.motd_plain || null
    : details?.motd_plain || server.address || null;

  const displayName =
    server.name ||
    (streamerMode ? "Server" : server.address) ||
    "Unnamed Server";

  return (
    <Card
      className={cn(
        "group flex items-center gap-3 overflow-hidden bg-[var(--surface-raised)] p-3",
        listItemAccentHover,
      )}
    >
      <div className="flex h-11 w-11 flex-shrink-0 items-center justify-center overflow-hidden rounded-xl border border-[var(--surface-border)] bg-[var(--surface-base)]">
        {faviconSrc ? (
          <img
            src={faviconSrc}
            alt=""
            className="h-full w-full object-cover"
            style={{ imageRendering: "pixelated" }}
          />
        ) : (
          <Icon icon="streamline-cyber:server" className="h-5 w-5 text-[var(--text-muted)]" />
        )}
      </div>

      <div className="min-w-0 flex-1">
        <h3 className="truncate text-sm font-semibold text-white">
          {displayName}
        </h3>

        <div className="mt-0.5 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-[var(--text-secondary)]">
          {server.profileName && (
            <span className="truncate">{server.profileName}</span>
          )}
          {hasValidPlayerCount(details) && (
            <>
              {server.profileName && (
                <span className="h-3 w-px bg-[var(--surface-border-strong)]" aria-hidden />
              )}
              <span className="inline-flex flex-shrink-0 items-center gap-1">
                <Icon icon="solar:users-group-two-rounded-bold" className="h-3 w-3" />
                {details.players_online.toLocaleString()}/
                {details.players_max.toLocaleString()}
              </span>
            </>
          )}
        </div>

        {subtitle && (
          <p className="mt-1 truncate text-xs text-[var(--text-muted)]">{subtitle}</p>
        )}
      </div>

      <IconButton
        size="sm"
        title={server.profileName ? `Connect with ${server.profileName}` : "Connect"}
        aria-label={server.profileName ? `Connect with ${server.profileName}` : "Connect"}
        disabled={isConnecting || !server.profileId}
        onClick={() => onConnect(server)}
        className="flex-shrink-0 opacity-70 transition-opacity group-hover:opacity-100"
      >
        <Icon
          icon={isConnecting ? "solar:refresh-bold" : "solar:play-bold"}
          className={cn("h-4 w-4", isConnecting && "animate-spin-slow")}
        />
      </IconButton>
    </Card>
  );
}

const COLLAPSED_SERVER_PANEL_WIDTH = 56;

export function ServerSection({ className }: ServerSectionProps) {
  const serverRef = useRef<HTMLDivElement>(null);
  const [isResizing, setIsResizing] = useState(false);
  const [startX, setStartX] = useState(0);
  const [startWidth, setStartWidth] = useState(0);
  const serverSectionWidth = useThemeStore((state) => state.newsSectionWidth);
  const setServerSectionWidth = useThemeStore((state) => state.setNewsSectionWidth);
  const isCollapsed = useThemeStore((state) => state.serverSectionCollapsed);
  const setServerSectionCollapsed = useThemeStore((state) => state.setServerSectionCollapsed);

  const [userServers, setUserServers] = useState<ServerInfo[]>([]);
  const [serverDetails, setServerDetails] = useState<Record<string, ServerPingInfo>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [connectingAddress, setConnectingAddress] = useState<string | null>(null);
  const location = useLocation();
  const isPlayRoute =
    location.pathname === "/play" || location.pathname === "/" || location.pathname === "";
  const shouldPingServers = isPlayRoute && !isCollapsed;
  const wasCollapsedRef = useRef(isCollapsed);

  const pingUserServers = useCallback(async (servers: ServerInfo[], signal: AbortSignal) => {
    for (const srv of servers) {
      if (signal.aborted || !srv.address) continue;

      let pingInfo: ServerPingInfo = {};

      try {
        pingInfo = (await invoke("ping_minecraft_server", {
          address: srv.address,
        })) as ServerPingInfo;
      } catch (e) {
        if (signal.aborted) return;
        console.warn("local ping failed", e);
      }

      if (signal.aborted) return;

      try {
        const response = await fetch(`https://eu.mc-api.net/v3/server/ping/${srv.address}`, {
          signal,
        });
        if (response.ok) {
          const data = (await response.json()) as Record<string, unknown>;
          const rawMotd = extractMotd(data);
          if (rawMotd) {
            pingInfo.motd_plain = stripMotdFormatting(rawMotd);
          }
          const version = data.version as { name?: string } | undefined;
          if (version?.name) pingInfo.version_name = version.name;
          const players = data.players as { online?: number; max?: number } | undefined;
          if (typeof players?.online === "number") pingInfo.players_online = players.online;
          if (typeof players?.max === "number") pingInfo.players_max = players.max;
          const favicon = data.favicon_base64;
          if (typeof favicon === "string") {
            pingInfo.favicon_base64 = favicon.replace("data:image/png;base64,", "");
          }
        }
      } catch (e) {
        if (signal.aborted) return;
        console.warn("eu.mc-api.net fetch failed", e);
      }

      if (signal.aborted) return;

      if (pingInfo.description && !pingInfo.motd_plain) {
        pingInfo.motd_plain = stripMotdFormatting(pingInfo.description);
      }

      setServerDetails((prev) => ({
        ...prev,
        [srv.address!]: { ...prev[srv.address!], ...pingInfo },
      }));
    }
  }, []);

  const loadUserServers = useCallback(async () => {
    setIsLoading(true);
    try {
      setError(null);

      const profilesResponse = (await invoke("get_all_profiles_and_last_played")) as {
        all_profiles?: Array<{ id: string; name: string; game_version?: string; loader?: string }>;
      };
      const profiles = profilesResponse.all_profiles || [];

      if (!Array.isArray(profiles) || profiles.length === 0) {
        setUserServers([]);
        return;
      }

      const allServers: ServerInfo[] = [];

      for (const profile of profiles) {
        try {
          const fetched = await invoke("get_servers_for_profile", { profileId: profile.id });
          const raw = fetched as ServerInfo[];
          allServers.push(
            ...raw.map((server) => ({
              ...server,
              profileId: profile.id,
              profileName: profile.name,
              gameVersion: profile.game_version,
              loader: profile.loader,
            })),
          );
        } catch (e) {
          console.warn(`Failed to fetch servers for profile ${profile.name}:`, e);
        }
      }

      const dedup: ServerInfo[] = [];
      const seen = new Set<string>();
      allServers.forEach((s) => {
        if (!s.address) return;
        const key = s.address.toLowerCase();
        if (!seen.has(key)) {
          seen.add(key);
          dedup.push(s);
        }
      });

      setUserServers(dedup);
    } catch (err) {
      console.error("Failed to load user servers:", err);
      setError(err instanceof Error ? err.message : "Failed to load servers");
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleServerConnect = async (server: ServerInfo) => {
    if (!server.address || !server.profileId) return;

    setConnectingAddress(server.address);
    try {
      await invoke("launch_profile", {
        id: server.profileId,
        quickPlayMultiplayer: server.address,
      });
    } catch (e) {
      console.error("Failed to launch profile:", e);
    } finally {
      setConnectingAddress(null);
    }
  };

  const handleResizeStart = useCallback(
    (e: React.MouseEvent) => {
      if (isCollapsed) return;
      setIsResizing(true);
      setStartX(e.clientX);
      setStartWidth(serverSectionWidth);
      e.preventDefault();
    },
    [isCollapsed, serverSectionWidth],
  );

  const handleResizeEnd = useCallback(() => {
    setIsResizing(false);
  }, []);

  const handleResizeMove = useCallback(
    (e: MouseEvent) => {
      if (!isResizing) return;
      const deltaX = e.clientX - startX;
      const newWidth = Math.max(280, Math.min(420, startWidth - deltaX));
      setServerSectionWidth(newWidth);
    },
    [isResizing, startX, startWidth, setServerSectionWidth],
  );

  useEffect(() => {
    if (!isPlayRoute) {
      return;
    }

    void loadUserServers();
  }, [isPlayRoute, loadUserServers]);

  useEffect(() => {
    if (!shouldPingServers || userServers.length === 0) {
      return;
    }

    const abortController = new AbortController();
    void pingUserServers(userServers, abortController.signal);

    return () => {
      abortController.abort();
    };
  }, [shouldPingServers, userServers, pingUserServers]);

  const handleExpandPanel = useCallback(() => {
    setServerSectionCollapsed(false);
  }, [setServerSectionCollapsed]);

  useEffect(() => {
    const wasCollapsed = wasCollapsedRef.current;
    wasCollapsedRef.current = isCollapsed;

    if (wasCollapsed && !isCollapsed && isPlayRoute) {
      void loadUserServers();
    }
  }, [isCollapsed, isPlayRoute, loadUserServers]);

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => handleResizeMove(e);
    const handleMouseUp = () => handleResizeEnd();

    if (isResizing) {
      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    }

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isResizing, handleResizeMove, handleResizeEnd]);

  const renderContent = () => {
    if (isLoading) {
      return <LoadingState message="Loading servers..." />;
    }

    if (error) {
      return (
        <div className="px-2 py-6 text-center">
          <Icon icon="streamline-cyber:server" className="mx-auto mb-2 h-8 w-8 text-[var(--text-muted)]" />
          <p className="text-sm text-[var(--text-secondary)]">Could not load your servers.</p>
        </div>
      );
    }

    if (userServers.length === 0) {
      return (
        <div className="px-2 py-6 text-center">
          <Icon icon="streamline-cyber:server" className="mx-auto mb-2 h-8 w-8 text-[var(--text-muted)]" />
          <p className="text-sm text-[var(--text-secondary)]">No servers added to your profiles.</p>
        </div>
      );
    }

    return (
      <div className="space-y-2">
        {userServers.map((server) => {
          const details = server.address ? serverDetails[server.address] : undefined;
          return (
            <ServerListItem
              key={`${server.address}-${server.profileId}`}
              server={server}
              details={details}
              isConnecting={connectingAddress === server.address}
              onConnect={handleServerConnect}
            />
          );
        })}
      </div>
    );
  };

  if (isCollapsed) {
    return (
      <div
        className={cn(
          "relative flex h-full flex-shrink-0 flex-col items-center border-l border-[var(--surface-border)] transition-all duration-200",
          className,
        )}
        style={{ width: COLLAPSED_SERVER_PANEL_WIDTH }}
      >
        <button
          type="button"
          onClick={handleExpandPanel}
          title="Show your servers"
          aria-label="Show your servers"
          className="mt-4 flex h-10 w-10 items-center justify-center rounded-lg bg-[rgba(var(--accent-rgb),0.12)] text-[var(--accent)] transition-colors hover:bg-[rgba(var(--accent-rgb),0.2)]"
        >
          <Icon icon="streamline-cyber:server" className="h-5 w-5" />
        </button>
        {!isLoading && userServers.length > 0 && (
          <span className="mt-2 inline-flex h-5 min-w-5 select-none items-center justify-center rounded-full bg-[rgba(var(--accent-rgb),0.15)] px-1.5 text-[10px] font-semibold text-[var(--accent)]">
            {userServers.length}
          </span>
        )}
      </div>
    );
  }

  return (
    <div
      ref={serverRef}
      className={cn("relative flex h-full flex-shrink-0 flex-col transition-all duration-200", className)}
      style={{ width: `${serverSectionWidth}px` }}
    >
      <div
        className={cn(
          "absolute bottom-0 left-0 top-0 z-10 w-1 cursor-ew-resize hover:bg-[var(--accent)]/20",
          isResizing && "bg-[var(--accent)]/30",
        )}
        onMouseDown={handleResizeStart}
      />

      <div className="border-b border-[var(--surface-border)] px-4 py-4">
        <div className="flex items-start justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2.5">
            <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-[rgba(var(--accent-rgb),0.12)] text-[var(--accent)]">
              <Icon icon="streamline-cyber:server" className="h-5 w-5" />
            </div>
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-white">Your Servers</h2>
              <p className="text-xs text-[var(--text-muted)]">From your profiles</p>
            </div>
          </div>

          <IconButton
            size="sm"
            title="Hide servers"
            aria-label="Hide servers"
            onClick={() => setServerSectionCollapsed(true)}
            className="flex-shrink-0"
          >
            <Icon icon="solar:double-alt-arrow-right-bold" className="h-4 w-4" />
          </IconButton>
        </div>
      </div>

      <div className="custom-scrollbar flex-1 overflow-y-auto px-4 py-3">{renderContent()}</div>
    </div>
  );
}
