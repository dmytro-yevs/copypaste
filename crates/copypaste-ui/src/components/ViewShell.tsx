import type { ReactNode } from "react";

/**
 * Shared screen frame: a header bar + content area.
 * Wired to the redesign shell chrome (Slice 5 / CopyPaste-g27b.12):
 * `.view` > `.vhead` (draggable header, `.vtitle`) + content.
 */
export function ViewShell({
  title,
  actions,
  children
}: {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="view">
      <header
        className="vhead"
        data-tauri-drag-region
      >
        <h1 className="vtitle" data-tauri-drag-region>
          {title}
        </h1>
        {/* Actions slot: flex row (`.vhead__actions`). A search `.field` inside
            grows to fill; buttons trail to the right. Shared by History (toolbar),
            Logs (filter + Refresh/Export) and Devices (Revoke all); empty for
            About/Settings, which pass no actions. */}
        {actions ? <div className="vhead__actions">{actions}</div> : null}
      </header>

      <div className="view__body">
        {children}
      </div>
    </div>
  );
}
