"use client";

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Icon } from "@iconify/react";
import { useNavigate } from "react-router-dom";
import { Button as UiV2Button } from "../../../ui-v2";
import { Button } from "../../../ui/buttons/Button";
import { ContentActionButtons, type ContentActionButton } from "../../../ui/ContentActionButtons";
import type { ActionButtonVariant } from "../../../ui/ActionButton";
import { GenericDetailListItem } from "../items/GenericDetailListItem";
import { TagBadge } from "../../../ui/TagBadge";
import { useThemeStore } from "../../../../store/useThemeStore";
import { GenericContentTab } from "../../../ui/GenericContentTab";
import { preloadIcons } from "../../../../lib/icon-utils";
import type { Profile } from "../../../../types/profile";
import { SearchInput } from "../../../ui/SearchInput";
import { SearchWithFilters } from "../../../ui/SearchWithFilters";
import { CheckboxV2 } from "../../../ui/CheckboxV2";
import { ConfirmDeleteDialog } from "../../../modals/ConfirmDeleteDialog";
import { formatFileSize } from "../../../../utils/format-file-size";
import { toast } from "react-hot-toast";
import {
  type LocalContentItem,
  type LocalContentType,
  useLocalContentManager,
} from "../../../../hooks/useLocalContentManager";

// SVG Components converted to data URLs
const modrinthSvgDataUrl = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath fill='%2322c55e' d='M12.252.004a11.78 11.768 0 0 0-8.92 3.73a11 10.999 0 0 0-2.17 3.11a11.37 11.359 0 0 0-1.16 5.169c0 1.42.17 2.5.6 3.77c.24.759.77 1.899 1.17 2.529a12.3 12.298 0 0 0 8.85 5.639c.44.05 2.54.07 2.76.02c.2-.04.22.1-.26-1.7l-.36-1.37l-1.01-.06a8.5 8.489 0 0 1-5.18-1.8a5.34 5.34 0 0 1-1.3-1.26c0-.05.34-.28.74-.5a37.572 37.545 0 0 1 2.88-1.629c.03 0 .5.45 1.06.98l1 .97l2.07-.43l2.06-.43l1.47-1.47c.8-.8 1.48-1.5 1.48-1.52c0-.09-.42-1.63-.46-1.7c-.04-.06-.2-.03-1.02.18c-.53.13-1.2.3-1.45.4l-.48.15l-.53.53l-.53.53l-.93.1l-.93.07l-.52-.5a2.7 2.7 0 0 1-.96-1.7l-.13-.6l.43-.57c.68-.9.68-.9 1.46-1.1c.4-.1.65-.2.83-.33c.13-.099.65-.579 1.14-1.069l.9-.9l-.7-.7l-.7-.7l-1.95.54c-1.07.3-1.96.53-1.97.53c-.03 0-2.23 2.48-2.63 2.97l-.29.35l.28 1.03c.16.56.3 1.16.31 1.34l.03.3l-.34.23c-.37.23-2.22 1.3-2.84 1.63c-.36.2-.37.2-.44.1c-.08-.1-.23-.6-.32-1.03c-.18-.86-.17-2.75.02-3.73a8.84 8.839 0 0 1 7.9-6.93c.43-.03.77-.08.78-.1c.06-.17.5-2.999.47-3.039c-.01-.02-.1-.02-.2-.03Zm3.68.67c-.2 0-.3.1-.37.38c-.06.23-.46 2.42-.46 2.52c0 .04.1.11.22.16a8.51 8.499 0 0 1 2.99 2a8.38 8.379 0 0 1 2.16 3.449a6.9 6.9 0 0 1 .4 2.8c0 1.07 0 1.27-.1 1.73a9.37 9.369 0 0 1-1.76 3.769c-.32.4-.98 1.06-1.37 1.38c-.38.32-1.54 1.1-1.7 1.14c-.1.03-.1.06-.07.26c.03.18.64 2.56.7 2.78l.06.06a12.07 12.058 0 0 0 7.27-9.4c.13-.77.13-2.58 0-3.4a11.96 11.948 0 0 0-5.73-8.578c-.7-.42-2.05-1.06-2.25-1.06Z'%3E%3C/path%3E%3C/svg%3E";

const curseforgeSvgDataUrl = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath fill='%23f97316' d='M18.326 9.214s4.9-.772 5.674-3.026h-7.507V4.4H0l2.032 2.358v2.415s5.127-.267 7.11 1.237c2.714 2.516-3.053 5.917-3.053 5.917l-.99 3.273c1.547-1.473 4.494-3.377 9.899-3.286c-2.057.65-4.125 1.665-5.735 3.286h10.925l-1.029-3.273s-7.918-4.668-.833-7.112z'%3E%3C/path%3E%3C/svg%3E";
import type { UnifiedVersion, UnifiedModpackVersionsResponse, ModpackSwitchRequest } from "../../../../types/unified";
import { ModPlatform, UnifiedVersionType, UnifiedDependencyType } from "../../../../types/unified";
import * as ProfileService from "../../../../services/profile-service";
import * as ContentService from "../../../../services/content-service"; // Added import
import {
  ContentType as BackendContentType,
  type SwitchContentVersionPayload,
} from "../../../../types/content"; // Added import
import { ThemedSurface } from "../../../ui/ThemedSurface";
import { useAppDragDropStore } from "../../../../store/appStore"; // Import the store
import { createPortal } from "react-dom";
import { ModrinthService } from "../../../../services/modrinth-service"; // Added import
import UnifiedService from "../../../../services/unified-service";
import { ModVersionCache } from "../../../../store/mod-version-cache"; // Added import
import { EmptyState } from "../../../ui-v2/EmptyState";
import { useProfileStore } from "../../../../store/profile-store"; // Added import
import { useConfirmDialog } from "../../../../hooks/useConfirmDialog"; // Added import
import { Tooltip } from "../../../ui/Tooltip"; // Added for custom tooltips
import { ActionButton } from "../../../ui/ActionButton"; // Added for custom update button
import { ModUpdateText, useModUpdateText } from "../../../ui/ModUpdateText"; // Added for formatted update text
import { getUpdateIdentifier } from "../../../../utils/update-identifier-utils";
import { parseMotdToHtml } from "../../../../utils/motd-utils";

/**
 * Determines if a given version is the currently installed version for an item
 * Handles different info structures (modrinth_info, curseforge_info) and version matching logic
 */
