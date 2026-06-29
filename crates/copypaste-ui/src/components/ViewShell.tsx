import type { ReactNode } from "react";

/**
 * Shared screen frame: a floating glass header bar + a floating glass content area.
 *
 * Both the header and the content panel are separate floating glass cards with a
 * gap between them, reinforcing the "panels hovering in space" look.
 *
 * Layout contract (set by App.tsx <main>):
 *   • <main> is `min-h-0 flex-1 overflow-hidden` — fills available height.
 *   • ViewShell is `h-full flex flex-col gap-[10px]`.
 *   • Header: fixed height, rounded glass card, pinned to top (no scroll).
 *   • Content: flex-1, rounded glass card, scrollable inside.
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
    <div className="flex h-full flex-col gap-[10px]">
      {/*
        Floating header — a detached rounded glass bar.
        Inset from the top by the parent's 10px padding (already applied by
        the gap in App.tsx's column). The header itself has NO top margin here
        because the column gap provides the 10px spacing from the window top.

        data-tauri-drag-region: the macOS custom titlebar drag MUST stay on
        this element so the user can drag the window by grabbing the header bar.
        The title <h1> also carries it for full-width draggability.
        The `actions` slot intentionally does NOT carry it — Tauri v2 stops
        drag-initiation on elements without the attribute, keeping buttons clickable.
      */}
      <header
        data-tauri-drag-region
        className={[
          // §jxbx-6: card-in entrance — cubic-bezier spring from index.css utility.
          "surface-glass card-in",
          "flex h-11 shrink-0 items-center justify-between px-4",
        ].join(" ")}
        style={{
          borderRadius: "var(--r-card)",
          boxShadow: "var(--sh1)",
        }}
      >
        <h1
          data-tauri-drag-region
          // §3: view title = 13px medium (design spec).
          className="text-[13px] font-medium tracking-wide text-ide-text"
        >
          {title}
        </h1>
        <div className="flex items-center gap-2">{actions}</div>
      </header>

      {/*
        Floating content panel — a separate glass card below the header.
        flex-1 + min-h-0 so it fills the remaining height.
        overflow-auto: content scrolls INSIDE the floating card.
      */}
      {/* §jxbx-7: reveal-up entrance — content panel rises after header settles. */}
      <div
        className="surface-glass reveal-up min-h-0 flex-1 overflow-auto p-4"
        style={{
          borderRadius: "var(--r-card)",
          boxShadow: "var(--sh1)",
        }}
      >
        {children}
      </div>
    </div>
  );
}
