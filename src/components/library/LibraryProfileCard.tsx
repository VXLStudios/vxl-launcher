"use client";

import { useCallback, useEffect, useMemo, useState, type MouseEvent } from "react";
import { Icon } from "@iconify/react";
import { toast } from "react-hot-toast";
import type { Profile } from "../../types/profile";
import { ProfileIconV2 } from "../profiles/ProfileIconV2";
import { ExportProfileModal } from "../profiles/ExportProfileModal";
import { Card } from "../ui-v2";
import { TagBadge } from "../ui/TagBadge";
import { cn } from "../../lib/utils";
import { useProfileLaunch } from "../../hooks/useProfileLaunch";
import { SettingsContextMenu, type ContextMenuItem } from "../ui/SettingsContextMenu";
import { useThemeStore } from "../../store/useThemeStore";
import { useProfileSettingsStore } from "../../store/profile-settings-store";
import { useProfileDuplicateStore } from "../../store/profile-duplicate-store";
import { useProfileStore } from "../../store/profile-store";
import { useGlobalModal } from "../../hooks/useGlobalModal";
import { useConfirmDialog } from "../../hooks/useConfirmDialog";
import * as ProfileService from "../../services/profile-service";
import UnifiedService from "../../services/unified-service";

type LibraryCardLayout = "list" | "grid" | "compact";

interface LibraryProfileCardProps {
  profile: Profile;
  selected?: boolean;
  onSelect?: (profile: Profile) => void;
  layout?: LibraryCardLayout;
}

function formatLoader(loader: string | undefined) {
  if (!loader || loader === "vanilla") return "Vanilla";
  if (loader === "neoforge") return "NeoForge";
  return loader.charAt(0).toUpperCase() + loader.slice(1);
}

function formatGroup(group: string) {
  const normalized = group.trim();
  if (!normalized) return null;
  if (normalized.toUpperCase() === "MODPACKS") return null;
  return normalized.charAt(0).toUpperCase() + normalized.slice(1).toLowerCase();
}

function getLoaderIcon(loader: string | undefined) {
  switch (loader) {
    case "fabric":
      return "/icons/fabric.png";
    case "forge":
      return "/icons/forge.png";
    case "quilt":
      return "/icons/quilt.png";
    case "neoforge":
      return "/icons/neoforge.png";
    default:
      return "/icons/minecraft.png";
  }
}

function formatLastPlayed(lastPlayed: string | null | undefined) {
  if (!lastPlayed) return null;

  const date = new Date(lastPlayed);
  const now = new Date();
  const diffInMs = now.getTime() - date.getTime();
  const diffInMinutes = Math.floor(diffInMs / (1000 * 60));
  const diffInHours = Math.floor(diffInMinutes / 60);
  const diffInDays = Math.floor(diffInHours / 24);
  const diffInWeeks = Math.floor(diffInDays / 7);
  const diffInMonths = Math.floor(diffInDays / 30);
  const diffInYears = Math.floor(diffInDays / 365);

  if (diffInMinutes < 1) return "Just now";
  if (diffInMinutes < 60) return `${diffInMinutes}m ago`;
  if (diffInHours < 24) return `${diffInHours}h ago`;
  if (diffInDays < 7) return `${diffInDays}d ago`;
  if (diffInWeeks < 4) return `${diffInWeeks}w ago`;
  if (diffInMonths < 12) return `${diffInMonths}mo ago`;
  return `${diffInYears}y ago`;
}

function ProfileMeta({
  profile,
  groupLabel,
  showLastPlayed = false,
}: {
  profile: Profile;
  groupLabel?: string | null;
  showLastPlayed?: boolean;
}) {
  const lastPlayed = showLastPlayed ? formatLastPlayed(profile.last_played) : null;

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-[var(--text-secondary)]">
      {profile.game_version && (
        <span className="inline-flex items-center gap-1.5">
          <img src="/icons/minecraft.png" alt="" className="h-3.5 w-3.5 object-contain opacity-90" />
          <span>{profile.game_version}</span>
        </span>
      )}

      {profile.game_version && (
        <span className="hidden h-3 w-px bg-[var(--surface-border-strong)] sm:block" aria-hidden />
      )}

      <span className="inline-flex items-center gap-1.5">
        <img
          src={getLoaderIcon(profile.loader)}
          alt=""
          className="h-3.5 w-3.5 object-contain opacity-90"
        />
        <span>{formatLoader(profile.loader)}</span>
      </span>

      {groupLabel && (
        <>
          <span className="hidden h-3 w-px bg-[var(--surface-border-strong)] sm:block" aria-hidden />
          <TagBadge size="sm" variant="flat" className="max-w-[140px] truncate">
            {groupLabel}
          </TagBadge>
        </>
      )}

      {lastPlayed && (
        <>
          <span className="hidden h-3 w-px bg-[var(--surface-border-strong)] sm:block" aria-hidden />
          <span className="text-[var(--text-muted)]">{lastPlayed}</span>
        </>
      )}
    </div>
  );
}

