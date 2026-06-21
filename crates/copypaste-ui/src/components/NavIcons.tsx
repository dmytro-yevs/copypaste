/**
 * NavIcons.tsx — nav-bar icons from lucide-react (CopyPaste-dt5k, ICON-11).
 *
 * ICON-11: unified to lucide-react so all icon glyphs in the app share the
 * same family (stroke 1.5, round caps/joins, currentColor). The previous
 * hand-crafted SF Symbol clones are replaced with the closest lucide
 * equivalents:
 *
 *   History  → lucide History          (clock face + ccw arrow — same concept)
 *   Devices  → lucide MonitorSmartphone (laptop + phone side-by-side)
 *   Settings → lucide Settings          (gear)
 *   About    → lucide Info              (info circle)
 *   Logs     → lucide ScrollText        (scroll with ruled lines)
 *
 * Usage: <HistoryIcon className="text-white" />
 * The icon is always aria-hidden — the parent nav button carries the label.
 */

import type { CSSProperties } from "react";
import { History, MonitorSmartphone, Settings, Info, ScrollText, type LucideProps } from "lucide-react";

// Shared size/stroke for all nav icons.
const NAV_ICON_PROPS: Partial<LucideProps> = {
  width: 18,
  height: 18,
  strokeWidth: 1.85,
  "aria-hidden": true,
} satisfies Partial<LucideProps>;

// ---------------------------------------------------------------------------
// History — clock face with a counter-clockwise arrow (lucide History)
// ---------------------------------------------------------------------------
export function HistoryIcon({ className, style }: { className?: string; style?: CSSProperties }) {
  return <History {...NAV_ICON_PROPS} className={className} style={style} />;
}

// ---------------------------------------------------------------------------
// Devices — laptop + small phone (lucide MonitorSmartphone)
// ---------------------------------------------------------------------------
export function DevicesIcon({ className, style }: { className?: string; style?: CSSProperties }) {
  return <MonitorSmartphone {...NAV_ICON_PROPS} className={className} style={style} />;
}

// ---------------------------------------------------------------------------
// Settings — gear (lucide Settings)
// ---------------------------------------------------------------------------
export function SettingsIcon({ className, style }: { className?: string; style?: CSSProperties }) {
  return <Settings {...NAV_ICON_PROPS} className={className} style={style} />;
}

// ---------------------------------------------------------------------------
// About — info circle (lucide Info)
// ---------------------------------------------------------------------------
export function AboutIcon({ className, style }: { className?: string; style?: CSSProperties }) {
  return <Info {...NAV_ICON_PROPS} className={className} style={style} />;
}

// ---------------------------------------------------------------------------
// Logs — scroll with text lines (lucide ScrollText)
// ---------------------------------------------------------------------------
export function LogsIcon({ className, style }: { className?: string; style?: CSSProperties }) {
  return <ScrollText {...NAV_ICON_PROPS} className={className} style={style} />;
}
