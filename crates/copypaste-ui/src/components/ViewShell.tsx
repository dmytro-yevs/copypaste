import type { ReactNode } from "react";

/**
 * Shared screen frame: a floating glass header bar + a floating glass content area.
 *
 * Both the header and the content panel are separate floating glass cards that sit
 * over the aurora backdrop with a gap between them — the aurora shows through the
 * gap, reinforcing the "panels hovering in space" Liquid Glass look.
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
        Floating header — a detached rounded glass bar sitting over the aurora.
        Inset from the top by the parent's 10px padding (already applied by
        the gap in App.tsx's column). The header itself has NO top margin here
        because the column gap provides the 10px spacing from the window top.

        data-tauri-drag-region: the macOS custom titlebar drag MUST stay on
        this element so the user can drag the window by grabbing the header bar.
        The title <h1> also carries it for full-width draggability.
        The `actions` slot intentionally does NOT carry it — Tauri v2 stops
        drag-initiation on elements without the attribute, keeping buttons clickable.

        Radius: rounded-ide-lg = 14px (styleguide --radius-card).
        Shadow: shadow-ide-sm = float shadow (reads as hovering over aurora).
        No bottom border: the card border from surface-glass provides the rim.
      */}
      <header
        data-tauri-drag-region
        className={[
          // §jxbx-6: card-in entrance — cubic-bezier spring from index.css utility.
          "surface-glass card-in",
          "flex h-11 shrink-0 items-center justify-between px-4",
          // W-C2: radius + shadow driven by skin tokens so quiet/vapor skins apply
          // without code changes. Classic values are identical to rounded-ide-lg(14px)
          // and shadow-ide-sm(--ide-e2), so classic look is byte-identical.
        ].join(" ")}
        style={{
          borderRadius: "var(--skin-r-card)",
          boxShadow: "var(--skin-shadow-card)",
        }}
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

      {/*
        Floating content panel — a separate glass card below the header.
        flex-1 + min-h-0 so it fills the remaining height.
        overflow-auto: content scrolls INSIDE the floating card; the card
        stays pinned (does not grow to push out of the window).
        rounded-ide-lg: same 14px radius as the header.
      */}
      {/* §jxbx-7: reveal-up entrance — content panel rises after header settles. */}
      {/* W-C2: radius uses --skin-r-card; shadow uses --skin-shadow-card.
          CopyPaste-aq5w: classic --skin-shadow-card = var(--ide-e2) = shadow-ide-sm (E2),
          so the content panel is byte-identical to pre-skin. quiet/vapor get their card
          shadow (none) instead of the stronger float shadow (E3). */}
      <div
        className="surface-glass reveal-up min-h-0 flex-1 overflow-auto p-4"
        style={{
          borderRadius: "var(--skin-r-card)",
          boxShadow: "var(--skin-shadow-card)",
        }}
      >
        {children}
      </div>
    </div>
  );
}
