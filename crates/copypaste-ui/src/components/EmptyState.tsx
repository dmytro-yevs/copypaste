import React from "react";

/**
 * Shared empty / error / offline hero block used by HistoryView, DevicesView,
 * and the Popup.
 *
 * - `title`  — primary line.
 * - `body`   — secondary line.
 * - `action` — optional action node (e.g. a RestartDaemonButton) below the body.
 */
export function EmptyState({
  title,
  body,
  action,
}: {
  title: string;
  body: string;
  action?: React.ReactNode;
}) {
  return (
    <div>
      <p>
        {title}
      </p>
      <p>
        {body}
      </p>
      {action}
    </div>
  );
}
