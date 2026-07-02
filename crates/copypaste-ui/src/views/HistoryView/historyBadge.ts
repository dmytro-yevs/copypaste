/**
 * historyBadge.ts — toolbar count-badge logic for HistoryView (CopyPaste-g27b.37).
 *
 * Pure so it can be unit-tested without rendering the (heavily IPC-coupled)
 * HistoryView component tree — mirrors historyVirtualizer.ts / historySignature.ts's
 * "pure fn extracted for testability" pattern in this directory.
 */

export interface HistoryBadgeInput {
  /** Daemon-reported total row count across the whole DB, or null before the
   *  first page has resolved (badge stays hidden in that case). */
  totalCount: number | null;
  /** Length of useHistoryFilter's `filtered` array (search + device filter + sort applied). */
  filteredCount: number;
  /** useHistoryFilter's `search` state (raw, untrimmed). */
  search: string;
  /** useHistoryFilter's `deviceFilter` state ("all" = no device filter active). */
  deviceFilter: string;
}

/**
 * The toolbar badge previously always showed `totalCount` (the full daemon
 * DB count) even while a search query or device filter narrowed the visible
 * list down to a handful of rows — or zero. That reads as "14 items" next to
 * an empty "No matches" result, which is misleading. Whenever a search or
 * device filter is active, the badge should reflect what's actually on
 * screen (`filteredCount`); otherwise it shows the unfiltered daemon total.
 */
export function historyBadgeCount(input: HistoryBadgeInput): number | null {
  const { totalCount, filteredCount, search, deviceFilter } = input;
  if (totalCount === null) return null;
  const isFiltered = search.trim().length > 0 || deviceFilter !== "all";
  return isFiltered ? filteredCount : totalCount;
}
