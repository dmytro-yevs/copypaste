import React from "react";
import { Inbox } from "lucide-react";

/**
 * Shared empty / error / offline hero block used by HistoryView, DevicesView,
 * and the Popup.
 *
 * - `title`  — primary line.
 * - `body`   — secondary line.
 * - `icon`   — optional icon node shown in the icon chip above the title.
 *              Falls back to a generic lucide icon when omitted (no current
 *              caller passes one, so every existing usage gets the default).
 * - `action` — optional action node (e.g. a RestartDaemonButton) below the body.
 */
export function EmptyState({
  title,
  body,
  icon,
  action,
}: {
  title: string;
  body: string;
  icon?: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <div className="empty">
      <div className="empty__ic">
        {icon ?? <Inbox />}
      </div>
      <p className="empty__t">
        {title}
      </p>
      <p className="empty__s">
        {body}
      </p>
      {action}
    </div>
  );
}
