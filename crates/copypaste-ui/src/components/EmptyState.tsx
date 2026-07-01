import React from "react";

/**
 * Shared empty / error / offline hero block used by HistoryView, DevicesView,
 * and the Popup.
 *
 * Layout: flex-col centred, gap-2, px-6 py-8 text-center.
 * - `icon`   — any React node; rendered at 28 px / text-ide-faint by default.
 * - `title`  — primary line, 13 px text-ide-dim.
 * - `body`   — secondary line, 11 px text-ide-faint.
 * - `action` — optional action node (e.g. a RestartDaemonButton) below the body.
 * - `className` — extra classes added to the outer wrapper (e.g. "h-full").
 */
export function EmptyState({
  icon,
  title,
  body,
  action,
}: {
  icon: React.ReactNode;
  title: string;
  body: string;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div>
      {/* Decorative icon: --ide-ghost-deco = 3.01:1 on panel (WCAG AA large/decorative ≥3:1). */}
      <span>
        {icon}
      </span>
      {/* Title: --ide-dim = #9da0a8, contrast > 4.5:1 on panel. */}
      <p>
        {title}
      </p>
      {/* Body: --ide-ghost = rgba(255,255,255,0.46) = 4.56:1 on panel (WCAG AA). */}
      <p>
        {body}
      </p>
      {action}
    </div>
  );
}
