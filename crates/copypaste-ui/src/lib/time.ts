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
 *
 * i2sr (PG-40): formatSyncTime — hybrid formatter for device last-sync timestamps.
 *  Matches Android relative display when ≤24 h ago; falls back to locale date beyond 24 h.
 *  Returns null for null/zero input so callers can hide the field entirely.
 */
import { formatWallTime } from "./ipc";

/**
 * i2sr (PG-40): Hybrid formatter for device last-sync timestamps.
 *
 * - null / 0 → null (caller should hide the field).
 * - <60 s ago  → "just now"
 * - <1 h        → "Xm ago"  (whole minutes)
 * - <24 h       → "Xh ago"  (whole hours)
 * - ≥24 h      → locale absolute date string (toLocaleString).
 *
 * Input is Unix epoch seconds (PairedDevice.last_sync_at field) or
 * Unix epoch milliseconds (SyncStatus.last_sync_ms) depending on the call site.
 * Pass `unit: "ms"` for the milliseconds variant (default: "secs").
 *
 * Android parity: Android uses "Xs ago" / "Xm ago" / "Xh ago" for ≤24 h,
 * then absolute date. Prepend "Synced " at the call site to match Android label.
 */
export function formatSyncTime(
  value: number | null | undefined,
  unit: "secs" | "ms" = "secs"
): string | null {
  if (!value || value <= 0) return null;
  const ms = unit === "ms" ? value : value * 1000;
  const diffMs = Date.now() - ms;
  if (diffMs < 0) return new Date(ms).toLocaleString(); // future timestamp — show absolute
  const diffSecs = Math.floor(diffMs / 1000);
  if (diffSecs < 60) return "just now";
  if (diffMs < 3_600_000) return `${Math.floor(diffSecs / 60)}m ago`;
  if (diffMs < 86_400_000) return `${Math.floor(diffMs / 3_600_000)}h ago`;
  // Beyond 24 h — absolute locale date (matches macOS existing absolute style)
  return new Date(ms).toLocaleString();
}

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
