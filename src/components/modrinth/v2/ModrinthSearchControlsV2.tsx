"use client";

import React from "react";
import type {
  ModrinthProjectType,
} from "../../../types/modrinth";
import { UnifiedSortType, ModPlatform } from "../../../types/unified";
import { SearchWithFilters } from "../../ui/SearchWithFilters";
import { SelectTab } from "../../ui-v2/SelectTab";
import { TagBadge } from "../../ui/TagBadge";
import { Icon } from "@iconify/react";
import { cn } from "../../../lib/utils";
import { CustomDropdown } from "../../ui/CustomDropdown";

// SVG Components converted to data URLs
const modrinthSvgDataUrl = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath fill='%2322c55e' d='M12.252.004a11.78 11.768 0 0 0-8.92 3.73a11 10.999 0 0 0-2.17 3.11a11.37 11.359 0 0 0-1.16 5.169c0 1.42.17 2.5.6 3.77c.24.759.77 1.899 1.17 2.529a12.3 12.298 0 0 0 8.85 5.639c.44.05 2.54.07 2.76.02c.2-.04.22.1-.26-1.7l-.36-1.37l-1.01-.06a8.5 8.489 0 0 1-5.18-1.8a5.34 5.34 0 0 1-1.3-1.26c0-.05.34-.28.74-.5a37.572 37.545 0 0 1 2.88-1.629c.03 0 .5.45 1.06.98l1 .97l2.07-.43l2.06-.43l1.47-1.47c.8-.8 1.48-1.5 1.48-1.52c0-.09-.42-1.63-.46-1.7c-.04-.06-.2-.03-1.02.18c-.53.13-1.2.3-1.45.4l-.48.15l-.53.53l-.53.53l-.93.1l-.93.07l-.52-.5a2.7 2.7 0 0 1-.96-1.7l-.13-.6l.43-.57c.68-.9.68-.9 1.46-1.1c.4-.1.65-.2.83-.33c.13-.099.65-.579 1.14-1.069l.9-.9l-.7-.7l-.7-.7l-1.95.54c-1.07.3-1.96.53-1.97.53c-.03 0-2.23 2.48-2.63 2.97l-.29.35l.28 1.03c.16.56.3 1.16.31 1.34l.03.3l-.34.23c-.37.23-2.22 1.3-2.84 1.63c-.36.2-.37.2-.44.1c-.08-.1-.23-.6-.32-1.03c-.18-.86-.17-2.75.02-3.73a8.84 8.839 0 0 1 7.9-6.93c.43-.03.77-.08.78-.1c.06-.17.5-2.999.47-3.039c-.01-.02-.1-.02-.2-.03Zm3.68.67c-.2 0-.3.1-.37.38c-.06.23-.46 2.42-.46 2.52c0 .04.1.11.22.16a8.51 8.499 0 0 1 2.99 2a8.38 8.379 0 0 1 2.16 3.449a6.9 6.9 0 0 1 .4 2.8c0 1.07 0 1.27-.1 1.73a9.37 9.369 0 0 1-1.76 3.769c-.32.4-.98 1.06-1.37 1.38c-.38.32-1.54 1.1-1.7 1.14c-.1.03-.1.06-.07.26c.03.18.64 2.56.7 2.78l.06.06a12.07 12.058 0 0 0 7.27-9.4c.13-.77.13-2.58 0-3.4a11.96 11.948 0 0 0-5.73-8.578c-.7-.42-2.05-1.06-2.25-1.06Z'%3E%3C/path%3E%3C/svg%3E";

const curseforgeSvgDataUrl = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath fill='%23f97316' d='M18.326 9.214s4.9-.772 5.674-3.026h-7.507V4.4H0l2.032 2.358v2.415s5.127-.267 7.11 1.237c2.714 2.516-3.053 5.917-3.053 5.917l-.99 3.273c1.547-1.473 4.494-3.377 9.899-3.286c-2.057.65-4.125 1.665-5.735 3.286h10.925l-1.029-3.273s-7.918-4.668-.833-7.112z'%3E%3C/path%3E%3C/svg%3E";

// Define Profile type locally, similar to ModrinthSearchV2.tsx
type Profile = any;

