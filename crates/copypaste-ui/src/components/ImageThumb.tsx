import { useEffect, useState } from "react";
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
}

// ---------------------------------------------------------------------------
// ImageThumb — lazy-loaded thumbnail with Maccy-parity scaling rules:
//
//   • Scale to fit within 340px × imageMaxHeight (uniform aspect-preserving).
//   • The smaller of (width-ratio, height-ratio) wins — no clipping.
//   • Never upscale: if the image is already smaller than the box, show at
//     natural size (CSS: max-width / max-height, no min-* forcing).
//   • CSS object-fit: contain; image-rendering: auto (high-quality downscale).
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

export function ImageThumb({ id, maxHeight, className = "" }: ImageThumbProps) {
  const cached = cacheGet(id);
  const [src, setSrc] = useState<string | null>(cached ?? null);

  useEffect(() => {
    // Skip if already in cache (hit or recorded miss).
    if (cacheGet(id) !== undefined) return;

    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        cacheSet(id, data_uri);
        setSrc(data_uri);
      })
      .catch(() => {
        // Record miss so we don't retry on every render.
        cacheSet(id, null);
      });
  }, [id]);

  if (!src) return null;

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
