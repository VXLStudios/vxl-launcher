"use client";

import { Icon } from "@iconify/react";
import { cn } from "../../../lib/utils";
import { ColorPicker } from "../../ColorPicker";
import { ColorPickerModal } from "../../modals/ColorPickerModal";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { SettingsSlider } from "../../ui/settings/SettingsSlider";
import { SettingsSection } from "../../ui/settings/SettingsSection";
import { SettingRow } from "../../ui/settings/SettingRow";
import { useThemeStore } from "../../../store/useThemeStore";
import { useGlobalModal } from "../../../hooks/useGlobalModal";
import { useSettingsConfig, useSettingsKeywords } from "./settings-context";
import { useStreamerModeStore } from "../../../store/streamer-mode-store";

const COLOR_PICKER_MODAL_ID = "color-picker-modal";
const NESTED_MODAL_Z_INDEX = 1100;

export function GeneralTab() {
  const kw = useSettingsKeywords();
  const { tempConfig, setTempConfig, saving } = useSettingsConfig();
  const { accentColor, setBorderRadius, borderRadius } = useThemeStore();
  const { showModal, hideModal } = useGlobalModal();
  const setStreamerMode = useStreamerModeStore((state) => state.setEnabled);

  const canShowExperimental = false;

  const handleConcurrentDownloadsChange = (value: number) => {
    if (tempConfig) setTempConfig({ ...tempConfig, concurrent_downloads: value });
  };

  const handleConcurrentIoLimitChange = (value: number) => {
    if (tempConfig) setTempConfig({ ...tempConfig, concurrent_io_limit: value });
  };

  return (
    <div className="space-y-6">
      <SettingsSection
        id="settings-section-accent"
        title="Accent Color"
        icon="solar:palette-bold"
        keywords={kw("accent", "color", "colour", "theme")}
        description="Choose your preferred accent color for the launcher"
      >
        <div className="flex items-center gap-6 py-3">
          <div className="flex-1">
            <ColorPicker shape="square" size="md" showCustomOption={false} />
          </div>

          <button
            type="button"
            onClick={() => {
              showModal(
                COLOR_PICKER_MODAL_ID,
                <ColorPickerModal
                  onClose={() => hideModal(COLOR_PICKER_MODAL_ID)}
                />,
                NESTED_MODAL_Z_INDEX,
              );
            }}
            className={cn(
              "group flex items-center gap-3 px-4 py-3 rounded-lg border-2 border-dashed border-[#ffffff30]",
              "hover:border-[#ffffff50] cursor-pointer transition-all duration-200",
            )}
            title="Click to open advanced color picker"
          >
            <div
              className="w-8 h-8 rounded-md border-2 border-white/20 shadow-lg group-hover:scale-105 transition-transform"
              style={{ backgroundColor: accentColor.value }}
            />
            <div className="flex flex-col items-start">
              <span className="text-base text-white/80 group-hover:text-white transition-colors">
                Custom
              </span>
              <span className="text-xs text-white/60">{accentColor.value}</span>
            </div>
            <Icon
              icon="solar:palette-bold"
              className="w-5 h-5 text-white/60 group-hover:text-white transition-colors"
            />
          </button>
        </div>
      </SettingsSection>

      <SettingsSection
        id="settings-section-behaviour"
        title="Behaviour"
        icon="solar:tuning-2-bold"
        keywords={kw("behaviour", "behavior", "updates", "discord", "logs")}
      >
        <SettingRow
          label="Auto Updates"
          description="Automatically check for and download launcher updates when available."
          searchKeywords={kw("auto updates", "update", "updates")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.auto_check_updates || false}
            onChange={(checked) =>
              tempConfig && setTempConfig({ ...tempConfig, auto_check_updates: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
        <SettingRow
          label="Discord Presence"
          description="Show your current game and launcher status in Discord."
          searchKeywords={kw("discord", "presence", "status")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.enable_discord_presence || false}
            onChange={(checked) =>
              tempConfig &&
              setTempConfig({ ...tempConfig, enable_discord_presence: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
        <SettingRow
          label="Beta Updates"
          description="Receive beta versions and pre-release updates."
          searchKeywords={kw("beta", "updates", "channel")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.check_beta_channel || false}
            onChange={(checked) =>
              tempConfig && setTempConfig({ ...tempConfig, check_beta_channel: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
        {canShowExperimental && (
          <SettingRow
            label="Experimental Mode"
            description="Enable experimental features and unstable functionality."
            searchKeywords={kw("experimental", "beta")}
            disabled={saving}
          >
            <ToggleSwitch
              checked={tempConfig?.is_experimental || false}
              onChange={(checked) =>
                tempConfig && setTempConfig({ ...tempConfig, is_experimental: checked })
              }
              disabled={saving}
              size="md"
            />
          </SettingRow>
        )}
        <SettingRow
          label="Streamer Mode"
          description="Hide Minecraft usernames and server addresses. Account names stay blurred until you hover them; the play screen never shows a nametag."
          searchKeywords={kw("streamer", "stream", "privacy", "hide username", "ip")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.streamer_mode || false}
            onChange={(checked) => {
              if (!tempConfig) return;
              setStreamerMode(checked);
              setTempConfig({ ...tempConfig, streamer_mode: checked });
            }}
            disabled={saving}
            size="md"
          />
        </SettingRow>
        <SettingRow
          label="Log level"
          description="Launcher log verbosity. VXL_LOG_LEVEL in the environment overrides this."
          searchKeywords={kw("log", "debug", "trace", "verbosity")}
          disabled={saving}
        >
          <select
            className="rounded-md border border-white/10 bg-white/5 px-3 py-2 text-sm text-white"
            value={tempConfig?.log_level ?? "debug"}
            disabled={saving}
            onChange={(e) =>
              tempConfig &&
              setTempConfig({
                ...tempConfig,
                log_level: e.target.value as import("../../../types/launcherConfig").LogLevel,
              })
            }
          >
            {(["error", "warn", "info", "debug", "trace"] as const).map((level) => (
              <option key={level} value={level}>
                {level}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          label="Open Logs After Starting"
          description="Automatically open the game logs window when launching Minecraft."
          searchKeywords={kw("logs", "log", "open logs")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.open_logs_after_starting || false}
            onChange={(checked) =>
              tempConfig &&
              setTempConfig({ ...tempConfig, open_logs_after_starting: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
        <SettingRow
          label="Hide Window on Launch"
          description="Automatically hide the launcher window when Minecraft starts."
          searchKeywords={kw("hide", "window", "launch", "minimize")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.hide_on_process_start || false}
            onChange={(checked) =>
              tempConfig &&
              setTempConfig({ ...tempConfig, hide_on_process_start: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
        <SettingRow
          label="Multiple Log Windows"
          description="Allow multiple log windows to be open simultaneously."
          searchKeywords={kw("multiple", "log", "windows")}
          disabled={saving}
        >
          <ToggleSwitch
            checked={tempConfig?.multiple_log_windows || false}
            onChange={(checked) =>
              tempConfig &&
              setTempConfig({ ...tempConfig, multiple_log_windows: checked })
            }
            disabled={saving}
            size="md"
          />
        </SettingRow>
      </SettingsSection>

      <SettingsSection
        id="settings-section-interface"
        title="Interface"
        icon="solar:slider-horizontal-bold"
        keywords={kw("interface", "downloads", "border", "radius", "performance")}
      >
        <SettingRow
          label="Concurrent Downloads"
          description="Maximum number of files downloaded simultaneously."
          searchKeywords={kw("concurrent", "downloads", "download")}
          disabled={saving}
        >
          <SettingsSlider
            value={tempConfig?.concurrent_downloads || 3}
            onChange={handleConcurrentDownloadsChange}
            min={1}
            max={10}
            step={1}
            disabled={saving}
          />
        </SettingRow>
        <SettingRow
          label="Concurrent I/O Operations"
          description="Maximum number of files written to disk simultaneously."
          searchKeywords={kw("concurrent", "io", "disk", "operations")}
          disabled={saving}
        >
          <SettingsSlider
            value={tempConfig?.concurrent_io_limit || 10}
            onChange={handleConcurrentIoLimitChange}
            min={1}
            max={20}
            step={1}
            disabled={saving}
          />
        </SettingRow>
        <SettingRow
          label="Border Radius"
          description="Adjust the corner roundness of all UI elements."
          searchKeywords={kw("border", "radius", "corners", "rounding")}
          disabled={saving}
        >
          <SettingsSlider
            value={borderRadius}
            onChange={setBorderRadius}
            min={0}
            max={20}
            step={1}
            unit="px"
            disabled={saving}
          />
        </SettingRow>
      </SettingsSection>
    </div>
  );
}