export // Define SelectOption type locally
interface SelectOption {
  value: UnifiedSortType;
  label: string;
  icon?: string;
}

interface ModrinthSearchControlsV2Props {
  searchTerm: string;
  onSearchTermChange: (term: string) => void;
  projectType: ModrinthProjectType;
  onProjectTypeChange: (type: ModrinthProjectType) => void;
  allProjectTypes: ModrinthProjectType[]; // This will be ALL_MODRINTH_PROJECT_TYPES from parent
  availableProjectTypes?: ModrinthProjectType[]; // Project types that have content (optional, defaults to allProjectTypes)
  profiles: Profile[];
  selectedProfile: Profile | null;
  onSelectedProfileChange: (profile: Profile | null) => void;
  sortOrder: UnifiedSortType;
  onSortOrderChange: (sort: UnifiedSortType) => void;
  sortOptions: SelectOption[];
  isSidebarVisible: boolean;
  onToggleSidebar: () => void;
  selectedGameVersions: string[];
  currentSelectedLoaders: string[];
  currentSelectedCategories: string[];
  filterClientRequired: boolean;
  filterServerRequired: boolean;
  onRemoveGameVersionTag: (version: string) => void;
  onRemoveLoaderTag: (loader: string) => void;
  onRemoveCategoryTag: (category: string) => void;
  onRemoveClientRequiredTag: () => void;
  onRemoveServerRequiredTag: () => void;
  onClearAllFilters: () => void;
  overrideDisplayContext?: "detail" | "standalone";
  modSource: ModPlatform;
  onModSourceChange: (source: ModPlatform) => void;
}

export const ModrinthSearchControlsV2: React.FC<
  ModrinthSearchControlsV2Props
