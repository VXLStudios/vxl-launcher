"use client";

import { cn } from "../../lib/utils";
import { useStreamerModeStore } from "../../store/streamer-mode-store";

interface StreamerSensitiveTextProps {
  text: string;
  className?: string;
  /** When true, text is revealed on parent `group` hover (blur otherwise). */
  revealOnGroupHover?: boolean;
  title?: string;
}

export function StreamerSensitiveText({
  text,
  className,
  revealOnGroupHover = true,
  title,
}: StreamerSensitiveTextProps) {
  const streamerMode = useStreamerModeStore((state) => state.enabled);

  if (!streamerMode) {
    return (
      <span className={className} title={title ?? text}>
        {text}
      </span>
    );
  }

  return (
    <span
      className={cn(
        className,
        revealOnGroupHover &&
          "blur-[5px] transition-[filter] duration-200 group-hover:blur-none",
      )}
      title={title}
    >
      {text}
    </span>
  );
}