function isCurrentInstalledVersion(
  version: UnifiedVersion,
  item: LocalContentItem,
  debugMode: boolean = false
): boolean {
  // Check Modrinth info first (most common)
  const localModrinthInfo = item.modrinth_info;
  if (localModrinthInfo) {
    // Prioritize version_id if it exists on localModrinthInfo (typical for GenericModrinthInfo)
    if (
      typeof localModrinthInfo === "object" &&
      localModrinthInfo !== null &&
      "version_id" in localModrinthInfo &&
      localModrinthInfo.version_id === version.id
    ) {
      if (debugMode) {
        console.log(`[${item.filename}] Version match by version_id: ${localModrinthInfo.version_id}`);
      }
      return true;
    }

    // Fallback to id if version_id didn't match or doesn't exist (typical for full ModrinthVersion object)
    if (
      typeof localModrinthInfo === "object" &&
      localModrinthInfo !== null &&
      "id" in localModrinthInfo &&
      (localModrinthInfo as any).id === version.id
    ) {
      if (debugMode) {
        console.log(`[${item.filename}] Version match by id fallback: ${(localModrinthInfo as any).id}`);
      }
      return true;
    }
  }

  // Check CurseForge info
  const localCurseForgeInfo = item.curseforge_info;
  if (localCurseForgeInfo) {
    // For CurseForge, we primarily check file_id
    if (
      typeof localCurseForgeInfo === "object" &&
      localCurseForgeInfo !== null &&
      "file_id" in localCurseForgeInfo &&
      localCurseForgeInfo.file_id === version.id
    ) {
      if (debugMode) {
        console.log(`[${item.filename}] CurseForge version match by file_id: ${localCurseForgeInfo.file_id}`);
      }
      return true;
    }
  }

  // Optional: secondary check by version_number if no ID match - can be less reliable if IDs truly differ for same version string
  // This is commented out as it's less reliable, but could be useful in some edge cases
  /*
  if (localModrinthInfo?.version_number === version.version_number) {
    if (debugMode) {
      console.log(`[${item.filename}] Version match by version_number fallback: ${version.version_number}`);
    }
    return true;
  }
  */

  if (debugMode) {
    const installedVersionStr = localModrinthInfo?.version_number ||
                               localCurseForgeInfo?.version_number ||
                               "N/A";
    const installedIdToCompare = localModrinthInfo?.version_id ||
                                (localModrinthInfo && 'id' in localModrinthInfo ? localModrinthInfo.id : undefined) ||
                                localCurseForgeInfo?.file_id ||
                                "N/A";

    console.log(
      `[${item.filename}] Checking: List ver: ${version.version_number} (ID: ${version.id}) vs Installed: ${installedVersionStr} (Stored ID: ${installedIdToCompare}) -> NO MATCH`
    );
  }

  return false;
}

function getLatestModpackVersion(
  versions: UnifiedModpackVersionsResponse,
): UnifiedVersion | null {
  if (versions.latest_version) {
    return versions.latest_version;
  }

  if (versions.all_versions.length === 0) {
    return null;
  }

  return [...versions.all_versions].sort(
    (a, b) =>
      new Date(b.date_published).getTime() - new Date(a.date_published).getTime(),
  )[0];
}

// Generic icons that can be used across different content types
const LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD = [
  "solar:gallery-bold-duotone", // Fallback icon, empty state (can be overridden by prop)
  "solar:settings-bold-duotone", // Placeholder for potential future settings
  "solar:info-circle-bold-duotone", // Placeholder for potential future info
  "solar:check-circle-bold", // Enabled status
  "solar:close-circle-bold", // Disabled status
  "solar:folder-open-bold-duotone",
  "solar:trash-bin-trash-bold",
  "solar:menu-dots-bold",
  "solar:sort-from_top_to_bottom-bold-duotone", // Placeholder for sort
  "solar:refresh-square-bold-duotone", // Refresh button in list item (not used yet)
  "solar:download-minimalistic-bold", // For Update Available button
  "solar:refresh-bold", // For Check for Updates loading spinner / general loading
  "solar:add-circle-bold-duotone", // For Add Content button
  "solar:refresh-outline", // For primary refresh button normal state
  "solar:download-minimalistic-bold", // For Update All button
  "solar:alt-arrow-down-bold", // For version dropdown button
  "solar:shield-cross-bold-duotone", // For NoRisk blocked badge
];

interface LocalContentTabV2Props<T extends LocalContentItem> {
  profile?: Profile;
  contentType: LocalContentType; // e.g., 'ResourcePack', 'ShaderPack'
  getDisplayFileName: (item: T) => string;
  itemTypeName: string; // Singular, e.g., "resource pack"
  itemTypeNamePlural: string; // Plural, e.g., "resource packs"
  addContentButtonText: string; // e.g., "Add Resource Packs"
  onAddContent?: () => void; // Action for the add button
  emptyStateIconOverride?: string; // Optional override for the main empty/fallback icon
  onRefreshRequired?: () => void;
  onBrowseContentRequest?: (browseContentType: string) => void; // Added new prop
  onChangeModpackVersion?: () => void;
}

// Hook for formatted update text
const { getUpdateText } = useModUpdateText();

