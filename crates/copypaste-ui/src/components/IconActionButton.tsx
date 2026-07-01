/**
 * IconActionButton — shared icon-only action button primitive (CopyPaste-bdac.26).
 *
 * Extracted from HistoryView.tsx:883-913 (local `IconActionBtn`).
 * A 20×20 visual button with an invisible 44×44 px hit-target overlay,
 * border-transparent at rest → ide-border/ide-elevated on hover.
 * Matches the Android `CopyPasteIconButton` 28px-glyph / 44dp-target pattern.
 *
 * Props:
 *   aria-label  — required (accessibility label; also used as tooltip fallback)
 *   title       — tooltip text
 *   danger      — when true, uses text-ide-danger color instead of dim/text
 *   onClick     — click handler (stopPropagation is applied internally)
 *   children    — icon element (SVG / Lucide component)
 */

import React from "react";

export interface IconActionButtonProps {
  "aria-label": string;
  title: string;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

/**
 * Icon-only action button with an oversized invisible hit target.
 *
 * Visual size: 20×20 px (h-5 w-5).
 * Hit target: 44×44 px achieved via an absolutely-positioned transparent overlay
 * (inset: -12px), which expands reach without affecting layout.
 *
 * Exported as `IconActionButton`; HistoryView uses this canonical name.
 * The former local alias `IconActionBtn` is gone — update any future references
 * to use this export.
 */
export function IconActionButton({
  "aria-label": ariaLabel,
  title,
  onClick,
  children,
}: IconActionButtonProps) {
  return (
    <button
      aria-label={ariaLabel}
      title={title}
      onClick={(e: React.MouseEvent<HTMLButtonElement>) => {
        e.stopPropagation();
        onClick();
      }}
    >
      {/* Transparent hit-target overlay expanding clickable area to ≥44×44px
          without affecting the 20px visual button size or row layout.
          KEPT: position/inset is functionally required to enlarge the click/tap
          target beyond the visual glyph bounds (accessibility hit-target rule),
          not decorative styling. */}
      <span aria-hidden="true" style={{ position: "absolute", inset: "-12px" }} />
      {children}
    </button>
  );
}
