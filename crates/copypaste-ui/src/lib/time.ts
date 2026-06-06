/**
 * Shared relative-time formatter for HistoryView and the popup.
 *
 * Two styles:
 *  - "long":  used in HistoryView rows.
 *             0 → "—", <1 min → "just now", <1 h → "Xm ago",
 *             <1 d → "Xh ago", <7 d → "Xd ago", else formatWallTime.
 *  - "short": used in the popup right-cluster (tabular-nums).
 *             0 → "", <1 min → "now", <1 h → "Xm", <1 d → "Xh",
 *             else "Xd" (no cap — popup list is limited to MAX_ITEMS rows).
 */
import { formatWallTime } from "./ipc";

export function formatRelativeTime(ms: number, style: "short" | "long"): string {
  if (style === "long") {
    if (ms <= 0) return "—";
    const diff = Date.now() - ms;
    if (diff < 60_000) return "just now";
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)}d ago`;
    return formatWallTime(ms);
  }
  // "short"
  if (!ms || ms <= 0) return "";
  const diff = Date.now() - ms;
  if (diff < 60_000) return "now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h`;
  return `${Math.floor(diff / 86_400_000)}d`;
}
