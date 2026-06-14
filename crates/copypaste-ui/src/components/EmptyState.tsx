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
  className,
}: {
  icon: React.ReactNode;
  title: string;
  body: string;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={[
        "flex flex-col items-center justify-center gap-2 px-6 py-8 text-center",
        className ?? "",
      ]
        .join(" ")
        .trim()}
    >
      {/* Decorative icon: --ide-ghost-deco = 3.01:1 on panel (WCAG AA large/decorative ≥3:1). */}
      <span style={{ color: "var(--ide-ghost-deco)", fontSize: 28, lineHeight: 1 }}>
        {icon}
      </span>
      {/* Title: --ide-dim = #9da0a8, contrast > 4.5:1 on panel. */}
      <p className="text-[13px] text-ide-dim">
        {title}
      </p>
      {/* Body: --ide-ghost = rgba(255,255,255,0.46) = 4.56:1 on panel (WCAG AA). */}
      <p className="text-[11px]" style={{ color: "var(--ide-ghost)" }}>
        {body}
      </p>
      {action}
    </div>
  );
}