export function LocalContentTabV2<T extends LocalContentItem>({
  profile,
  contentType,
  getDisplayFileName,
  itemTypeName,
  itemTypeNamePlural,
  addContentButtonText,
  onAddContent: onAddContentProp,
  emptyStateIconOverride,
  onRefreshRequired,
  onBrowseContentRequest, // Destructure new prop
  onChangeModpackVersion,
}: LocalContentTabV2Props<T>) {
  const navigate = useNavigate();
  const accentColor = useThemeStore((state) => state.accentColor);
  const { confirm, confirmDialog } = useConfirmDialog(); // Added hook
  const { copyProfile, fetchProfiles, updateProfile } = useProfileStore(); // Added updateProfile
  const {
    setActiveDropContext,
    registerRefreshCallback,
    unregisterRefreshCallback,
  } = useAppDragDropStore();

  const isInstalledModpack = Boolean(profile?.modpack_info);
  const [isCheckingModpackUpdates, setIsCheckingModpackUpdates] = useState(false);
  const [isUpdatingModpack, setIsUpdatingModpack] = useState(false);
  const [modpackHasUpdate, setModpackHasUpdate] = useState(false);
  const [latestModpackVersion, setLatestModpackVersion] = useState<UnifiedVersion | null>(null);
  const [isBlockedConfigLoaded, setIsBlockedConfigLoaded] = useState(false);
  const [openVersionDropdownId, setOpenVersionDropdownId] = useState<
    string | null
  >(null);
  const versionDropdownRef = useRef<HTMLDivElement>(null);
  const versionButtonRef = useRef<HTMLButtonElement | null>(null); // Allow null

  // State for version dropdown content
  const [availableVersions, setAvailableVersions] = useState<
    UnifiedVersion[] | null
  >(null);
  const [isLoadingVersions, setIsLoadingVersions] = useState(false);
  const [versionsError, setVersionsError] = useState<string | null>(null);


  const {
    items,
    isLoading,
    isFetchingHashes,
    isFetchingModrinthDetails,
    isAnyTaskRunning,
    error,
    searchQuery,
    setSearchQuery,
    selectedItemIds,
    handleItemSelectionChange,
    handleSelectAllToggle,
    areAllFilteredSelected,
    filteredItems,
    itemBeingToggled,
    itemBeingDeleted,
    isBatchToggling,
    isBatchDeleting,
    activeDropdownId,
    setActiveDropdownId,
    dropdownRef,
    isConfirmDeleteDialogOpen,
    isDialogActionLoading,
    handleConfirmDeletion,
    handleCloseDeleteDialog,
    itemToDeleteForDialog,
    modrinthIcons,
    curseforgeIcons,
    localArchiveIcons,
    getItemIcon,
    getItemPlatformDisplayName,
    fetchData,
    handleToggleItemEnabled,
    handleDeleteItem,
    handleBatchToggleSelected,
    handleBatchDeleteSelected,
    handleOpenItemFolder,
    contentUpdates,
    isCheckingUpdates,
    itemsBeingUpdated,
    contentUpdateError,
    isUpdatingAll,
    updatableContentCount,
    checkForContentUpdates,
    handleUpdateContentItem,
    handleUpdateAllAvailableContent,
    handleSwitchContentVersion,
    handleToggleItemUpdatesEnabled,
    handleBatchToggleSelectedUpdatesEnabled,
  } = useLocalContentManager<T>({
    // Hook uses the generic type T
    profile,
    contentType,
    getDisplayFileName,
    onRefreshRequired,
  });

  // Map UI contentType to BackendContentType for the store
  const backendContentTypeForStore = useMemo(() => {
    return contentType as BackendContentType;
  }, [contentType]);

  useEffect(() => {
    if (profile && backendContentTypeForStore) {
      setActiveDropContext(profile.id, backendContentTypeForStore);

      // Register refresh callback for this specific content type instance
      const refreshThisTabData = () => fetchData(true);
      registerRefreshCallback(backendContentTypeForStore, refreshThisTabData);
    }
    return () => {
      // Clear context when this specific tab instance is no longer focused or unmounted
      // Only clear if this was the one setting it (or manage this more globally)
      // For simplicity, we clear based on this instance.
      // A more robust solution might involve checking if the current global context matches this instance before clearing.
      setActiveDropContext(null, null);
      unregisterRefreshCallback(backendContentTypeForStore);
    };
  }, [
    profile,
    backendContentTypeForStore,
    setActiveDropContext,
    registerRefreshCallback,
    unregisterRefreshCallback,
  ]);

  // Helper to convert LocalContentType to URL-friendly string for BrowseTab
  const getBrowseTabContentType = (
    currentTabContentType: LocalContentType,
  ): string => {
    switch (currentTabContentType) {
      case "Mod":
        return "mods";
      case "ResourcePack":
        return "resourcepacks";
      case "ShaderPack":
        return "shaderpacks";
      case "DataPack":
        return "datapacks";
      default:
        return "mods"; // Fallback
    }
  };


  // Update default onAddContent to use the new dialog and service call
  const defaultOnAddContent = async () => {
    if (!profile) {
      toast.error("Profile data is not available to add content.");
      return;
    }

    const currentContentType = contentType;

    if (
      currentContentType !== "Mod" &&
      currentContentType !== "ResourcePack" &&
      currentContentType !== "ShaderPack" &&
      currentContentType !== "DataPack"
    ) {
      toast.error(
        `Local import is not configured for content type: ${currentContentType}`,
      );
      return;
    }

    const toastId = toast.loading(`Select ${itemTypeNamePlural.toLowerCase()} to import...`);
    try {
      await ContentService.pickAndInstallLocalContentToProfile({
        profile_id: profile.id,
        content_type: currentContentType as BackendContentType,
      });
      toast.dismiss(toastId);
      toast.success(
        `${itemTypeNamePlural} import finished. List will refresh.`,
      );
      fetchData(true);
      if (onRefreshRequired) {
        onRefreshRequired();
      }
    } catch (importError) {
      console.error(
        `Error importing local ${itemTypeNamePlural.toLowerCase()}:`,
        importError,
      );
      toast.error(
        `Failed to import ${itemTypeNamePlural.toLowerCase()}: ${importError instanceof Error ? importError.message : String(importError)}`,
        { id: toastId },
      );
    }
  };

  // Use the provided onAddContent prop if available, otherwise use the new default implementation.
  const effectiveOnAddContent = onAddContentProp || defaultOnAddContent;

  const isContentListReady =
    !isLoading && !isFetchingHashes && !isFetchingModrinthDetails && items.length > 0;

  const installedModpackVersionKey = profile?.modpack_info?.source
    ? profile.modpack_info.source.source === "modrinth"
      ? profile.modpack_info.source.version_id
      : String(profile.modpack_info.source.file_id)
    : null;

  useEffect(() => {
    setModpackHasUpdate(false);
    setLatestModpackVersion(null);
  }, [profile?.id, installedModpackVersionKey]);

  const handleUpdateCheckButtonClick = useCallback(async () => {
    if (isInstalledModpack && profile?.modpack_info?.source) {
      if (modpackHasUpdate) {
        setIsUpdatingModpack(true);
        try {
          let versionToInstall = latestModpackVersion;
          if (!versionToInstall) {
            const versions = await UnifiedService.getModpackVersions(
              profile.modpack_info.source,
            );
            versionToInstall = getLatestModpackVersion(versions);
          }

          if (!versionToInstall) {
            toast.error("Could not determine the latest modpack version");
            return;
          }

          let request: ModpackSwitchRequest;
          try {
            request = UnifiedService.buildModpackSwitchRequest(versionToInstall, profile.id);
          } catch (error) {
            toast.error(
              error instanceof Error ? error.message : "Failed to update modpack",
            );
            return;
          }

          await toast.promise(UnifiedService.switchModpackVersion(request), {
            loading: `Updating ${profile.name} to ${versionToInstall.version_number}...`,
            success: `Updated ${profile.name} to ${versionToInstall.version_number}`,
            error: (error) =>
              error instanceof Error ? error.message : "Failed to update modpack",
          });

          setModpackHasUpdate(false);
          setLatestModpackVersion(null);
          await fetchProfiles(true);
          await fetchData(true);
          onRefreshRequired?.();
        } catch (error) {
          console.error("Failed to update modpack:", error);
        } finally {
          setIsUpdatingModpack(false);
        }
        return;
      }

      setIsCheckingModpackUpdates(true);
      try {
        const versions = await UnifiedService.getModpackVersions(
          profile.modpack_info.source,
        );
        const latest = getLatestModpackVersion(versions);
        const hasUpdate = Boolean(versions.updates_available);
        setLatestModpackVersion(latest);
        setModpackHasUpdate(hasUpdate);
        toast.success(
          hasUpdate
            ? "A new modpack version is available"
            : "Modpack is up to date",
        );
      } catch (error) {
        console.error("Failed to check for modpack updates:", error);
        toast.error(
          error instanceof Error
            ? error.message
            : "Failed to check for modpack updates",
        );
      } finally {
        setIsCheckingModpackUpdates(false);
      }
      return;
    }

    if (updatableContentCount > 0) {
      await handleUpdateAllAvailableContent();
      return;
    }

    const availableUpdates = await checkForContentUpdates();
    if (availableUpdates > 0) {
      toast.success(
        availableUpdates === 1
          ? "1 update available"
          : `${availableUpdates} updates available`,
      );
    } else {
      toast.success("All mods are up to date");
    }
  }, [
    isInstalledModpack,
    profile,
    latestModpackVersion,
    modpackHasUpdate,
    fetchProfiles,
    fetchData,
    onRefreshRequired,
    updatableContentCount,
    handleUpdateAllAvailableContent,
    checkForContentUpdates,
  ]);

  const updateCheckButtonAction = useMemo(() => {
    if (contentType !== "Mod") {
      return null;
    }

    if (isInstalledModpack) {
      const isModpackBusy = isCheckingModpackUpdates || isUpdatingModpack;
      return {
        id: modpackHasUpdate ? "update-modpack" : "check-updates",
        label: isUpdatingModpack
          ? "UPDATING..."
          : isCheckingModpackUpdates
            ? "CHECKING..."
            : modpackHasUpdate
              ? "UPDATE MODPACK"
              : "CHECK FOR UPDATES",
        icon:
          isModpackBusy
            ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11]
            : modpackHasUpdate
              ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[14]
              : "solar:refresh-circle-bold",
        variant: (modpackHasUpdate ? "highlight" : "text") as ActionButtonVariant,
        disabled: isModpackBusy,
        loading: isModpackBusy,
        tooltip: modpackHasUpdate
          ? "Update this modpack to the latest version"
          : "Check this modpack for a newer version",
        onClick: handleUpdateCheckButtonClick,
      };
    }

    const hasUpdates = updatableContentCount > 0;

    return {
      id: hasUpdates ? "update-all" : "check-updates",
      label: isUpdatingAll
        ? "UPDATING ALL..."
        : isCheckingUpdates
          ? "CHECKING..."
          : hasUpdates
            ? `UPDATE ALL (${updatableContentCount})`
            : "CHECK FOR UPDATES",
      icon:
        isUpdatingAll || isCheckingUpdates
          ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11]
          : hasUpdates
            ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[14]
            : "solar:refresh-circle-bold",
      variant: (hasUpdates ? "highlight" : "text") as ActionButtonVariant,
      disabled:
        !isContentListReady || isUpdatingAll || isCheckingUpdates || isBatchToggling || isBatchDeleting,
      loading: isUpdatingAll || isCheckingUpdates,
      tooltip: hasUpdates
        ? `Update all ${updatableContentCount} mods with enabled updates`
        : "Check installed mods for available updates",
      onClick: handleUpdateCheckButtonClick,
    };
  }, [
    contentType,
    isInstalledModpack,
    modpackHasUpdate,
    isCheckingModpackUpdates,
    isUpdatingModpack,
    updatableContentCount,
    isUpdatingAll,
    isCheckingUpdates,
    isContentListReady,
    isBatchToggling,
    isBatchDeleting,
    handleUpdateCheckButtonClick,
  ]);

  useEffect(() => {
    preloadIcons(LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD);
  }, []);

  // Update dropdown position and fetch versions
  useEffect(() => {
    const updatePosition = () => {
      if (
        openVersionDropdownId &&
        versionDropdownRef.current &&
        versionButtonRef.current
      ) {
        if (!versionButtonRef.current.isConnected) {
          setOpenVersionDropdownId(null); // Close if button is detached
          return;
        }
        const buttonRect = versionButtonRef.current.getBoundingClientRect();
        const dropdownElement = versionDropdownRef.current;

        if (
          buttonRect.width === 0 &&
          buttonRect.height === 0 &&
          buttonRect.x === 0 &&
          buttonRect.y === 0
        ) {
          // Button likely not properly laid out yet, or invisible
          // Hide dropdown until next frame attempts to position it
          dropdownElement.style.visibility = "hidden";
          requestAnimationFrame(updatePosition); // Retry positioning on next frame
          return;
        }

        dropdownElement.style.top = `${buttonRect.bottom + 2}px`;
        dropdownElement.style.left = `${buttonRect.left}px`;
        dropdownElement.style.visibility = "visible";
      } else if (versionDropdownRef.current) {
        versionDropdownRef.current.style.visibility = "hidden";
      }
    };

    const fetchVersionsForDropdown = async () => {
      if (openVersionDropdownId) {
        const currentItem = items.find(
          (it) => it.filename === openVersionDropdownId,
        );

        // Determine platform and project ID from the item
        let platform: ModPlatform | null = null;
        let projectId: string | null = null;
        let platformName = "Unknown";

        if (currentItem) {
          if (currentItem.platform) {
            platform = currentItem.platform;
            if (platform === ModPlatform.Modrinth) {
              projectId = currentItem.modrinth_info?.project_id || null;
              platformName = "Modrinth";
            } else if (platform === ModPlatform.CurseForge) {
              projectId = currentItem.curseforge_info?.project_id || null;
              platformName = "CurseForge";
            }
          } else {
            // Fallback: determine platform from available info
            if (currentItem.modrinth_info?.project_id) {
              platform = ModPlatform.Modrinth;
              projectId = currentItem.modrinth_info.project_id;
              platformName = "Modrinth";
            } else if (currentItem.curseforge_info?.project_id) {
              platform = ModPlatform.CurseForge;
              projectId = currentItem.curseforge_info.project_id;
              platformName = "CurseForge";
            }
          }
        }

        if (platform && projectId) {
          const loadersArg =
            contentType === "Mod" && profile?.loader ? [profile.loader] : undefined;
          const gameVersionsArg = profile?.game_version ? [profile.game_version] : undefined;

          const cachedVersions = ModVersionCache.get(
            platform,
            projectId,
            profile?.game_version,
            loadersArg,
          );

          if (cachedVersions) {
            setAvailableVersions(cachedVersions);
            setIsLoadingVersions(false);
            setVersionsError(null);
            return;
          }

          setIsLoadingVersions(true);
          setAvailableVersions(null);
          setVersionsError(null);
          try {
            const versions = await UnifiedService.getModVersions({
              source: platform,
              project_id: projectId,
              loaders: loadersArg,
              game_versions: gameVersionsArg,
            });
            ModVersionCache.set(
              platform,
              projectId,
              profile?.game_version,
              loadersArg,
              versions.versions,
            );
            setAvailableVersions(versions.versions);
          } catch (error) {
            console.error(`Failed to fetch ${platformName} versions:`, error);
            setVersionsError(
              error instanceof Error
                ? `Failed to load versions from ${platformName}: ${error.message}`
                : `Failed to load versions from ${platformName}.`,
            );
          }
          setIsLoadingVersions(false);
        } else {
          // Item is not from a supported platform or no project_id available
          setAvailableVersions(null);
          setIsLoadingVersions(false);
          setVersionsError(
            currentItem
              ? `Version history not available on ${getItemPlatformDisplayName(currentItem)}.`
              : "Item not found.",
          );
        }
      }
    };

    if (openVersionDropdownId) {
      requestAnimationFrame(updatePosition);
      fetchVersionsForDropdown();
    } else {
      if (versionDropdownRef.current) {
        versionDropdownRef.current.style.visibility = "hidden";
      }
      versionButtonRef.current = null;
      // Reset version states when dropdown closes
      setAvailableVersions(null);
      setIsLoadingVersions(false);
      setVersionsError(null);
    }

    // Event listeners for keeping position updated and closing
    const scrollableParents = document.querySelectorAll(".custom-scrollbar");
    const handleScrollOrResize = () => requestAnimationFrame(updatePosition);

    scrollableParents.forEach((el) =>
      el.addEventListener("scroll", handleScrollOrResize),
    );
    window.addEventListener("scroll", handleScrollOrResize);
    window.addEventListener("resize", handleScrollOrResize);
    document.addEventListener("wheel", handleScrollOrResize, { passive: true });

    const handleClickOutside = (event: MouseEvent) => {
      if (
        versionDropdownRef.current &&
        !versionDropdownRef.current.contains(event.target as Node) &&
        versionButtonRef.current &&
        !versionButtonRef.current.contains(event.target as Node)
      ) {
        setOpenVersionDropdownId(null);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      scrollableParents.forEach((el) =>
        el.removeEventListener("scroll", handleScrollOrResize),
      );
      window.removeEventListener("scroll", handleScrollOrResize);
      window.removeEventListener("resize", handleScrollOrResize);
      document.removeEventListener("wheel", handleScrollOrResize);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [openVersionDropdownId]); // Effect runs when dropdown open state changes

  const handleItemTitleClick = useCallback((item: T) => {
    // Only navigate for mods with platform info (Modrinth or CurseForge)
    if (contentType !== "Mod") return;
    
    // Use the platform field to determine which platform the mod is from
    if (item.platform === ModPlatform.Modrinth && item.modrinth_info?.project_id) {
      navigate(`/mods/modrinth/${item.modrinth_info.project_id}`);
    } else if (item.platform === ModPlatform.CurseForge && item.curseforge_info?.project_id) {
      navigate(`/mods/curseforge/${item.curseforge_info.project_id}`);
    }
  }, [contentType, navigate]);

  const renderListItem = useCallback(
    (item: T) => {
      const itemTitleRaw = getDisplayFileName(item);
      const itemTitle = (
        <span dangerouslySetInnerHTML={{ __html: parseMotdToHtml(itemTitleRaw) }} />
      );
      const isToggling = itemBeingToggled === item.filename;
      const isDeleting = itemBeingDeleted === item.filename;
      const isCurrentlyUpdating = itemsBeingUpdated.has(item.filename);

      // Get update using the centralized identifier logic
      const updateIdentifier = getUpdateIdentifier(item);

      const updateAvailableVersion = updateIdentifier
        ? contentUpdates[updateIdentifier]
        : null;

      const isItemOpen = openVersionDropdownId === item.filename;

      // Get the appropriate icon using the platform-aware helper function
      const itemIconUrl = getItemIcon(item);

      let iconToShow: React.ReactNode;
      if (itemIconUrl) {
        iconToShow = (
          <img
            src={itemIconUrl}
            alt={`${itemTitle} ${getItemPlatformDisplayName(item)} icon`}
            className="w-full h-full object-contain image-pixelated"
            style={item.is_disabled ? {
              filter: "grayscale(100%) brightness(0.7)"
            } : undefined}
            onError={(e) => {
              (e.target as HTMLImageElement).style.visibility = "hidden";
            }}
          />
        );
      } else {
        iconToShow = (
          <Icon
            icon={
              emptyStateIconOverride || LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[0]
            }
            className="w-8 h-8 sm:w-10 sm:h-10 text-white/40"
          />
        );
      }

      const itemIconNode = (
        <div className="absolute inset-0 w-full h-full flex items-center justify-center">
          {iconToShow}
        </div>
      );

      const itemDescriptionNode = (() => {
        let descriptionText: string;
        let titleText: string;
        let versionText: string | null = null;

        const isItemWaitingForHash =
          item.sha1_hash === null && isFetchingHashes;
        const isItemStillLoadingDetails =
          isItemWaitingForHash ||
          (item.sha1_hash !== null &&
            !item.modrinth_info &&
            isFetchingModrinthDetails);

        if (item.fallback_version) {
          versionText = item.fallback_version;
          descriptionText = `Version: ${item.fallback_version}`;
          titleText = `Version: ${item.fallback_version}`;
        } else if (item.modrinth_info?.version_number) {
          versionText = item.modrinth_info.version_number;
          descriptionText = `Version: ${item.modrinth_info.version_number}`;
          titleText = `Modrinth Version: ${item.modrinth_info.version_number}`;
        } else if (isItemStillLoadingDetails) {
          descriptionText = "Loading...";
          titleText = "Loading details...";
        } else {
          descriptionText = formatFileSize(item.file_size || 0);
          titleText = `Size: ${formatFileSize(item.file_size || 0)}`;
        }

        return (
          <span title={titleText} className="flex items-center">
            {versionText ? (
              <>
                <span>Version: {versionText}</span>
                {contentType !== "NoRiskMod" && (
                  <div className="relative">
                    <button
                      ref={(el) => {
                        if (isItemOpen && el) {
                          versionButtonRef.current = el;
                        }
                      }}
                      className="ml-1 px-1 text-xs hover:bg-white/10 flex items-center border border-transparent hover:border-white/20"
                      onClick={(e) => {
                        e.stopPropagation();
                        if (isItemOpen) {
                          setOpenVersionDropdownId(null);
                        } else {
                          versionButtonRef.current = e.currentTarget;
                          setIsLoadingVersions(true);
                          setAvailableVersions(null);
                          setVersionsError(null);
                          setOpenVersionDropdownId(item.filename);
                        }
                      }}
                      title="View version options"
                    >
                      <span
                        className={` transition-transform duration-200 ${isItemOpen ? "rotate-90" : ""}`}
                      >
                        &gt;
                      </span>
                    </button>
                    {isItemOpen &&
                      createPortal(
                        <div
                          ref={versionDropdownRef}
                          className="fixed z-[100] "
                          style={{
                            backgroundColor: "rgb(20, 20, 20)",
                            border: `2px solid rgba(${parseInt(accentColor.value.substring(1, 3), 16)}, ${parseInt(accentColor.value.substring(3, 5), 16)}, ${parseInt(accentColor.value.substring(5, 7), 16)}, 0.6)`,
                            boxShadow: `0 6px 16px rgba(0, 0, 0, 0.7)`,
                            padding: "12px",
                            minWidth: "170px",
                            visibility: "hidden",
                          }}
                        >
                          {isLoadingVersions ? (
                            <div className="text-white/70 text-sm tracking-wider">
                              Loading versions...
                            </div>
                          ) : versionsError ? (
                            <div className="text-red-400 text-sm tracking-wider">
                              {versionsError}
                            </div>
                          ) : availableVersions &&
                            availableVersions.length > 0 ? (
                            <>
                              <div className="font-bold mb-2 text-sm tracking-wider">
                                Available Versions on {getItemPlatformDisplayName(item)}:
                              </div>
                              <div className="max-h-48 overflow-y-auto custom-scrollbar">
                                {availableVersions.map((version) => {
                                  // Use the centralized function to determine if this version is currently installed
                                  const isCurrent = isCurrentInstalledVersion(
                                    version,
                                    item,
                                    item.filename === openVersionDropdownId // Enable debug mode for the currently open dropdown
                                  );

                                  return (
                                    <div
                                      key={version.id}
                                      className={`p-1.5 text-xs hover:bg-white/10 cursor-pointer rounded-sm ${isCurrent ? "font-bold text-white" : "text-white/80"}`}
                                      style={
                                        {
                                          // No specific background for individual items unless it's the current one, which is handled by font-bold
                                        }
                                      }
                                      onClick={() => {
                                        handleSwitchContentVersion(
                                          item,
                                          version,
                                        );
                                        setOpenVersionDropdownId(null);
                                      }}
                                    >
                                      {version.name}
                                    </div>
                                  );
                                })}
                              </div>
                            </>
                          ) : availableVersions &&
                            availableVersions.length === 0 ? (
                            <div className="text-white/70 text-sm tracking-wider">
                              No other compatible versions found on {getItemPlatformDisplayName(item)}.
                            </div>
                          ) : (
                            <div className="text-white/70 text-sm tracking-wider">
                              Version history not available on {getItemPlatformDisplayName(item)}.
                            </div> // Fallback for items without version history
                          )}
                        </div>,
                        document.body,
                      )}
                  </div>
                )}
              </>
            ) : (
              <span>{descriptionText}</span>
            )}
            {item.is_directory && (
              <span className="ml-1 text-xs text-white/60">(Folder)</span>
            )}
          </span>
        );
      })();

      // Determine the primary platform for this item
      const itemPlatform = getItemPlatformDisplayName(item);
      const isDisabled = item.is_disabled;

      const itemBadgesNode = [
        // Platform badge - only show the primary platform
        ...(itemPlatform !== 'Local' ? [{
          icon: itemPlatform === 'Modrinth'
            ? modrinthSvgDataUrl
            : curseforgeSvgDataUrl,
          text: itemPlatform,
          color: isDisabled ? "#6b7280" : (itemPlatform === 'Modrinth' ? "#22c55e" : "#f97316"),
          iconFilter: isDisabled ? "grayscale(100%) brightness(0.7)" : undefined
        }] : []),

        // Source type badge (for local/custom mods)
        ...(item.source_type && item.source_type !== 'custom' ? [{
          text: item.source_type.charAt(0).toUpperCase() + item.source_type.slice(1),
          color: isDisabled ? "#6b7280" : "#f59e0b"
        }] : []),

        // Updates status badge - removed for cleaner look
      ];

      // Build action buttons array for this item
      const itemActions: ContentActionButton[] = [];

      // Check if update is available (used for custom tooltip rendering)
      // Note: NoRisk mods can also have updates available, but they won't be auto-updatable
      const hasUpdateAvailable = updateAvailableVersion && !isCurrentlyUpdating;
      let shouldShowUpdateButton = false;
      let isUpdateButtonDimmed = false;
      let updateButtonTooltip = "";

      // Check if this mod comes from a modpack (defined outside if block for broader scope)
      const isFromModPack = item.modpack_origin !== null && item.modpack_origin !== undefined;

      if (hasUpdateAvailable) {
        // Check if current version differs from available update
        const currentVersionId = item.modrinth_info?.version_id || item.curseforge_info?.file_id || item.id;
        if (currentVersionId !== updateAvailableVersion.id) {

          const currentVersion = item.modrinth_info?.version_number || item.curseforge_info?.version_number;

          if (isFromModPack && item.updates_enabled !== true) {
            // Show disabled update button for modpack mods (only if updates are not explicitly enabled)
            shouldShowUpdateButton = true;
            isUpdateButtonDimmed = true; // Show dimmed styling and actually disable
            updateButtonTooltip = `This mod comes from a modpack and cannot be updated individually. Updates should be handled through the modpack.`;
          } else {
            // Check if updates are enabled for this mod (consistent with updatableContentCount logic)
            // Default to enabled if null/undefined, only disabled if explicitly false
            const updatesEnabledDefault = item.updates_enabled ?? true;
            const hasUpdatesEnabled = contentType === 'Mod' && updatesEnabledDefault !== false;

            if (hasUpdatesEnabled) {
              // Normal update button for mods with updates enabled
              shouldShowUpdateButton = true;
              isUpdateButtonDimmed = false;
              updateButtonTooltip = getUpdateText(isFromModPack, updateAvailableVersion, currentVersion, item.modpack_origin, item.updates_enabled);
            } else {
              // Show dimmed update button for mods with updates disabled
              shouldShowUpdateButton = true;
              isUpdateButtonDimmed = true;
              updateButtonTooltip = `Update checks are disabled for this mod. Enable update checks first to allow automatic updates.`;
            }
          }
        }
      }

      // Build action buttons array for this item

      // Update action is handled separately with custom tooltip below
      // Only add update action if no update available
      if (isCurrentlyUpdating && !item.norisk_info) {
        itemActions.push({
          id: "updating",
          icon: LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11],
          variant: "secondary",
          tooltip: "Updating...",
          disabled: true,
          loading: true,
          onClick: () => {},
        });
      }

      // Main toggle action
      itemActions.push({
        id: "toggle",
        label: isToggling ? "..." : !item.is_disabled ? "DISABLE" : "ENABLE",
        icon: !item.is_disabled ? "solar:close-circle-bold" : "solar:check-circle-bold",
        variant: !item.is_disabled ? "secondary" : "primary",
        tooltip: !item.is_disabled ? "Disable this item" : "Enable this item",
        disabled: isToggling,
        onClick: () => handleToggleItemEnabled(item),
      });

      // Delete action (if not NoRisk mod) - icon-only
      if (!item.norisk_info) {
        itemActions.push({
          id: "delete",
          icon: isDeleting ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11] : LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[6],
          variant: "destructive",
          tooltip: `Delete ${itemTypeName}`,
          disabled: isDeleting,
          loading: isDeleting,
          onClick: () => handleDeleteItem(item),
        });
      }

      // More actions - icon-only
      itemActions.push({
        id: "more",
        icon: LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[7],
        variant: "secondary",
        tooltip: "More Actions",
        onClick: (e) => {
          e.stopPropagation();
          e.preventDefault();
          setActiveDropdownId(
            activeDropdownId === item.filename ? null : item.filename,
          );
        },
      });

      // Render update button separately with custom tooltip if available
      const updateButtonNode = shouldShowUpdateButton && updateAvailableVersion ? (
        <Tooltip
          content={
            <div className="max-w-xs">
              {/* Use the formatted text component for rich tooltips */}
              <ModUpdateText
                isFromModPack={isFromModPack}
                updateVersion={updateAvailableVersion}
                currentVersion={item.modrinth_info?.version_number || item.curseforge_info?.version_number}
                className="text-left"
                modpackOrigin={item.modpack_origin}
                updatesEnabled={item.updates_enabled}
              />
            </div>
          }
        >
          <ActionButton
            id="update"
            label="" // Force icon-only but keep highlight variant
            icon={LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[10]}
            variant={isUpdateButtonDimmed ? "secondary" : "highlight"}
            disabled={isUpdateButtonDimmed}
            onClick={() => handleUpdateContentItem(item, updateAvailableVersion)}
            className={isUpdateButtonDimmed ? "opacity-50 cursor-not-allowed grayscale" : ""}
          />
        </Tooltip>
      ) : null;

      const itemActionsNode = (
        <div className="flex items-center gap-2">
          {updateButtonNode}
          <ContentActionButtons
            actions={itemActions}
          />
        </div>
      );



      const itemDropdownNode = (
        <div
          ref={dropdownRef}
          className="absolute top-full right-0 mt-1 bg-black/90 backdrop-blur-sm border border-white/20 rounded-lg shadow-xl z-50 overflow-hidden"
          style={{
            minWidth: "200px",
          }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="py-2">
            <button
              onClick={() => {
                if (item.path) handleOpenItemFolder(item);
                setActiveDropdownId(null);
              }}
              className="w-full flex items-center gap-3 px-4 py-3 text-left  text-sm text-white/80 hover:text-white transition-colors duration-150"
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = `${accentColor.value}15`;
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
            >
              <Icon
                icon={LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[5]}
                className="w-4 h-4 flex-shrink-0 text-white/70"
              />
              <span className="flex-1">Open Folder</span>
            </button>
            {item.id && (
              <button
                onClick={() => {
                  handleToggleItemUpdatesEnabled(item);
                  setActiveDropdownId(null);
                }}
              className="w-full flex items-center gap-3 px-4 py-3 text-left  text-sm text-white/80 hover:text-white transition-colors duration-150"
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = `${accentColor.value}15`;
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'transparent';
              }}
            >
              <Icon
                icon={(item.updates_enabled ?? true) ? "solar:close-circle-bold" : "solar:check-circle-bold"}
                className="w-4 h-4 flex-shrink-0 text-white/70"
              />
              <span className="flex-1">
                {(item.updates_enabled ?? true) ? "Disable Check" : "Enable Check"}
              </span>
            </button>
            )}
          </div>
        </div>
      );

      return (
        <GenericDetailListItem
          key={item.filename}
          id={item.filename}
          isSelected={selectedItemIds.has(item.filename)}
          onSelectionChange={(checked) =>
            handleItemSelectionChange(item.filename, checked)
          }
          iconNode={itemIconNode}
          title={itemTitle}
          descriptionNode={itemDescriptionNode}
          infoItems={itemBadgesNode}
          isDisabled={item.is_disabled}
          actionsNode={itemActionsNode}
          dropdownNode={itemDropdownNode}
          isDropdownVisible={activeDropdownId === item.filename}
          accentColor={accentColor.value}
          onTitleClick={() => handleItemTitleClick(item)}
        />
      );
    },
    [
      accentColor.value,
      getDisplayFileName,
      handleToggleItemEnabled,
      itemBeingToggled,
      itemBeingDeleted,
      handleDeleteItem,
      handleOpenItemFolder,
      profile,
      selectedItemIds,
      handleItemSelectionChange,
      isBatchToggling,
      isBatchDeleting,
      isCheckingUpdates,
      itemsBeingUpdated,
      contentUpdates,
      activeDropdownId,
      setActiveDropdownId,
      dropdownRef,
      handleUpdateContentItem,
      modrinthIcons,
      localArchiveIcons,
      getItemIcon,
      getItemPlatformDisplayName,
      isUpdatingAll,
      isAnyTaskRunning,
      isLoading, // Added for item-specific loading states
      isFetchingHashes,
      isFetchingModrinthDetails,
      itemTypeName,
      emptyStateIconOverride,
      openVersionDropdownId,
      setOpenVersionDropdownId,
      versionButtonRef,
      availableVersions,
      isLoadingVersions,
      versionsError,
      handleSwitchContentVersion,
      isBlockedConfigLoaded,
      handleItemTitleClick,
    ],
  );

  const isBusyWithEssentialLoad = isLoading && items.length === 0;
  const isAnyBatchActionInProgress =
    isBatchToggling || isBatchDeleting || isUpdatingAll;

  // Helper function to determine the appropriate updates toggle action
  const getUpdatesToggleConfig = useCallback(() => {
    if (selectedItemIds.size === 0) return null;

    const selectedItems = items.filter(item => selectedItemIds.has(item.filename));
    const validItems = selectedItems.filter(item => item.id);

    if (validItems.length === 0) return null;

    // Count how many items have updates enabled vs disabled
    const enabledCount = validItems.filter(item => item.updates_enabled ?? true).length;
    const disabledCount = validItems.filter(item => !(item.updates_enabled ?? true)).length;

    // If more items have updates enabled, suggest disabling
    // If more items have updates disabled, suggest enabling
    // If equal or mixed, suggest enabling (more common use case)
    const shouldEnable = disabledCount >= enabledCount;

    return {
      shouldEnable,
      actionCount: validItems.length,
      enabledCount,
      disabledCount,
    };
  }, [selectedItemIds, items]);

  // Get the updates toggle configuration
  const updatesToggleConfig = getUpdatesToggleConfig();

  const primaryLeftActionsContent = (
    <div className="flex flex-col flex-grow min-w-0">
      <div className="flex items-center gap-2">
        <SearchWithFilters
          searchValue={searchQuery}
          onSearchChange={setSearchQuery}
          placeholder={`Search ${itemTypeNamePlural}...`}
          showSort={false}
          showFilter={false}
          className="!h-8 flex-grow"
        />
      </div>
      {contentUpdateError && (
        <div className="text-xs text-red-400 p-1 bg-red-900/30 border border-red-700/50 rounded">
          Update Check Error: {contentUpdateError}
        </div>
      )}
      <>
        <div className="flex items-center justify-between w-full min-h-14">
          {/* Left side: Select All Checkbox */}
          <CheckboxV2
            size="md"
            checked={areAllFilteredSelected}
            onChange={(checked) => handleSelectAllToggle(checked)}
            label={
              selectedItemIds.size > 0
                ? `${selectedItemIds.size} selected`
                : "Select All"
            }
            tooltip={
              areAllFilteredSelected
                ? "Deselect all visible"
                : "Select all visible"
            }
          />

          {/* Right side: Action Buttons and NoRiskPack Dropdown */}
          <div className="flex items-center gap-2">
            {/* Batch Actions - Always visible when items are selected */}
            {selectedItemIds.size > 0 && (
              <ContentActionButtons
                actions={[
                  {
                    id: "batch-toggle",
                    label: isBatchToggling ? "TOGGLING..." : `TOGGLE (${selectedItemIds.size})`,
                    icon: isBatchToggling ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11] : "solar:refresh-bold",
                    variant: "text" as const,
                    disabled: isBatchToggling,
                    loading: isBatchToggling,
                    tooltip: `Toggle enable/disable for ${selectedItemIds.size} selected items`,
                    onClick: handleBatchToggleSelected,
                  },
                  // Smart updates toggle button - only show if we have valid items
                  ...(updatesToggleConfig ? [{
                    id: "batch-toggle-updates",
                    label: updatesToggleConfig.shouldEnable
                      ? `ENABLE CHECK (${updatesToggleConfig.actionCount})`
                      : `DISABLE CHECK (${updatesToggleConfig.actionCount})`,
                    icon: updatesToggleConfig.shouldEnable
                      ? "solar:check-circle-bold"
                      : "solar:close-circle-bold",
                    variant: "text" as const,
                    tooltip: updatesToggleConfig.shouldEnable
                      ? `Enable update checks for ${updatesToggleConfig.actionCount} selected items`
                      : `Disable update checks for ${updatesToggleConfig.actionCount} selected items`,
                    onClick: () => handleBatchToggleSelectedUpdatesEnabled(updatesToggleConfig.shouldEnable),
                  }] : []),
                  ...(contentType !== "NoRiskMod" ? [{
                    id: "batch-delete",
                    label: isBatchDeleting ? "DELETING..." : `DELETE (${selectedItemIds.size})`,
                    icon: isBatchDeleting ? LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[11] : LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[6],
                    variant: "text" as const,
                    disabled: isBatchDeleting,
                    loading: isBatchDeleting,
                    tooltip: `Delete ${selectedItemIds.size} selected items`,
                    onClick: handleBatchDeleteSelected,
                  }] : []),
                ]}
                size="sm"
              />
            )}

            {/* Hide other buttons when any items are selected */}
            {selectedItemIds.size === 0 && (
              <>
                {updateCheckButtonAction ? (
                  <ContentActionButtons actions={[updateCheckButtonAction]} size="sm" />
                ) : null}

                {/* Browse and Add buttons - only for non-NoRiskMod types */}
                {effectiveOnAddContent && contentType !== "NoRiskMod" && profile && (
                  <ContentActionButtons
                    actions={[
                          isInstalledModpack && contentType === "Mod"
                        ? {
                            id: "change-modpack-version",
                            label: "CHANGE MODPACK VERSION",
                            icon: "solar:archive-bold",
                            variant: "highlight" as const,
                            tooltip: "Switch this modpack to a different version",
                            disabled: !onChangeModpackVersion,
                            onClick: () => onChangeModpackVersion?.(),
                          }
                        : {
                            id: "browse",
                            label: `DOWNLOAD ${itemTypeNamePlural.toUpperCase()}`,
                            icon: "solar:add-circle-bold",
                            variant: "highlight" as const,
                            tooltip: `Browse and download ${itemTypeNamePlural} online`,
                            onClick: () => {
                              if (profile && onBrowseContentRequest) {
                                const browseContentType = getBrowseTabContentType(contentType);
                                onBrowseContentRequest(browseContentType);
                              } else if (profile) {
                                const browseContentType = getBrowseTabContentType(contentType);
                                navigate(`/profiles/${profile.id}/browse/${browseContentType}`);
                              }
                            },
                          },
                      {
                        id: "add",
                        label: "IMPORT",
                        icon: "solar:folder-with-files-bold",
                        variant: "text" as const,
                        tooltip: addContentButtonText,
                        onClick: effectiveOnAddContent,
                      },
                    ]}
                    size="sm"
                  />
                )}

                {/* Refresh button - visible when no items are selected */}
                <ContentActionButtons
                  actions={[
                    {
                      id: "refresh",
                      label: "REFRESH",
                      icon: LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[13],
                      variant: "text" as const,
                      tooltip: "Refresh List",
                      onClick: () => fetchData(true),
                    },
                  ]}
                  size="sm"
                />
              </>
            )}
          </div>
        </div>
      </>
    </div>
  );

  const primaryRightActionsContent = null;

  if (!profile) {
    return (
      <div className="p-4  text-center text-white/70">
        Profile data is not available. Cannot display{" "}
        {itemTypeNamePlural.toLowerCase()}.
      </div>
    );
  }

  // Determine if the special empty state for standard profiles should be shown
  const shouldShowStandardProfileEmptyState = false;

  if (shouldShowStandardProfileEmptyState) {
    const handleCloneProfile = async () => {
      if (!profile) return;

      try {
        const newName = await confirm({
          title: "Clone Profile",
          inputLabel: "New profile name",
          inputPlaceholder: "Enter a name for the cloned profile",
          inputInitialValue: `${profile.name} (Copy)`,
          inputRequired: true,
          inputMaxLength: 35,
          confirmText: "CLONE",
          type: "input",
          fullscreen: true, // Or false, depending on desired dialog style
        });

        if (newName && typeof newName === "string") {
          const clonePromise = copyProfile(
            profile.id,
            newName,
            undefined,
            true,
          );

          toast.promise(clonePromise, {
            loading: `Cloning profile '${profile.name}' as '${newName}'...`,
            success: (newProfileId) => {
              // Immediately attempt to update the group after cloning is successful
              updateProfile(newProfileId, { group: "CUSTOM" })
                .then(() => {
                  //toast.success(`Profile '${newName}' set to group CUSTOM.`);
                  // Refresh data after group update as well if needed, or rely on fetchProfiles below
                })
                .catch((updateError) => {
                  console.error(
                    "Failed to update group for cloned profile:",
                    updateError,
                  );
                  //toast.error(`Failed to set group for '${newName}'.`);
                });

              void fetchProfiles(true);
              if (onRefreshRequired) onRefreshRequired(); // Refresh parent view if callback provided
              navigate(`/profiles/${newProfileId}`); // Navigate to the new profile's detail view
              return `Profile '${newName}' cloned successfully!`; // Toast for cloning success
            },
            error: (err) =>
              `Failed to clone profile: ${err instanceof Error ? err.message : String(err.message)}`,
          });
        }
      } catch (err) {
        // This catch block is for errors from the confirm dialog itself (e.g., user cancelled)
        // If it's a cancel, we don't need to show an error toast.
        if (err !== "cancel") {
          // Check if it's not a cancellation
          console.error("Error in clone setup or dialog: ", err);
          toast.error("Could not initiate cloning process.");
        }
      }
    };

    const cloneButton = (
      <Button
        variant="default"
        size="md"
        onClick={handleCloneProfile} // Updated onClick
        icon={<Icon icon="solar:copy-bold-duotone" className="mr-2" />}
      >
        CLONE PROFILE
      </Button>
    );

    return (
      <>
        <div className="flex h-full flex-col items-center justify-center">
          <EmptyState
            icon="solar:shield-warning-bold-duotone"
            title="Standard profiles are read-only"
            description="Clone to make changes and manage content."
          />
          <div className="mt-4">{cloneButton}</div>
        </div>
        {confirmDialog}
      </>
    );
  }

  const hasSelectedItems = selectedItemIds.size > 0;

  // Dynamic empty state messages
  const getEmptyStateMessage = () => {
    if (error) {
      return "Could not load content";
    } else if (isLoading && items.length === 0) {
      return `Loading ${itemTypeNamePlural}...`;
    } else if (
      !searchQuery &&
      items.length === 0 &&
      selectedItemIds.size === 0
    ) {
      return `No ${itemTypeNamePlural} yet`;
    } else if (
      searchQuery &&
      filteredItems.length === 0 &&
      selectedItemIds.size === 0
    ) {
      return "No results found";
    } else {
      return `Manage your ${itemTypeNamePlural}`;
    }
  };

  const getEmptyStateDescription = () => {
    if (error) {
      return "Please try refreshing or check the console.";
    } else if (isLoading && items.length === 0) {
      return "Please wait while content is being loaded.";
    } else if (
      !searchQuery &&
      items.length === 0 &&
      selectedItemIds.size === 0
    ) {
      return `Drag and drop ${itemTypeNamePlural} here, or browse online to install.`;
    } else if (
      searchQuery &&
      filteredItems.length === 0 &&
      selectedItemIds.size === 0
    ) {
      return "Try a different search term or clear the search filter.";
    } else {
      return `Select ${itemTypeNamePlural} to perform batch actions or manage them individually.`;
    }
  };

  const isTrulyEmptyState =
    !error && !searchQuery && items.length === 0 && selectedItemIds.size === 0;

  const handleEmptyStateBrowse = useCallback(() => {
    if (!profile) return;
    if (isInstalledModpack && contentType === "Mod") {
      onChangeModpackVersion?.();
      return;
    }
    const browseType = ((ct: typeof contentType) => {
      switch (ct) {
        case "Mod":
          return "mods";
        case "ResourcePack":
          return "resourcepacks";
        case "ShaderPack":
          return "shaderpacks";
        case "DataPack":
          return "datapacks";
        default:
          return "mods";
      }
    })(contentType);
    if (onBrowseContentRequest) onBrowseContentRequest(browseType);
    else navigate(`/profiles/${profile.id}/browse/${browseType}`);
  }, [onBrowseContentRequest, onChangeModpackVersion, isInstalledModpack, navigate, profile, contentType]);

  return (
    <>
      <GenericContentTab<T>
        items={filteredItems}
        renderListItem={renderListItem}
        isLoading={isBusyWithEssentialLoad}
        error={error}
        searchQuery={searchQuery}
        primaryLeftActions={primaryLeftActionsContent}
        primaryRightActions={primaryRightActionsContent}
        emptyStateIcon={
          emptyStateIconOverride || LOCAL_CONTENT_TAB_ICONS_TO_PRELOAD[0]
        }
        emptyStateMessage={getEmptyStateMessage()}
        emptyStateDescription={getEmptyStateDescription()}
        emptyStateAction={
          (isTrulyEmptyState ||
           (searchQuery && filteredItems.length === 0 && selectedItemIds.size === 0) ||
           (error && !isLoading)) ? (
            <UiV2Button
              variant="primary"
              size="md"
              onClick={handleEmptyStateBrowse}
              icon={<Icon icon={isInstalledModpack && contentType === "Mod" ? "solar:archive-bold" : "solar:add-circle-bold"} className="h-4 w-4" />}
            >
              {isInstalledModpack && contentType === "Mod"
                ? "Change Modpack Version"
                : `Browse ${itemTypeNamePlural}`}
            </UiV2Button>
          ) : undefined
        }
        loadingItemCount={Math.min(items.length > 0 ? items.length : 5, 10)}
        showSkeletons={false}
        accentColorOverride={accentColor.value}
      />

      <ConfirmDeleteDialog
        isOpen={isConfirmDeleteDialogOpen}
        itemName={
          itemToDeleteForDialog
            ? getDisplayFileName(itemToDeleteForDialog)
            : `${selectedItemIds.size} ${itemTypeName}${selectedItemIds.size === 1 ? "" : "s"}`
        }
        onClose={handleCloseDeleteDialog}
        onConfirm={handleConfirmDeletion}
        isDeleting={isDialogActionLoading}
        title={
          itemToDeleteForDialog
            ? `Delete ${getDisplayFileName(itemToDeleteForDialog)}?`
            : `Delete Selected ${itemTypeNamePlural}?`
        }
      />
    </>
  );
}
