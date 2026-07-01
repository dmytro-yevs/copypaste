import type { ReactNode } from "react";

/**
 * Shared screen frame: a header bar + content area.
 * Classnames stripped in design-demolition pass (CopyPaste-h1n3).
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
    <div>
      <header
        data-tauri-drag-region
      >
        <h1 data-tauri-drag-region>
          {title}
        </h1>
        <div>{actions}</div>
      </header>

      <div>
        {children}
      </div>
    </div>
  );
}
