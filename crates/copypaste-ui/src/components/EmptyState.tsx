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
      <span style={{ color: "rgba(255,255,255,0.20)", fontSize: 28, lineHeight: 1 }}>
        {icon}
      </span>
      <p className="text-[13px]" style={{ color: "rgba(255,255,255,0.45)" }}>
        {title}
      </p>
      <p className="text-[11px]" style={{ color: "rgba(255,255,255,0.28)" }}>
        {body}
      </p>
      {action}
    </div>
  );
}
