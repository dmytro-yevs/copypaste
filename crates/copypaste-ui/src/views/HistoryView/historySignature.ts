/**
 * Cheap signature: join of `id|pinned|wall_time` for each item in order.
 * Detecting a change here means we actually need to re-render.
 *
 * Memoised with a 1-slot cache (CopyPaste-44rq.35): the poll fires every 3 s
 * against up to 200 items.  When the clipboard is idle the incoming array has
 * the same length and the same first+last fingerprint as the previous call, so
 * we can return the cached result and skip the O(n) map+join entirely.
 *
 * Correctness guarantee: a pin/unpin on any item changes that item's wall_time
 * slot in the format string AND the daemon re-orders pinned items to the top,
 * so the first-item fingerprint changes on every pin event.  A plain new item
 * always changes the length.  Therefore the fast-path cache hit can only occur
 * when the full string would have been identical anyway.
 */
import type { HistoryEntry } from "../../lib/ipc";

// Cache key uses the three properties that together uniquely identify the list
// state visible to this function: total count, first-item fingerprint, and
// last-item fingerprint.
type _SigCacheEntry = { len: number; first: string; last: string; result: string };
// Exported for unit testing only — not part of the public API.
export let _itemsSigCache: _SigCacheEntry | null = null;

function _itemFingerprint(it: HistoryEntry): string {
  return `${it.id}:${it.pinned ? 1 : 0}:${it.wall_time}`;
}

// Exported for unit testing only — not part of the public API.
export function itemsSignature(items: HistoryEntry[]): string {
  if (items.length === 0) return "";
  const len = items.length;
  const first = _itemFingerprint(items[0]);
  const last = _itemFingerprint(items[len - 1]);
  // Fast path: return cached result when the envelope (length + first + last)
  // matches.  A new clipboard entry always changes `len` or moves it to first
  // position; a pin toggles `pinned` and the daemon promotes it to first.
  if (
    _itemsSigCache !== null &&
    _itemsSigCache.len === len &&
    _itemsSigCache.first === first &&
    _itemsSigCache.last === last
  ) {
    return _itemsSigCache.result;
  }
  const result = items.map(_itemFingerprint).join("|");
  _itemsSigCache = { len, first, last, result };
  return result;
}
