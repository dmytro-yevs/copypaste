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
        Header — §8: glass surface (surface-glass replaces solid bg-ide-panel),
        hairline bottom border kept. data-tauri-drag-region makes the whole bar
        draggable (macOS titleBarStyle "Overlay" + decorations:true custom titlebar).
        The title <h1> also carries the attribute so dragging over the text works too.
        The `actions` slot intentionally does NOT carry it: Tauri v2 stops
        drag-initiation on elements that lack the attribute, so buttons/inputs inside
        `actions` remain clickable.
      */}
      <header
        data-tauri-drag-region
        className={[
          "flex h-11 shrink-0 items-center justify-between px-4",
          // §8: glass header surface; hairline bottom border stays.
          "border-b border-ide-border surface-glass",
          "shadow-ide-xs",
        ].join(" ")}
      >
        <h1
          data-tauri-drag-region
          // §3: view title = 14px medium (was 13px semibold).
          className="text-[14px] font-medium tracking-wide text-ide-text"
        >
          {title}
        </h1>
        <div className="flex items-center gap-2">{actions}</div>
      </header>

      {/* Content — scrollable, bg matches root. surface-glass applies the
          canonical translucency recipe (rgba(19,20,26,.72)+blur(30px)+saturate(180%))
          per §3 — no inline glass recipe needed. */}
      <div
        className="surface-glass min-h-0 flex-1 overflow-auto p-4"
      >
        {children}
      </div>
    </div>
  );
}
