"use client";

import { useState, useEffect } from "react";
import { Icon } from "@iconify/react";
import { cn } from "../../lib/utils";
import type { MinecraftAccount } from "../../types/minecraft";
import { useCrafatarAvatar } from "../../hooks/useCrafatarAvatar";
import { getAvatarUrl } from "../../lib/avatar-utils";
import { IconButton } from "../ui-v2";
import { StreamerSensitiveText } from "../ui/StreamerSensitiveText";
import { useStreamerModeStore } from "../../store/streamer-mode-store";

interface AccountSwitcherPanelProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  accounts: MinecraftAccount[];
  activeAccount: MinecraftAccount | null;
  isLoading: boolean;
  onSetActive: (accountId: string) => Promise<void>;
  onRemoveAccount: (accountId: string) => Promise<void>;
  onAddAccount: () => Promise<void>;
  mode?: "switcher" | "list";
}

function AccountAvatar({
  account,
  size = 32,
}: {
  account: MinecraftAccount | null;
  size?: number;
}) {
  const streamerMode = useStreamerModeStore((state) => state.enabled);
  const [failedSrc, setFailedSrc] = useState<string | null>(null);
  const avatarUrl = useCrafatarAvatar({
    uuid: account?.id,
    overlay: true,
  });
  const username = account?.minecraft_username || account?.username || "?";
  const remoteFallback = account?.id
    ? getAvatarUrl(account.id, { overlay: true, size: 64 })
    : null;
  const sources = [avatarUrl, remoteFallback].filter(Boolean) as string[];
  const displayUrl = sources.find((source) => source !== failedSrc) ?? null;

  useEffect(() => {
    setFailedSrc(null);
  }, [account?.id, avatarUrl]);

  return (
    <div
      className="relative flex flex-shrink-0 items-center justify-center overflow-hidden rounded-lg border border-[var(--surface-border)] bg-[var(--surface-base)]"
      style={{ width: size, height: size }}
    >
      {displayUrl ? (
        <img
          src={displayUrl}
          alt={streamerMode ? "Account avatar" : `${username}'s avatar`}
          className="h-full w-full object-cover pixelated"
          style={{ imageRendering: "pixelated" }}
          onError={() => setFailedSrc(displayUrl)}
        />
      ) : (
        <span
          className={cn(
            "text-xs text-white",
            streamerMode &&
              "blur-[5px] transition-[filter] duration-200 group-hover:blur-none",
          )}
        >
          {username.charAt(0).toUpperCase()}
        </span>
      )}
    </div>
  );
}

function AccountSwitcherRow({
  account,
  isLoading,
  onSetActive,
  onRemoveAccount,
}: {
  account: MinecraftAccount;
  isLoading: boolean;
  onSetActive: (accountId: string) => Promise<void>;
  onRemoveAccount: (accountId: string) => Promise<void>;
}) {
  const [isActivating, setIsActivating] = useState(false);
  const [isRemoving, setIsRemoving] = useState(false);
  const username = account.minecraft_username || account.username || "Unknown";
  const busy = isLoading || isActivating || isRemoving;

  const handleSelect = async () => {
    if (account.active || busy) return;
    setIsActivating(true);
    try {
      await onSetActive(account.id);
    } finally {
      setIsActivating(false);
    }
  };

  const handleRemove = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (busy) return;
    setIsRemoving(true);
    try {
      await onRemoveAccount(account.id);
    } catch {
      setIsRemoving(false);
    }
  };

  return (
    <div
      className={cn(
        "group relative flex items-center rounded-lg px-1.5 py-1",
        !account.active && "cursor-pointer",
        busy && "opacity-70",
      )}
    >
      <button
        type="button"
        onClick={() => void handleSelect()}
        disabled={account.active || busy}
        className={cn(
          "flex w-full min-w-0 items-center gap-2 py-0.5 pl-0.5 pr-0.5 text-left group-hover:pr-9",
          !account.active && !busy && "rounded-lg hover:bg-[var(--surface-base)]",
        )}
      >
        <AccountAvatar account={account} size={28} />
        <StreamerSensitiveText
          text={username}
          className={cn(
            "min-w-0 flex-1 truncate text-sm",
            account.active ? "text-[var(--accent)]" : "text-white",
          )}
        />
        {isActivating && (
          <Icon
            icon="solar:refresh-bold"
            className="h-3.5 w-3.5 flex-shrink-0 animate-spin text-[var(--text-secondary)]"
          />
        )}
      </button>

      <IconButton
        size="sm"
        onClick={handleRemove}
        disabled={busy}
        aria-label={`Remove ${username}`}
        className="absolute right-1 top-1/2 h-8 w-8 -translate-y-1/2 flex-shrink-0 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
      >
        {isRemoving ? (
          <Icon icon="solar:refresh-bold" className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <Icon icon="solar:trash-bin-trash-bold" className="h-3.5 w-3.5" />
        )}
      </IconButton>
    </div>
  );
}

