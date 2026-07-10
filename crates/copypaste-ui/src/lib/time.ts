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
 * CopyPaste-8ebg.55: delegates to `formatRelativeTime(ms, "long")` for the
 * relative/absolute split instead of duplicating its own thresholds. Before
 * this change Devices used a 24 h cutoff while History (formatRelativeTime
 * "long") used 7 days, so the same ~30 h-old item read as an absolute date
 * in one screen and "1d ago" in the other — same underlying policy now
 * everywhere a "long" relative time is shown:
 * - 0 / negative input → null (caller should hide the field).
 * - <60 s ago  → "just now"
 * - <1 h        → "Xm ago"
 * - <24 h       → "Xh ago"
 * - <7 d        → "Xd ago"
 * - ≥7 d / future → absolute (formatWallTime / locale string).
 *
 * Input is Unix epoch seconds (PairedDevice.last_sync_at field) or
 * Unix epoch milliseconds (SyncStatus.last_sync_ms) depending on the call site.
 * Pass `unit: "ms"` for the milliseconds variant (default: "secs").
 *
 * Android parity note (superseded by the unification above): Android uses
 * "Xs ago" / "Xm ago" / "Xh ago" for ≤24 h, then absolute date. Prepend
 * "Synced " at the call site to match Android's label wording.
 */
export function formatSyncTime(
  value: number | null | undefined,
  unit: "secs" | "ms" = "secs"
): string | null {
  if (!value || value <= 0) return null;
  const ms = unit === "ms" ? value : value * 1000;
  if (Date.now() - ms < 0) return new Date(ms).toLocaleString(); // future timestamp — show absolute
  return formatRelativeTime(ms, "long");
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
