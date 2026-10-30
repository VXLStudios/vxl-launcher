"use client";

import React, { useState } from "react";
import { cn } from "../../../lib/utils";
import { sanitizeHtml } from "../../../utils/motd-utils";
import type {
  ModrinthProjectType,
  ModrinthCategory,
  ModrinthGameVersion,
  ModrinthLoader,
} from "../../../types/modrinth";
import type { AccentColor } from "../../../store/useThemeStore";
import { Icon } from "@iconify/react";
import { CheckboxV2 } from "../../ui/CheckboxV2";
import { ModPlatform } from "../../../types/unified";
import {
  CURSEFORGE_MODPACK_CATEGORIES,
  CURSEFORGE_MOD_CATEGORIES,
  CURSEFORGE_RESOURCEPACK_CATEGORIES,
  CURSEFORGE_DATAPACK_CATEGORIES,
} from "../../../constants/curseforge-categories";

interface UIDynamicFilterGroup {
  accordionTitle: string;
  headerValue: string;
  options: ModrinthCategory[];
}

interface AccordionItemProps {
  title: string;
  children: React.ReactNode;
  defaultOpen?: boolean;
  activeCount?: number;
}

function AccordionItem({
  title,
  children,
  defaultOpen = false,
  activeCount = 0,
}: AccordionItemProps) {
  const [isOpen, setIsOpen] = useState(defaultOpen || activeCount > 0);

  return (
    <div className="border-b border-[var(--surface-border)] last:border-b-0">
      <button
        type="button"
        onClick={() => setIsOpen((open) => !open)}
        className="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left transition-colors hover:bg-[var(--surface-overlay)]/40"
      >
        <span className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-medium text-white">{title}</span>
          {activeCount > 0 && (
            <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-[rgba(var(--accent-rgb),0.15)] px-1.5 text-[10px] font-semibold text-[var(--accent)]">
              {activeCount}
            </span>
          )}
        </span>
        <Icon
          icon={isOpen ? "solar:alt-arrow-up-linear" : "solar:alt-arrow-down-linear"}
          className="h-4 w-4 flex-shrink-0 text-[var(--text-muted)]"
        />
      </button>

      {isOpen && <div className="px-3 pb-3">{children}</div>}
    </div>
  );
}

function FilterOption({
  label,
  icon,
  isSelected,
  onClick,
  modSource,
}: {
  label: string;
  icon?: React.ReactNode | string;
  isSelected: boolean;
  onClick: () => void;
  accentColor: AccentColor;
  modSource: ModPlatform;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "mb-1 flex w-full items-center justify-between rounded-md px-2 py-1.5 text-xs transition-colors",
        isSelected
          ? "border border-[var(--accent)]/30 bg-[rgba(var(--accent-rgb),0.12)] text-white"
          : "border border-transparent text-[var(--text-secondary)] hover:bg-[var(--surface-overlay)] hover:text-white",
      )}
    >
      <span className="flex min-w-0 flex-grow items-center text-left">
        {typeof icon === "string" ? (
          <span
            className="mr-1.5 h-4 w-4 flex-shrink-0"
            dangerouslySetInnerHTML={{ __html: sanitizeHtml(icon) }}
          />
        ) : icon ? (
          <span className="mr-1.5 flex-shrink-0">{icon}</span>
        ) : null}
        <span className="truncate">
          {modSource === ModPlatform.Modrinth
            ? label.charAt(0).toUpperCase() + label.slice(1)
            : label}
        </span>
      </span>
      {isSelected && (
        <Icon icon="solar:check-circle-bold" className="ml-2 h-3.5 w-3.5 flex-shrink-0 text-[var(--accent)]" />
      )}
    </button>
  );
}

interface ModrinthFilterSidebarV2Props {
  projectType: ModrinthProjectType;
  accentColor: AccentColor;
  gameVersionSearchTerm: string;
  onGameVersionSearchTermChange: (term: string) => void;
  displayedGameVersions: ModrinthGameVersion[];
  selectedGameVersions: string[];
  onGameVersionToggle: (version: string) => void;
  showAllGameVersionsSidebar: boolean;
  onShowAllGameVersionsSidebarChange: (show: boolean) => void;
  availableLoaders: ModrinthLoader[];
  currentSelectedLoaders: string[];
  onLoaderToggle: (loaderName: string) => void;
  allLoadersData: ModrinthLoader[];
  dynamicFilterGroups: UIDynamicFilterGroup[];
  currentSelectedCategories: string[];
  onCategoryToggle: (categoryName: string) => void;
  filterClientRequired: boolean;
  onClientRequiredToggle: () => void;
  filterServerRequired: boolean;
  onServerRequiredToggle: () => void;
  modSource: ModPlatform;
}