> = ({
  searchTerm,
  onSearchTermChange,
  projectType,
  onProjectTypeChange,
  allProjectTypes,
  availableProjectTypes,
  profiles,
  selectedProfile,
  onSelectedProfileChange,
  sortOrder,
  onSortOrderChange,
  sortOptions,
  isSidebarVisible,
  onToggleSidebar,
  selectedGameVersions,
  currentSelectedLoaders,
  currentSelectedCategories,
  filterClientRequired,
  filterServerRequired,
  onRemoveGameVersionTag,
  onRemoveLoaderTag,
  onRemoveCategoryTag,
  onRemoveClientRequiredTag,
  onRemoveServerRequiredTag,
  onClearAllFilters,
  modSource,
  onModSourceChange,
}) => {
  // Calculate total number of active filters
  const totalFilters =
    selectedGameVersions.length +
    currentSelectedLoaders.length +
    currentSelectedCategories.length +
    (filterClientRequired ? 1 : 0) +
    (filterServerRequired ? 1 : 0);

  // Create groups array for project types - only show available types
  const typesToShow = availableProjectTypes || allProjectTypes;
  const groups = typesToShow.map(type => ({
    id: type,
    name: type.charAt(0).toUpperCase() + type.slice(1) + 's',
  }));

  return (
    <>
      <div className="flex items-center gap-2 flex-wrap mb-4">
        {groups.map((group) => (
          <SelectTab
            key={group.id}
            active={projectType === group.id}
            onClick={() => onProjectTypeChange(group.id as ModrinthProjectType)}
          >
            {group.name}
          </SelectTab>
        ))}
      </div>

      {/* Search & Filter Header */}
      <div className="mb-4">
        <div className="flex h-9 items-center gap-2">
          <SearchWithFilters
            placeholder={`Search ${projectType}s...`}
            searchValue={searchTerm}
            onSearchChange={onSearchTermChange}
            showSort={false}
            onFilterToggle={onToggleSidebar}
            isFilterActive={isSidebarVisible}
            filterBadgeCount={totalFilters}
          />

          <CustomDropdown
            value={sortOrder}
            onChange={(value) => onSortOrderChange(value as UnifiedSortType)}
            options={sortOptions}
            className="h-9 w-auto shrink-0"
            variant="search"
          />

          <div className="flex h-9 shrink-0 items-stretch gap-0.5 rounded-lg border border-[var(--surface-border)] bg-[var(--surface-overlay)] p-0.5">
            <button
              onClick={() => {
                onModSourceChange(ModPlatform.Modrinth);
                onClearAllFilters();
              }}
              className={cn(
                "flex h-full items-center gap-1.5 rounded-md px-3 text-sm font-medium transition-all duration-200",
                modSource === ModPlatform.Modrinth
                  ? "bg-[#1bd96a]/20 text-white border border-[#1bd96a]/40"
                  : "text-[var(--text-secondary)] hover:text-white hover:bg-[var(--surface-base)] border border-transparent",
              )}
              title="Search Modrinth"
            >
              <img
                src={modrinthSvgDataUrl}
                alt="Modrinth"
                className="h-4 w-4 object-contain"
              />
              <span className="hidden sm:inline">Modrinth</span>
            </button>

            <button
              onClick={() => {
                onModSourceChange(ModPlatform.CurseForge);
                onClearAllFilters();
              }}
              className={cn(
                "flex h-full items-center gap-1.5 rounded-md px-3 text-sm font-medium transition-all duration-200",
                modSource === ModPlatform.CurseForge
                  ? "bg-orange-500/20 text-white border border-orange-400/30"
                  : "text-[var(--text-secondary)] hover:text-white hover:bg-[var(--surface-base)] border border-transparent",
              )}
              title="Search CurseForge"
            >
              <img
                src={curseforgeSvgDataUrl}
                alt="CurseForge"
                className="h-4 w-4 object-contain"
              />
              <span className="hidden sm:inline">CurseForge</span>
            </button>
          </div>
        </div>

        {/* Filter Tags - Under Search */}
        {totalFilters > 0 && (
          <div className="flex items-center gap-2 mt-4">
            <TagBadge
              variant="destructive"
              className="cursor-pointer hover:brightness-110 transition-all flex-shrink-0 flex items-center"
              onClick={onClearAllFilters}
              size="md"
            >
              <Icon
                icon="solar:trash-bin-trash-bold"
                className="w-4 h-4 mr-1"
              />
              <span>Clear All</span>
            </TagBadge>

            {selectedGameVersions.map((version) => (
              <TagBadge
                key={`gv-${version}`}
                variant="filter"
                className="inline-flex whitespace-nowrap items-center"
                size="md"
                onClick={() => onRemoveGameVersionTag(version)}
              >
                <span>{version}</span>
                <Icon
                  icon="solar:close-circle-bold"
                  className="w-4 h-4 ml-1"
                />
              </TagBadge>
            ))}

            {currentSelectedLoaders.map((loader) => (
              <TagBadge
                key={`loader-${loader}`}
                variant="filter"
                className="inline-flex whitespace-nowrap items-center"
                size="md"
                onClick={() => onRemoveLoaderTag(loader)}
              >
                <span>{loader}</span>
                <Icon
                  icon="solar:close-circle-bold"
                  className="w-4 h-4 ml-1"
                />
              </TagBadge>
            ))}

            {currentSelectedCategories.map((category) => (
              <TagBadge
                key={`cat-${category}`}
                variant="filter"
                className="inline-flex whitespace-nowrap items-center"
                size="md"
                onClick={() => onRemoveCategoryTag(category)}
              >
                <span>{category}</span>
                <Icon
                  icon="solar:close-circle-bold"
                  className="w-4 h-4 ml-1"
                />
              </TagBadge>
            ))}

            {filterClientRequired && (
              <TagBadge
                key="client-req"
                variant="filter"
                className="inline-flex whitespace-nowrap items-center"
                size="md"
                onClick={onRemoveClientRequiredTag}
              >
                <span>Client</span>
                <Icon
                  icon="solar:close-circle-bold"
                  className="w-4 h-4 ml-1"
                />
              </TagBadge>
            )}

            {filterServerRequired && (
              <TagBadge
                key="server-req"
                variant="filter"
                className="inline-flex whitespace-nowrap items-center"
                size="md"
                onClick={onRemoveServerRequiredTag}
              >
                <span>Server</span>
                <Icon
                  icon="solar:close-circle-bold"
                  className="w-4 h-4 ml-1"
                />
              </TagBadge>
            )}
          </div>
        )}
      </div>
    </>
  );
};
