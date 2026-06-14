/**
 * NavIcons.tsx — SF-like stroke SVG nav icons (CopyPaste-dt5k)
 *
 * 24×24 grid, fill=none, stroke=currentColor, strokeWidth=1.85,
 * strokeLinecap/Join=round. All paths mirror the styleguide SF Symbols:
 *   History  → 􀐫  clock.arrow.circlepath
 *   Devices  → 􀙧  laptopcomputer.and.iphone
 *   Settings → 􀍟  gear
 *   About    → 􀅴  info.circle
 *   Logs     → 􀈤  scroll
 *
 * Usage: <HistoryIcon className="text-white" />
 * The svg is always aria-hidden — the parent nav button carries the accessible label.
 */

import type { SVGProps } from "react";

// Shared SVG prop subset consumed by each icon.
type IconProps = Pick<SVGProps<SVGSVGElement>, "className" | "style">;

const COMMON = {
  xmlns: "http://www.w3.org/2000/svg",
  viewBox: "0 0 24 24",
  width: 18,
  height: 18,
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.85,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true as const,
} satisfies SVGProps<SVGSVGElement>;

// ---------------------------------------------------------------------------
// History — clock face with a left-turning arrow (clock.arrow.circlepath)
// ---------------------------------------------------------------------------
export function HistoryIcon({ className, style }: IconProps) {
  return (
    <svg {...COMMON} className={className} style={style}>
      {/* outer clock circle */}
      <circle cx="12" cy="12" r="8.5" />
      {/* hour hand pointing up */}
      <polyline points="12 7.5 12 12 15 14.5" />
      {/* counter-clockwise refresh arc + arrowhead at the top-left */}
      <path d="M5.64 5.64A8.48 8.48 0 0 0 3.5 12" />
      <polyline points="1.5 10 3.5 12 5.5 10" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Devices — laptop + small phone (laptopcomputer.and.iphone)
// ---------------------------------------------------------------------------
export function DevicesIcon({ className, style }: IconProps) {
  return (
    <svg {...COMMON} className={className} style={style}>
      {/* laptop body */}
      <rect x="2" y="4" width="15" height="11" rx="1.5" />
      {/* laptop base / keyboard ledge */}
      <path d="M1 15h17l.5 1.5H.5L1 15Z" />
      {/* phone (right side) */}
      <rect x="18.5" y="8" width="4.5" height="8" rx="1" />
      {/* phone home indicator */}
      <line x1="20.75" y1="14.75" x2="20.75" y2="14.76" strokeWidth={2} />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Settings — gear (gear / gearshape)
// ---------------------------------------------------------------------------
export function SettingsIcon({ className, style }: IconProps) {
  return (
    <svg {...COMMON} className={className} style={style}>
      {/* inner circle */}
      <circle cx="12" cy="12" r="3" />
      {/* 8-tooth gear outline via path */}
      <path d="M19.14 12.94a7.25 7.25 0 0 0 .06-.94 7.25 7.25 0 0 0-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.62l-1.92-3.32a.49.49 0 0 0-.59-.21l-2.4.96a7.11 7.11 0 0 0-1.62-.94l-.36-2.54A.48.48 0 0 0 14 3h-4a.48.48 0 0 0-.48.41l-.36 2.54a7.11 7.11 0 0 0-1.61.94l-2.4-.96a.49.49 0 0 0-.6.21L2.63 9.46a.48.48 0 0 0 .12.62l2.03 1.58A7.34 7.34 0 0 0 4.72 13l-2.03 1.58a.48.48 0 0 0-.12.62l1.92 3.32c.12.22.38.3.6.21l2.4-.96c.5.36 1.04.67 1.61.94l.36 2.54c.06.24.27.41.48.41h4c.22 0 .42-.17.48-.41l.36-2.54a7.11 7.11 0 0 0 1.61-.94l2.4.96c.22.09.48 0 .6-.21l1.92-3.32a.48.48 0 0 0-.12-.62L19.14 12.94Z" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// About — info circle (info.circle)
// ---------------------------------------------------------------------------
export function AboutIcon({ className, style }: IconProps) {
  return (
    <svg {...COMMON} className={className} style={style}>
      <circle cx="12" cy="12" r="9.25" />
      {/* info dot */}
      <line x1="12" y1="11.5" x2="12" y2="16.5" />
      <line x1="12" y1="8" x2="12.01" y2="8" strokeWidth={2.2} />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Logs — scroll / document with lines (scroll)
// ---------------------------------------------------------------------------
export function LogsIcon({ className, style }: IconProps) {
  return (
    <svg {...COMMON} className={className} style={style}>
      {/* scroll body */}
      <path d="M7 4h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7" />
      {/* scroll left curl */}
      <path d="M7 20a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2" />
      {/* text lines */}
      <line x1="11" y1="9" x2="17" y2="9" />
      <line x1="11" y1="12" x2="17" y2="12" />
      <line x1="11" y1="15" x2="15" y2="15" />
    </svg>
  );
}
