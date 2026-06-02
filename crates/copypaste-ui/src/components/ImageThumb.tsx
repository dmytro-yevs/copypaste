import { useEffect, useRef, useState } from "react";
import { api } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Shared image thumbnail cache — keyed by item id, value is the resolved data
// URI (or null when the fetch failed). Module-level so it survives re-mounts
// across HistoryView and PopupRow.
//
// Bounded LRU at IMAGE_CACHE_MAX entries: we delete-then-re-set on every
// access ("touch") so the Map insertion order reflects recency, and we evict
// the oldest (first) key when the cap is hit.
// ---------------------------------------------------------------------------

const IMAGE_CACHE_MAX = 100;
const imageCache = new Map<string, string | null>();

// In-flight promise cache — prevents parallel duplicate fetches for the same
// item id (e.g. popup + history both mount an ImageThumb for the same id).
// Mirrors the pattern used in AppIcon.tsx.
const inflight = new Map<string, Promise<string | null>>();

function cacheGet(id: string): string | null | undefined {
  if (!imageCache.has(id)) return undefined;
  const value = imageCache.get(id) as string | null;
  // Touch — move to most-recently-used tail.
  imageCache.delete(id);
  imageCache.set(id, value);
  return value;
}

function cacheSet(id: string, value: string | null): void {
  if (imageCache.has(id)) imageCache.delete(id);
  imageCache.set(id, value);
  while (imageCache.size > IMAGE_CACHE_MAX) {
    const oldest = imageCache.keys().next().value;
    if (oldest === undefined) break;
    imageCache.delete(oldest);
  }
}

/** Drop all cached thumbnails (call when the user clears history). */
export function clearImageCache(): void {
  imageCache.clear();
  inflight.clear();
}

// ---------------------------------------------------------------------------
// fetchImage — coalesces concurrent fetches for the same item id so that when
// popup + history both mount an ImageThumb for the same id only one IPC call
// is made. Mirrors AppIcon.tsx's fetchIcon pattern.
// ---------------------------------------------------------------------------

function fetchImage(id: string): Promise<string | null> {
  // Resolved cache — fastest path.
  const cached = cacheGet(id);
  if (cached !== undefined) return Promise.resolve(cached);

  // Coalesce in-flight fetches.
  const existing = inflight.get(id);
  if (existing) return existing;

  const p = api
    .getItemImage(id)
    .then(({ data_uri }) => data_uri)
    .catch(() => null as string | null)
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
// ---------------------------------------------------------------------------

interface ImageThumbProps {
  /** Clipboard item ID passed to api.getItemImage. */
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

export function ImageThumb({ id, maxHeight, className = "" }: ImageThumbProps) {
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

    fetchImage(id).then((result) => {
      if (!mountedRef.current) return;
      // null from fetchImage means fetch failed — use sentinel so render shows
      // placeholder instead of staying blank (null = "still loading" ambiguity).
      setSrc(result ?? FETCH_FAILED);
    });
  }, [id]);

  if (src === null) {
    // Still loading — render nothing (avoids layout shift; row height is already
    // reserved by the virtualizer).
    return null;
  }

  if (src === FETCH_FAILED) {
    // Fetch failed — render a small faint placeholder so the row isn't blank.
    return (
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: 48,
          height: Math.min(maxHeight, 32),
          borderRadius: 3,
          background: "var(--ide-elevated)",
          border: "1px solid var(--ide-divider)",
          flexShrink: 0,
        }}
        aria-label="Image unavailable"
        title="Image unavailable"
      >
        {/* Faint broken-image glyph */}
        <svg
          viewBox="0 0 16 16"
          width="12"
          height="12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ color: "var(--ide-faint)" }}
          aria-hidden="true"
        >
          <rect x="1.5" y="2.5" width="13" height="11" rx="1" />
          <path d="m1.5 11 3.5-3.5 2 2 2-2 4.5 4" strokeDasharray="2 2" />
          <line x1="10" y1="2.5" x2="10" y2="13.5" strokeDasharray="2 2" />
        </svg>
      </span>
    );
  }

  return (
    <img
      src={src}
      alt=""
      // max-width/max-height + object-fit:contain implements the Maccy bounding
      // box: the image shrinks to fit but is never upscaled past its natural size.
      style={{
        maxWidth: 340,
        maxHeight: maxHeight,
        width: "auto",
        height: "auto",
        objectFit: "contain",
        // "auto" lets the browser choose the highest-quality resampling algorithm
        // (typically Lanczos/Mitchell on modern engines) rather than nearest-neighbor.
        imageRendering: "auto",
        display: "block",
        flexShrink: 0,
        borderRadius: 2,
      }}
      className={className}
    />
  );
}
