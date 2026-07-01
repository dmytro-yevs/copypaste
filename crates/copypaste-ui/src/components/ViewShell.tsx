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
        {/* Actions slot left structurally as-is (no new wrapper layout rules):
            ViewShell is shared by History/Devices/Settings/About/Logs, and
            only LogView (this slice) populates it with a `.field` + buttons
            cluster — a flex-row treatment here would also reflow the other
            views' still-unwired action rows, which are out of scope. */}
        <div>{actions}</div>
      </header>

      <div>
        {children}
      </div>
    </div>
  );
}