function AccountSwitcherList({
  accounts,
  isLoading,
  onSetActive,
  onRemoveAccount,
}: {
  accounts: MinecraftAccount[];
  isLoading: boolean;
  onSetActive: (accountId: string) => Promise<void>;
  onRemoveAccount: (accountId: string) => Promise<void>;
}) {
  if (isLoading && accounts.length === 0) {
    return (
      <div className="flex items-center justify-center gap-2 px-2 py-5 text-sm text-[var(--text-secondary)]">
        <Icon icon="solar:refresh-bold" className="h-4 w-4 animate-spin" />
        Loading...
      </div>
    );
  }

  if (accounts.length === 0) {
    return (
      <div className="px-2 py-5 text-center">
        <p className="text-sm text-[var(--text-secondary)]">No accounts yet</p>
      </div>
    );
  }

  return (
    <div className="space-y-0.5 px-2 py-1.5">
      {accounts.map((account) => (
        <AccountSwitcherRow
          key={account.id}
          account={account}
          isLoading={isLoading}
          onSetActive={onSetActive}
          onRemoveAccount={onRemoveAccount}
        />
      ))}
    </div>
  );
}

function AccountTrigger({
  activeAccount,
  open,
  onToggle,
}: {
  activeAccount: MinecraftAccount | null;
  open: boolean;
  onToggle: () => void;
}) {
  const username =
    activeAccount?.minecraft_username || activeAccount?.username || "Add account";

  return (
    <button
      type="button"
      onClick={onToggle}
      className="group flex w-full items-center gap-2 rounded-xl border border-[var(--surface-border)] bg-[var(--surface-overlay)] p-2.5 text-left transition-colors hover:border-[var(--surface-border-strong)] hover:bg-[var(--surface-base)]"
    >
      {activeAccount ? (
        <AccountAvatar account={activeAccount} size={32} />
      ) : (
        <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg border border-[var(--surface-border)] bg-[var(--surface-base)]">
          <span className="text-xs text-white">+</span>
        </div>
      )}

      <div className="min-w-0 flex-1">
        <StreamerSensitiveText
          text={username}
          className="block truncate text-sm font-medium text-white"
        />
        <span className="mt-0.5 flex items-center gap-1 text-[11px] text-[var(--accent)]">
          {activeAccount ? (
            <>
              <span className="h-1.5 w-1.5 flex-shrink-0 rounded-full bg-[var(--accent)]" />
              Online
            </>
          ) : (
            <span className="text-[var(--text-muted)]">Sign in to play</span>
          )}
        </span>
      </div>

      <Icon
        icon={open ? "solar:alt-arrow-up-bold" : "solar:alt-arrow-down-bold"}
        className="ml-1 h-4 w-4 flex-shrink-0 text-[var(--text-secondary)]"
      />
    </button>
  );
}

export function AccountSwitcherPanel({
  open,
  onOpenChange,
  accounts,
  activeAccount,
  isLoading,
  onSetActive,
  onRemoveAccount,
  onAddAccount,
  mode = "switcher",
}: AccountSwitcherPanelProps) {
  if (mode === "list") {
    return (
      <AccountSwitcherList
        accounts={accounts}
        isLoading={isLoading}
        onSetActive={onSetActive}
        onRemoveAccount={onRemoveAccount}
      />
    );
  }

  return (
    <div className="relative">
      {open && (
        <div className="absolute bottom-full left-0 right-0 z-50 mb-1.5">
          <div className="overflow-hidden rounded-xl border border-[var(--surface-border)] bg-[var(--surface-raised)] shadow-xl">
            <div className="custom-scrollbar max-h-52 overflow-y-auto">
              <AccountSwitcherList
                accounts={accounts}
                isLoading={isLoading}
                onSetActive={onSetActive}
                onRemoveAccount={onRemoveAccount}
              />
            </div>

            <button
              type="button"
              onClick={() => void onAddAccount()}
              disabled={isLoading}
              className="flex w-full items-center justify-center gap-2 border-t border-[var(--surface-border)] px-2 py-2 text-sm text-[var(--text-secondary)] transition-colors hover:bg-[var(--surface-base)] hover:text-white disabled:opacity-50"
            >
              {isLoading ? (
                <Icon icon="solar:refresh-bold" className="h-4 w-4 animate-spin" />
              ) : (
                <Icon icon="solar:add-circle-linear" className="h-4 w-4" />
              )}
              {isLoading ? "Processing..." : "Add account"}
            </button>
          </div>
        </div>
      )}

      <AccountTrigger
        activeAccount={activeAccount}
        open={open}
        onToggle={() => onOpenChange(!open)}
      />
    </div>
  );
}