export const ModrinthFilterSidebarV2: React.FC<ModrinthFilterSidebarV2Props> = ({
  projectType,
  accentColor,
  gameVersionSearchTerm,
  onGameVersionSearchTermChange,
  displayedGameVersions,
  selectedGameVersions,
  onGameVersionToggle,
  showAllGameVersionsSidebar,
  onShowAllGameVersionsSidebarChange,
  availableLoaders,
  currentSelectedLoaders,
  onLoaderToggle,
  allLoadersData,
  dynamicFilterGroups,
  currentSelectedCategories,
  onCategoryToggle,
  filterClientRequired,
  onClientRequiredToggle,
  filterServerRequired,
  onServerRequiredToggle,
  modSource,
}) => {
  const categoriesGroup = dynamicFilterGroups.find(
    (group) => group.headerValue.toLowerCase() === "categories",
  );

  const otherDynamicGroups = dynamicFilterGroups.filter(
    (group) => group.headerValue.toLowerCase() !== "categories",
  );

  const totalGameVersionFilters = selectedGameVersions.length;
  const totalLoaderFilters = currentSelectedLoaders.length;
  const totalEnvironmentFilters =
    (filterClientRequired ? 1 : 0) + (filterServerRequired ? 1 : 0);

  const dynamicGroupCounts = dynamicFilterGroups.reduce(
    (acc, group) => {
      acc[group.headerValue] = group.options.filter((opt) =>
        currentSelectedCategories.includes(opt.name),
      ).length;
      return acc;
    },
    {} as Record<string, number>,
  );

  const categoryActiveCount =
    (categoriesGroup && dynamicGroupCounts[categoriesGroup.headerValue]) || 0;

  const curseForgeCategories =
    projectType === "modpack"
      ? CURSEFORGE_MODPACK_CATEGORIES
      : projectType === "mod"
        ? CURSEFORGE_MOD_CATEGORIES
        : projectType === "resourcepack"
          ? CURSEFORGE_RESOURCEPACK_CATEGORIES
          : projectType === "datapack"
            ? CURSEFORGE_DATAPACK_CATEGORIES
            : [];

  return (
    <aside className="flex h-full w-56 flex-shrink-0 flex-col xl:w-60">
      <div className="flex max-h-full flex-col overflow-hidden rounded-xl border border-[var(--surface-border)] bg-[var(--surface-raised)]">
        <div className="flex items-center gap-2 border-b border-[var(--surface-border)] px-3 py-2.5">
          <Icon icon="solar:filter-bold" className="h-4 w-4 text-[var(--accent)]" />
          <span className="text-sm font-semibold text-white">Filters</span>
        </div>

        <div className="custom-scrollbar flex-1 overflow-y-auto">
          <AccordionItem
            title="Game version"
            defaultOpen={totalGameVersionFilters > 0}
            activeCount={totalGameVersionFilters}
          >
            <div className="space-y-2">
              <div className="flex items-center gap-2 rounded-lg border border-[var(--surface-border)] bg-[var(--surface-overlay)] px-3 py-2 transition-colors focus-within:border-[var(--accent)]/40">
                <Icon icon="solar:magnifer-linear" className="h-3.5 w-3.5 flex-shrink-0 text-[var(--text-muted)]" />
                <input
                  type="text"
                  placeholder="Search version..."
                  value={gameVersionSearchTerm}
                  onChange={(e) => onGameVersionSearchTermChange(e.target.value)}
                  className="min-w-0 flex-1 bg-transparent text-xs text-white outline-none placeholder:text-[var(--text-muted)]"
                />
              </div>

              <div className="max-h-64 space-y-0.5 overflow-y-auto pr-0.5">
                {displayedGameVersions.map((gv) => (
                  <FilterOption
                    key={gv.version}
                    label={`${gv.version} ${gv.version_type !== "release" ? `(${gv.version_type})` : ""}`}
                    isSelected={selectedGameVersions.includes(gv.version)}
                    onClick={() => onGameVersionToggle(gv.version)}
                    accentColor={accentColor}
                    modSource={modSource}
                  />
                ))}

                {displayedGameVersions.length === 0 && (
                  <p className="p-1 text-center text-xs italic text-[var(--text-muted)]">
                    No matching versions.
                  </p>
                )}
              </div>

              <CheckboxV2
                checked={showAllGameVersionsSidebar}
                onChange={onShowAllGameVersionsSidebarChange}
                label="Show all versions"
                size="sm"
              />
            </div>
          </AccordionItem>

          {modSource === ModPlatform.Modrinth && (
            <AccordionItem
              key={categoriesGroup?.headerValue || "categories_filter_accordion"}
              title={categoriesGroup?.accordionTitle || "Categories"}
              defaultOpen={categoryActiveCount > 0}
              activeCount={categoryActiveCount}
            >
              <div className="max-h-64 space-y-0.5 overflow-y-auto pr-0.5">
                {categoriesGroup && categoriesGroup.options.length > 0 ? (
                  categoriesGroup.options.map((cat) => (
                    <FilterOption
                      key={cat.name}
                      label={cat.name}
                      icon={cat.icon}
                      isSelected={currentSelectedCategories.includes(cat.name)}
                      onClick={() => onCategoryToggle(cat.name)}
                      accentColor={accentColor}
                      modSource={modSource}
                    />
                  ))
                ) : (
                  <p className="p-1 text-center text-xs italic text-[var(--text-muted)]">
                    No category options available.
                  </p>
                )}
              </div>
            </AccordionItem>
          )}

          {modSource === ModPlatform.CurseForge && (
            <AccordionItem
              key="curseforge-categories"
              title="Categories"
              defaultOpen={currentSelectedCategories.length > 0}
              activeCount={currentSelectedCategories.length}
            >
              <div className="max-h-64 space-y-0.5 overflow-y-auto pr-0.5">
                {curseForgeCategories.map((cat) => (
                  <FilterOption
                    key={cat.name}
                    label={cat.name}
                    isSelected={currentSelectedCategories.includes(cat.name)}
                    onClick={() => onCategoryToggle(cat.name)}
                    accentColor={accentColor}
                    modSource={modSource}
                  />
                ))}
              </div>
            </AccordionItem>
          )}

          <AccordionItem
            title="Loader"
            defaultOpen={totalLoaderFilters > 0}
            activeCount={totalLoaderFilters}
          >
            <div className="max-h-64 space-y-0.5 overflow-y-auto pr-0.5">
              {availableLoaders.map((loader) => {
                const fullLoaderData = allLoadersData.find((l) => l.name === loader.name);
                return (
                  <FilterOption
                    key={loader.name}
                    label={loader.name}
                    icon={fullLoaderData?.icon}
                    isSelected={currentSelectedLoaders.includes(loader.name)}
                    onClick={() => onLoaderToggle(loader.name)}
                    accentColor={accentColor}
                    modSource={modSource}
                  />
                );
              })}
              {availableLoaders.length === 0 && (
                <p className="p-1 text-center text-xs italic text-[var(--text-muted)]">
                  No loaders for {projectType}.
                </p>
              )}
            </div>
          </AccordionItem>

          {otherDynamicGroups.map((group) => (
            <AccordionItem
              key={group.headerValue}
              title={group.accordionTitle}
              defaultOpen={dynamicGroupCounts[group.headerValue] > 0}
              activeCount={dynamicGroupCounts[group.headerValue]}
            >
              <div className="max-h-64 space-y-0.5 overflow-y-auto pr-0.5">
                {group.options.length > 0 ? (
                  group.options.map((cat) => (
                    <FilterOption
                      key={cat.name}
                      label={cat.name}
                      icon={cat.icon}
                      isSelected={currentSelectedCategories.includes(cat.name)}
                      onClick={() => onCategoryToggle(cat.name)}
                      accentColor={accentColor}
                      modSource={modSource}
                    />
                  ))
                ) : (
                  <p className="p-1 text-center text-xs italic text-[var(--text-muted)]">
                    No options for {group.accordionTitle}.
                  </p>
                )}
              </div>
            </AccordionItem>
          ))}

          <AccordionItem
            title="Environment"
            defaultOpen={totalEnvironmentFilters > 0}
            activeCount={totalEnvironmentFilters}
          >
            <div className="space-y-0.5">
              <FilterOption
                label="Client"
                icon={<Icon icon="solar:monitor-bold" className="h-3.5 w-3.5" />}
                isSelected={filterClientRequired}
                onClick={onClientRequiredToggle}
                accentColor={accentColor}
                modSource={modSource}
              />
              <FilterOption
                label="Server"
                icon={<Icon icon="solar:server-bold" className="h-3.5 w-3.5" />}
                isSelected={filterServerRequired}
                onClick={onServerRequiredToggle}
                accentColor={accentColor}
                modSource={modSource}
              />
            </div>
          </AccordionItem>
        </div>
      </div>
    </aside>
  );
};
