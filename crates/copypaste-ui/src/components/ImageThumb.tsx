import { useEffect, useRef, useState } from "react";
import { api } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Shared image cache — keyed by item id, value is the resolved data URI (or
// null when the fetch failed). Module-level so it survives re-mounts across
// HistoryView and PopupRow.
//
// Byte-budget LRU (~24 MiB cap): each entry's cost is the byte length of its
// data-URI string. Null entries (recorded misses) cost 0 bytes. When the
// cumulative cost of all entries exceeds CACHE_BUDGET_BYTES the oldest (LRU)
// entry is evicted until the cache is within budget.
//
// LRU ordering is implemented via Map insertion order: on every access
// (cacheGet "touch" + cacheSet) we delete-then-re-insert so the Map tail is
// always the most-recently-used key and the head is the LRU candidate.
// ---------------------------------------------------------------------------

// 16 MiB (HB-10): caps the cumulative COMPRESSED data-URI string bytes held in
// memory. The dominant image-memory cost is the WebView's DECODED bitmaps, now
// bounded by the smaller 192 px thumbnail source + the intrinsic decode-size
// hint on the <img> below; this budget is the secondary, string-side bound and
// is trimmed modestly from 24 → 16 MiB to keep total RSS down.
const CACHE_BUDGET_BYTES = 16 * 1024 * 1024; // 16 MiB

// Value stored per entry: { uri, bytes }.
interface CacheEntry {
  uri: string | null;
  bytes: number;
}

const imageCache = new Map<string, CacheEntry>();
let cacheTotalBytes = 0;

// In-flight promise cache — prevents parallel duplicate fetches for the same
// item id (e.g. popup + history both mount an ImageThumb for the same id).
const inflight = new Map<string, Promise<string | null>>();

function cacheGet(id: string): string | null | undefined {
  const entry = imageCache.get(id);
  if (entry === undefined) return undefined;
  // Touch — move to most-recently-used tail.
  imageCache.delete(id);
  imageCache.set(id, entry);
  return entry.uri;
}

function cacheSet(id: string, uri: string | null): void {
  const existing = imageCache.get(id);
  if (existing !== undefined) {
    cacheTotalBytes -= existing.bytes;
    imageCache.delete(id);
  }
  const bytes = uri !== null ? uri.length : 0;
  imageCache.set(id, { uri, bytes });
  cacheTotalBytes += bytes;

  // Evict LRU entries until we are within budget.
  while (cacheTotalBytes > CACHE_BUDGET_BYTES && imageCache.size > 1) {
    const oldest = imageCache.keys().next().value;
    if (oldest === undefined) break;
    const evicted = imageCache.get(oldest)!;
    cacheTotalBytes -= evicted.bytes;
    imageCache.delete(oldest);
  }
}

/** Drop all cached thumbnails (call when the user clears history). */
export function clearImageCache(): void {
  imageCache.clear();
  cacheTotalBytes = 0;
  inflight.clear();
}

// ---------------------------------------------------------------------------
// Test-only exports — exposed solely for unit tests; not part of the public
// component API. The `__testOnly_` prefix signals this clearly.
//
// These are never imported by production code, so Rollup/Vite tree-shakes them
// out of the production bundle automatically (dead export elimination).
// Vitest imports them directly and runs in DEV mode where they are available.
// ---------------------------------------------------------------------------

/** Returns the current number of entries in the cache. */
export function __testOnly_cacheSize(): number {
  return imageCache.size;
}

/** Returns the byte-budget constant (CACHE_BUDGET_BYTES). */
export function __testOnly_cacheBudgetBytes(): number {
  return CACHE_BUDGET_BYTES;
}

/** Set a cache entry directly (test helper). */
export function __testOnly_cacheSet(id: string, uri: string | null): void {
  cacheSet(id, uri);
}

/** Get a cache entry directly (test helper). Returns undefined when absent. */
export function __testOnly_cacheGet(id: string): string | null | undefined {
  return cacheGet(id);
}

// ---------------------------------------------------------------------------
// fetchThumbnail — fetch the small thumbnail first; fall back to full-res
// when the daemon does not support get_item_thumbnail or returns null.
// Coalesces concurrent fetches for the same item id.
// ---------------------------------------------------------------------------

function fetchThumbnail(id: string): Promise<string | null> {
  // Resolved cache — fastest path.
  const cached = cacheGet(id);
  if (cached !== undefined) return Promise.resolve(cached);

  // Coalesce in-flight fetches.
  const existing = inflight.get(id);
  if (existing) return existing;

  const p = api
    .getItemThumbnail(id)
    .then(({ thumbnail }) => {
      if (thumbnail !== null) return thumbnail;
      // Daemon returned null thumbnail — fall back to full-res.
      return api.getItemImage(id).then(({ data_uri }) => data_uri);
    })
    .catch(() => {
      // get_item_thumbnail not supported by this daemon version — fall back.
      return api
        .getItemImage(id)
        .then(({ data_uri }) => data_uri)
        .catch(() => null as string | null);
    })
    .then((result) => {
      cacheSet(id, result);
      inflight.delete(id);
      return result;
    });

  inflight.set(id, p);
  return p;
}

// ---------------------------------------------------------------------------
// ImageThumb — lazy-loaded thumbnail with Maccy-parity scaling rules:
//
//   • Scale to fit within 340px × imageMaxHeight (uniform aspect-preserving).
//   • The smaller of (width-ratio, height-ratio) wins — no clipping.
//   • Never upscale: if the image is already smaller than the box, show at
//     natural size (CSS: max-width / max-height, no min-* forcing).
//   • CSS object-fit: contain; image-rendering: auto (high-quality downscale).
//   • On fetch failure renders a small placeholder instead of null (blank row).
//
// For list rows this component fetches the THUMBNAIL (small preview) and falls
// back to full-res only when the daemon doesn't support thumbnails. The detail
// modal (DetailsModal) uses getItemImage directly for full-resolution display.
// ---------------------------------------------------------------------------

interface ImageThumbProps {
  /** Clipboard item ID passed to api.getItemThumbnail (with full-res fallback). */
  id: string;
  /**
   * Height cap of the bounding box in px (the `imageMaxHeight` setting).
   * Width is always capped at 340 px to match Maccy's fixed column width.
   */
  maxHeight: number;
  /** Extra CSS classes forwarded to the <img> element. */
  className?: string;
}

// Sentinel distinct from null (= recorded miss) and undefined (= not cached).
const FETCH_FAILED = "__failed__";

export function ImageThumb({ id, maxHeight }: ImageThumbProps) {
  // Seed state from the resolved cache synchronously to avoid flicker on
  // re-mounts (same pattern as AppIcon).
  const [src, setSrc] = useState<string | null | typeof FETCH_FAILED>(() => {
    const cached = cacheGet(id);
    // Uncached (undefined) → null (loading; row height already reserved by the
    // virtualizer). A recorded miss (null) means a genuine fetch failure, so map
    // it to FETCH_FAILED and render the placeholder instead of staying blank —
    // otherwise a previously-failed row re-mounting (virtualized scroll) shows a
    // permanent blank because null is also the "still loading" state.
    if (cached === undefined) return null;
    return cached === null ? FETCH_FAILED : cached;
  });

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  useEffect(() => {
    // If already resolved, skip the fetch. A recorded miss (null) maps to the
    // FETCH_FAILED placeholder; only a truly uncached id (undefined) below falls
    // through to the loading state and a fresh fetch.
    const cached = cacheGet(id);
    if (cached !== undefined) {
      setSrc(cached === null ? FETCH_FAILED : cached);
      return;
    }

    // Reset to loading state while the fetch is in flight.
    setSrc(null);

    fetchThumbnail(id).then((result) => {
      if (!mountedRef.current) return;
      // null from fetchThumbnail means fetch failed — use sentinel so render shows
      // placeholder instead of staying blank (null = "still loading" ambiguity).
      setSrc(result ?? FETCH_FAILED);
    });
  }, [id]);

  // CopyPaste-8ebg.55: `.thumb-ph` (patterns.css) is a fixed 32x32 swatch, but
  // the real <img> below renders up to 340 x maxHeight — so the placeholder
  // was ~5x smaller than the thumbnail it stands in for, and the row visibly
  // grew/jumped the instant the fetch resolved. Sizing the placeholder inline
  // to the same box the real image will occupy (340 width cap, maxHeight cap)
  // reserves the final footprint up front without touching patterns.css.
  const placeholderStyle = { width: 340, height: maxHeight, maxWidth: "100%" };

  if (src === null) {
    // SCRH-11: Still loading — render a skeleton placeholder that occupies the
    // reserved row height so the row doesn't visually collapse while the fetch is
    // in flight. Previously returned null which left a blank gap in the list.
    return (
      <span
        className="thumb-ph"
        style={placeholderStyle}
        aria-label="Loading image…"
        aria-busy="true"
      />
    );
  }

  if (src === FETCH_FAILED) {
    // Fetch failed — render a small faint placeholder so the row isn't blank.
    return (
      <span
        className="thumb-ph thumb-ph--err"
        style={placeholderStyle}
        aria-label="Image unavailable"
        title="Image unavailable"
      />
    );
  }

  return (
    <img
      src={src}
      alt=""
      // Intrinsic decode-size hint (HB-10): the WebView decodes the data URI to
      // an RGBA bitmap whose RSS scales with the decoded pixel AREA, not with the
      // (small, LRU-capped) data-URI string length. Advertising the bounding-box
      // dimensions as the intrinsic width/height lets WebKit downsample at decode
      // time toward the displayed size rather than holding a larger source
      // bitmap. These attributes do not drive layout — they only steer the
      // decoder's target resolution (kept: functional decode-size hint, not styling).
      width={340}
      height={maxHeight}
      decoding="async"
    />
  );
}
