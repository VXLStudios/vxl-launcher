"use client";

import { Fragment, useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Icon } from "@iconify/react";
import { toast } from "react-hot-toast";
import { invoke } from "@tauri-apps/api/core";
import type { LauncherConfig } from "../../types/launcherConfig";
import * as ConfigService from "../../services/launcher-config-service";
import { useThemeStore } from "../../store/useThemeStore";
import { cn } from "../../lib/utils";
import { Modal } from "../ui/Modal";
import { SearchWithFilters } from "../ui/SearchWithFilters";
import { SettingsSearchContext } from "../ui/settings/SettingsSearchContext";
import { openLauncherDirectory } from "../../services/tauri-service";
import { GeneralTab } from "./settings/GeneralTab";
import { AppearanceTab } from "./settings/AppearanceTab";
import { AdvancedTab } from "./settings/AdvancedTab";
import { SettingsConfigProvider } from "./settings/settings-context";
import { useStreamerModeStore } from "../../store/streamer-mode-store";
import { Button, LoadingState, Alert, EmptyState } from "../ui-v2";

type SettingsTabId = "general" | "appearance" | "advanced";

interface SettingsTabProps {
  onClose: () => void;
}

export function SettingsTab({ onClose }: SettingsTabProps) {
  const [config, setConfig] = useState<LauncherConfig | null>(null);
  const [tempConfig, setTempConfig] = useState<LauncherConfig | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState<boolean>(false);
  const [activeTab, setActiveTab] = useState<SettingsTabId>("general");
  const [activeSection, setActiveSection] = useState<string | null>(null);
  const [sidebarSearch, setSidebarSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");

  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(sidebarSearch), 150);
    return () => clearTimeout(id);
  }, [sidebarSearch]);

  const sidebarQuery = debouncedSearch.trim().toLowerCase();

  useEffect(() => {
    void invoke("set_discord_state_tinkering").catch((err) => {
      console.error("[SettingsTab] Failed to update Discord state:", err);
    });
  }, []);

  const sectionDefs: Record<SettingsTabId, { id: string; label: string }[]> = {
    general: [
      { id: "accent", label: "Accent Color" },
      { id: "behaviour", label: "Behaviour" },
      { id: "interface", label: "Interface" },
    ],
    appearance: [
      { id: "background", label: "Background Effect" },
      { id: "custom-background", label: "Custom Background" },
      { id: "presets", label: "Preset Backgrounds" },
    ],
    advanced: [
      { id: "login", label: "Login" },
      { id: "gamedir", label: "Game Data Directory" },
      { id: "hooks", label: "Game Hooks" },
      { id: "licenses", label: "Third-party Code" },
    ],
  };

  const tabConfig: {
    id: SettingsTabId;
    label: string;
    icon: string;
    children?: { id: string; label: string }[];
  }[] = [
    {
      id: "general",
      label: "General",
      icon: "solar:settings-bold",
      children: sectionDefs.general,
    },
    {
      id: "appearance",
      label: "Appearance",
      icon: "solar:palette-bold",
      children: sectionDefs.appearance,
    },
    {
      id: "advanced",
      label: "Advanced",
      icon: "solar:tuning-bold",
      children: sectionDefs.advanced,
    },
  ];

  const selectTab = (id: SettingsTabId) => {
    setSidebarSearch("");
    setActiveTab(id);
  };

  const contentRef = useRef<HTMLDivElement>(null);
  const sidebarListRef = useRef<HTMLDivElement>(null);
  const autoSaveTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const isResettingRef = useRef<boolean>(false);
  const spySuppressRef = useRef(false);
  const spyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const { accentColor } = useThemeStore();

  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }, [activeTab, sidebarQuery]);

  useEffect(() => {
    if (!activeSection) return;
    const el = sidebarListRef.current?.querySelector(
      `[data-section-id="${activeSection}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [activeSection]);

  const scrollToSection = (id: string) => {
    const el = document.getElementById(`settings-section-${id}`);
    if (!el) return;
    spySuppressRef.current = true;
    setActiveSection(id);
    if (spyTimeoutRef.current) clearTimeout(spyTimeoutRef.current);
    spyTimeoutRef.current = setTimeout(() => {
      spySuppressRef.current = false;
    }, 500);
    el.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  useEffect(() => {
    if (sidebarQuery) return;
    const root = contentRef.current;
    const defs = sectionDefs[activeTab];
    if (!root || !defs) {
      setActiveSection(null);
      return;
    }
    const onScroll = () => {
      if (spySuppressRef.current) return;
      const rootTop = root.getBoundingClientRect().top;
      const line = 80;
      let current = defs[0].id;
      for (const d of defs) {
        const el = document.getElementById(`settings-section-${d.id}`);
        if (!el) continue;
        if (el.getBoundingClientRect().top - rootTop <= line) current = d.id;
      }
      setActiveSection(current);
    };
    onScroll();
    root.addEventListener("scroll", onScroll, { passive: true });
    return () => root.removeEventListener("scroll", onScroll);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab, sidebarQuery, config, tempConfig]);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const loadedConfig = await ConfigService.getLauncherConfig();
      const configWithHooks = {
        ...loadedConfig,
        hooks: loadedConfig.hooks || {
          pre_launch: null,
          wrapper: null,
          post_exit: null,
        },
      };
      setConfig(configWithHooks);
      setTempConfig({ ...configWithHooks });
      useStreamerModeStore.getState().setEnabled(!!configWithHooks.streamer_mode);
    } catch (err) {
      console.error("Failed to load launcher config:", err);
      setError(err instanceof Error ? err.message : String(err));
      setConfig(null);
      setTempConfig(null);
    } finally {
      setLoading(false);
    }
  }, []);

  const autoSaveConfig = useCallback(async (configToSave: LauncherConfig) => {
    if (isResettingRef.current) return;

    if (autoSaveTimeoutRef.current) {
      clearTimeout(autoSaveTimeoutRef.current);
    }

    autoSaveTimeoutRef.current = setTimeout(async () => {
      setSaving(true);
      try {
        const updatedConfig = await ConfigService.setLauncherConfig(configToSave);
        setConfig(updatedConfig);
        useStreamerModeStore.getState().setEnabled(!!updatedConfig.streamer_mode);
        toast.success("Settings auto-saved!", {
          duration: 2000,
          position: "bottom-right",
        });
      } catch (err) {
        console.error("Failed to auto-save configuration:", err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        toast.error(`Auto-save failed: ${errorMessage}`);
      } finally {
        setSaving(false);
      }
    }, 500);
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (
      tempConfig &&
      config &&
      JSON.stringify(config) !== JSON.stringify(tempConfig)
    ) {
      autoSaveConfig(tempConfig);
    }
  }, [tempConfig, config, autoSaveConfig]);

  const renderTabContent = () => {
    if (loading) {
      return <LoadingState message="Loading settings..." />;
    }

    if (error) {
      return (
        <Alert tone="error">
          <div className="flex items-start gap-3">
            <Icon
              icon="solar:danger-triangle-bold"
              className="w-5 h-5 flex-shrink-0 mt-0.5"
            />
            <div>
              <p className="font-medium mb-1">Error loading settings</p>
              <p className="opacity-90 mb-3">{error}</p>
              <Button
                onClick={loadConfig}
                variant="secondary"
                size="sm"
                icon={<Icon icon="solar:refresh-bold" className="w-4 h-4" />}
              >
                Try again
              </Button>
            </div>
          </div>
        </Alert>
      );
    }

    if (!config || !tempConfig) {
      return (
        <EmptyState icon="solar:settings-bold" title="Could not load configuration" />
      );
    }

    const bodyOf: Partial<Record<SettingsTabId, ReactNode>> = {
      general: <GeneralTab />,
      appearance: <AppearanceTab />,
      advanced: <AdvancedTab />,
    };

    if (sidebarQuery) {
      const order: SettingsTabId[] = ["general", "appearance", "advanced"];
      const ordered = [activeTab, ...order.filter((id) => id !== activeTab)].filter(
        (id) => bodyOf[id],
      ) as SettingsTabId[];
      return (
        <div className="space-y-6">
          {ordered.map((id) => (
            <Fragment key={id}>{bodyOf[id]}</Fragment>
          ))}
        </div>
      );
    }

    return bodyOf[activeTab] ?? null;
  };

  return (
    <Modal
      title="Settings"
      titleIcon={<Icon icon="solar:settings-bold" className="w-6 h-6" />}
      onClose={onClose}
      width="xl"
      className="!max-w-6xl h-[85vh] min-h-[600px] flex flex-col"
      contentClassName="overflow-hidden"
      headerActions={
        <Button
          variant="secondary"
          size="sm"
          icon={<Icon icon="solar:folder-bold" className="w-4 h-4" />}
          onClick={async () => {
            try {
              await openLauncherDirectory();
            } catch (err) {
              console.error("Failed to open launcher directory:", err);
              toast.error(
                `Failed to open launcher directory: ${
                  err instanceof Error ? err.message : String(err)
                }`,
              );
            }
          }}
        >
          Open directory
        </Button>
      }
    >
      <div className="flex h-full p-4 gap-2 min-h-0">
        <div className="w-64 flex flex-col flex-shrink-0 min-h-0">
          <div className="px-1 pb-3">
            <SearchWithFilters
              placeholder="Search settings..."
              searchValue={sidebarSearch}
              onSearchChange={setSidebarSearch}
              showSort={false}
              showFilter={false}
              className="w-full"
            />
          </div>
          <div
            ref={sidebarListRef}
            className="space-y-0 flex-1 overflow-y-auto custom-scrollbar"
          >
            {tabConfig.map((tab) => {
              const isActive = activeTab === tab.id;
              return (
                <div key={tab.id}>
                  <button
                    type="button"
                    className={cn(
                      "w-full text-left px-3 py-2.5 rounded-lg transition-colors border-0 outline-none flex items-center gap-3",
                      isActive
                        ? "text-white"
                        : "bg-transparent text-white/60 hover:bg-white/5 hover:text-white/90",
                    )}
                    style={
                      isActive ? { backgroundColor: `${accentColor.value}26` } : undefined
                    }
                    onClick={() => selectTab(tab.id)}
                  >
                    <Icon
                      icon={tab.icon}
                      className="w-5 h-5 transition-colors duration-200"
                      style={{ color: isActive ? accentColor.value : undefined }}
                    />
                    <span
                      className={cn(
                        "text-base transition-colors duration-200",
                        isActive && "font-medium",
                      )}
                    >
                      {tab.label}
                    </span>
                  </button>

                  {isActive && !sidebarQuery && tab.children && (
                    <div className="flex flex-col mt-1 ml-5 border-l border-[var(--surface-border)]">
                      {tab.children.map((child) => {
                        const childActive = activeSection === child.id;
                        return (
                          <button
                            key={child.id}
                            type="button"
                            data-section-id={child.id}
                            className={cn(
                              "w-full text-left pl-4 pr-2 py-1.5 -ml-px border-l-2 outline-none text-sm transition-[color,border-color] duration-150",
                              childActive
                                ? "text-white"
                                : "border-transparent text-[var(--text-secondary)] hover:text-white/75",
                            )}
                            style={
                              childActive ? { borderColor: accentColor.value } : undefined
                            }
                            onClick={() => scrollToSection(child.id)}
                          >
                            {child.label}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="flex items-center flex-shrink-0">
          <div className="border-l border-[var(--surface-border)] mx-2 my-3 h-[85%]" />
        </div>

        <div className="flex-1 flex flex-col min-w-0 overflow-hidden min-h-0">
          <div
            ref={contentRef}
            className="flex-1 py-2 px-5 overflow-y-auto overflow-x-hidden custom-scrollbar min-w-0"
          >
            <SettingsConfigProvider
              value={{ config, tempConfig, setTempConfig, saving }}
            >
              <SettingsSearchContext.Provider value={sidebarQuery}>
                {renderTabContent()}
              </SettingsSearchContext.Provider>
            </SettingsConfigProvider>
          </div>
        </div>
      </div>
    </Modal>
  );
}
