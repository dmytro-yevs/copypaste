import { useState } from "react";

import { absoluteTime, shortAge } from "@/lib/format";
import { cn } from "@/lib/cn";

interface TimestampToggleProps {
  createdAt: number;
  tabIndex?: number;
  className?: string;
}

/**
 * Keeps a history row compact while allowing a copied time to be inspected
 * without opening a separate surface. It is intentionally a sibling of the
 * row's copy button: nested controls would make a time click copy the clip.
 */
export function TimestampToggle({
  createdAt,
  tabIndex = -1,
  className,
}: TimestampToggleProps) {
  const [exact, setExact] = useState(false);
  const concise = shortAge(createdAt);
  const full = absoluteTime(createdAt);
  const label = exact
    ? `Copied ${full}. Activate to show relative time.`
    : `Copied ${concise}. Activate to show exact time.`;

  return (
    <button
      type="button"
      tabIndex={tabIndex}
      aria-pressed={exact}
      aria-label={label}
      title={label}
      onClick={(event) => {
        event.stopPropagation();
        setExact((shown) => !shown);
      }}
      onKeyDown={(event) => event.stopPropagation()}
      className={cn(
        "-my-1 min-h-7 shrink-0 rounded px-1 py-0.5 text-left text-xs text-muted-foreground outline-none transition-colors hover:bg-secondary hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring",
        className,
      )}
    >
      <time dateTime={new Date(createdAt).toISOString()}>{exact ? full : concise}</time>
    </button>
  );
}
