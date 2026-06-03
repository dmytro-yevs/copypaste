import type { ReactNode } from "react";

/** Shared screen frame: a titled header bar + a scrollable content area. */
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
    <div className="flex h-full flex-col">
      {/*
        Header — IDE toolbar style: panel bg, hairline bottom border, depth shadow.
        data-tauri-drag-region makes the whole bar draggable (macOS titleBarStyle
        "Overlay" + decorations:true custom titlebar).  The title <h1> also carries
        the attribute so dragging over the text works too.  The `actions` slot
        intentionally does NOT carry it: Tauri v2 stops drag-initiation on elements
        that lack the attribute, so buttons/inputs inside `actions` remain clickable.
      */}
      <header
        data-tauri-drag-region
        className={[
          "flex h-11 shrink-0 items-center justify-between px-4",
          "border-b border-ide-border bg-ide-panel",
          "shadow-ide-xs",
        ].join(" ")}
      >
        <h1
          data-tauri-drag-region
          className="text-[13px] font-semibold tracking-wide text-ide-text"
        >
          {title}
        </h1>
        <div className="flex items-center gap-2">{actions}</div>
      </header>

      {/* Content — scrollable, bg matches root */}
      <div className="min-h-0 flex-1 overflow-auto bg-ide-bg p-4">
        {children}
      </div>
    </div>
  );
}