export function LibraryProfileCard({
  profile,
  selected = false,
  onSelect,
  layout = "list",
}: LibraryProfileCardProps) {
  const isCompact = layout === "compact";
  const groupLabel = profile.group ? formatGroup(profile.group) : null;
  const iconSize = isCompact ? "sm" : "md";
  const { handleLaunch, isLaunching } = useProfileLaunch({
    profileId: profile.id,
    profileName: profile.name,
  });
  const { openContextMenuId, setOpenContextMenuId } = useThemeStore();
  const { openModal: openSettingsModal } = useProfileSettingsStore();
  const { openModal: openDuplicateModal } = useProfileDuplicateStore();
  const deleteProfile = useProfileStore((state) => state.deleteProfile);
  const { showModal, hideModal } = useGlobalModal();
  const { confirm, confirmDialog } = useConfirmDialog();
  const [isContextMenuOpen, setIsContextMenuOpen] = useState(false);
  const contextMenuId = `library-profile-${profile.id}`;
  const [contextMenuPosition, setContextMenuPosition] = useState({ x: 0, y: 0 });

  useEffect(() => {
    if (openContextMenuId && openContextMenuId !== contextMenuId && isContextMenuOpen) {
      setIsContextMenuOpen(false);
    }
  }, [openContextMenuId, contextMenuId, isContextMenuOpen]);

  const handleOpenFolder = useCallback((targetProfile: Profile) => {
    toast.promise(ProfileService.openProfileFolder(targetProfile.id), {
      loading: `Opening folder for '${targetProfile.name}'...`,
      success: `Opened folder for '${targetProfile.name}'.`,
      error: (err) => {
        const message = err instanceof Error ? err.message : String(err);
        if (
          message.toLowerCase().includes("not found") ||
          message.toLowerCase().includes("does not exist")
        ) {
          return `Profile folder for '${targetProfile.name}' does not exist yet. Launch the profile to create it.`;
        }
        return `Failed to open folder: ${message}`;
      },
    });
  }, []);

  const handleDeleteProfile = useCallback(async (targetProfile: Profile) => {
    if (targetProfile.is_standard_version) {
      toast.error("Standard profiles cannot be deleted.");
      return;
    }

    const confirmed = await confirm({
      title: "Delete profile",
      message: `Are you sure you want to delete profile "${targetProfile.name}"? This action cannot be undone.`,
      confirmText: "DELETE",
      cancelText: "CANCEL",
      type: "danger",
      fullscreen: true,
    });

    if (!confirmed) {
      return;
    }

    await toast.promise(deleteProfile(targetProfile.id), {
      loading: `Deleting profile '${targetProfile.name}'...`,
      success: `Profile '${targetProfile.name}' deleted successfully.`,
      error: (err) =>
        `Failed to delete profile: ${err instanceof Error ? err.message : String(err)}`,
    });
  }, [confirm, deleteProfile]);

  const handleFetchModpackVersions = useCallback(async (targetProfile: Profile) => {
    if (!targetProfile.modpack_info?.source) {
      return;
    }

    const { ModpackVersionsModal } = await import("../modals/ModpackVersionsModal");

    showModal(`modpack-versions-${targetProfile.id}`, (
      <ModpackVersionsModal
        isOpen={true}
        onClose={() => hideModal(`modpack-versions-${targetProfile.id}`)}
        versions={null}
        modpackName={targetProfile.name}
        profileId={targetProfile.id}
        onSwitchComplete={async () => {
          await useProfileStore.getState().fetchProfiles(true);
        }}
      />
    ));

    try {
      const versions = await UnifiedService.getModpackVersions(targetProfile.modpack_info.source);
      hideModal(`modpack-versions-${targetProfile.id}`);
      showModal(`modpack-versions-${targetProfile.id}`, (
        <ModpackVersionsModal
          isOpen={true}
          onClose={() => hideModal(`modpack-versions-${targetProfile.id}`)}
          versions={versions}
          modpackName={targetProfile.name}
          profileId={targetProfile.id}
          onSwitchComplete={async () => {
            await useProfileStore.getState().fetchProfiles(true);
          }}
        />
      ));
    } catch (err) {
      console.error("Failed to fetch modpack versions:", err);
      hideModal(`modpack-versions-${targetProfile.id}`);
      toast.error("Failed to fetch modpack versions");
    }
  }, [hideModal, showModal]);

  const contextMenuItems = useMemo((): ContextMenuItem[] => [
    {
      id: "play",
      label: isLaunching ? "Stop Launch" : "Play",
      icon: isLaunching ? "solar:stop-bold" : "solar:play-bold",
      onClick: () => {
        void handleLaunch();
      },
    },
    {
      id: "edit",
      label: "Edit Profile",
      icon: "solar:settings-bold",
      onClick: (targetProfile) => {
        openSettingsModal(targetProfile);
      },
    },
    {
      id: "duplicate",
      label: "Duplicate",
      icon: "solar:copy-bold",
      onClick: (targetProfile) => {
        openDuplicateModal(targetProfile);
      },
    },
    {
      id: "export",
      label: "Export",
      icon: "solar:download-bold",
      onClick: (targetProfile) => {
        showModal(`export-profile-${targetProfile.id}`, (
          <ExportProfileModal
            profile={targetProfile}
            isOpen={true}
            onClose={() => hideModal(`export-profile-${targetProfile.id}`)}
          />
        ));
      },
    },
    {
      id: "open-folder",
      label: "Open Folder",
      icon: "solar:folder-bold",
      onClick: handleOpenFolder,
    },
    ...(profile.modpack_info
      ? [{
          id: "modpack-versions",
          label: "Modpack Versions",
          icon: "solar:archive-bold",
          onClick: (targetProfile: Profile) => {
            void handleFetchModpackVersions(targetProfile);
          },
        }]
      : []),
    {
      id: "delete",
      label: "Delete",
      icon: "solar:trash-bin-trash-bold",
      destructive: true,
      separator: true,
      onClick: (targetProfile) => {
        void handleDeleteProfile(targetProfile);
      },
    },
  ], [
    handleDeleteProfile,
    handleFetchModpackVersions,
    handleLaunch,
    handleOpenFolder,
    hideModal,
    isLaunching,
    openDuplicateModal,
    openSettingsModal,
    profile.modpack_info,
    showModal,
  ]);

  const handleIconClick = (event: MouseEvent) => {
    event.stopPropagation();
    void handleLaunch();
  };

  const handleContextMenu = (event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (openContextMenuId && openContextMenuId !== contextMenuId) {
      setOpenContextMenuId(null);
    }

    setIsContextMenuOpen(true);
    setOpenContextMenuId(contextMenuId);

    setContextMenuPosition({
      x: event.clientX,
      y: event.clientY,
    });
  };

  return (
    <div className="relative">
      <Card
        interactive
        selected={selected}
        onClick={() => onSelect?.(profile)}
        onContextMenu={handleContextMenu}
        className={cn(
          "group flex items-center overflow-hidden bg-[var(--surface-raised)]",
          isCompact ? "gap-3 p-3" : "gap-4 p-4",
          layout === "grid" && "h-full",
        )}
      >
      <button
        type="button"
        className="group/play relative flex-shrink-0 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
        onClick={handleIconClick}
        title={isLaunching ? "Stop launch" : `Play ${profile.name}`}
        aria-label={isLaunching ? "Stop launch" : `Play ${profile.name}`}
      >
        <ProfileIconV2 profile={profile} size={iconSize} tone="neutral" />

        <div
          className={cn(
            "absolute inset-0 flex items-center justify-center rounded-xl bg-black/50 backdrop-blur-sm transition-opacity duration-150",
            isLaunching ? "opacity-100" : "opacity-0 group-hover:opacity-100",
          )}
        >
          <Icon
            icon={isLaunching ? "solar:stop-bold" : "solar:play-bold"}
            className={cn(
              "text-white transition-colors duration-150 group-hover/play:text-[var(--accent)]",
              isCompact ? "h-6 w-6" : "h-7 w-7",
            )}
          />
        </div>
      </button>

      <div className="min-w-0 flex-1">
        <h3
          className={cn(
            "truncate font-semibold text-white",
            isCompact ? "text-sm" : "text-[15px] leading-snug",
          )}
        >
          {profile.name}
        </h3>

        <div className="mt-1">
          <ProfileMeta
            profile={profile}
            groupLabel={groupLabel}
            showLastPlayed={layout === "list"}
          />
        </div>
      </div>

      {layout === "list" && (
        <Icon
          icon="solar:alt-arrow-right-linear"
          className="ml-1 h-4 w-4 flex-shrink-0 text-[var(--text-muted)] opacity-0 transition-opacity group-hover:opacity-100"
        />
      )}
      </Card>

      <SettingsContextMenu
        profile={profile}
        isOpen={isContextMenuOpen}
        position={contextMenuPosition}
        items={contextMenuItems}
        onClose={() => {
          setIsContextMenuOpen(false);
          setOpenContextMenuId(null);
        }}
      />

      {confirmDialog}
    </div>
  );
}
