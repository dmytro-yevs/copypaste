import { useEffect, useRef, useState } from "react";
import { api } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Module-level cache: bundleId → base64 PNG string or null (null = no icon).
// Shared across all AppIcon instances so each bundle id is fetched exactly once.
//
// Bounded LRU (HB-10): previously unbounded, so over a long session every
// distinct source app's icon PNG accumulated forever. Cap at MAX_ICON_ENTRIES
// using Map insertion-order LRU — on access we delete-then-re-insert so the
// Map tail is the most-recently-used key and the head is the eviction
// candidate, mirroring the daemon-side icon cache and the ImageThumb cache.
// ---------------------------------------------------------------------------
const MAX_ICON_ENTRIES = 128;
const iconCache = new Map<string, string | null>();

// In-flight promise cache — prevents parallel duplicate fetches for the same id.
const inflight = new Map<string, Promise<string | null>>();

/** Read with LRU touch: move the entry to the MRU tail when present. */
function iconCacheGet(bundleId: string): string | null | undefined {
  if (!iconCache.has(bundleId)) return undefined;
  const value = iconCache.get(bundleId) ?? null;
  iconCache.delete(bundleId);
  iconCache.set(bundleId, value);
  return value;
}

/** Insert/update and evict the LRU head until within MAX_ICON_ENTRIES. */
function iconCacheSet(bundleId: string, value: string | null): void {
  if (iconCache.has(bundleId)) iconCache.delete(bundleId);
  iconCache.set(bundleId, value);
  while (iconCache.size > MAX_ICON_ENTRIES) {
    const oldest = iconCache.keys().next().value;
    if (oldest === undefined) break;
    iconCache.delete(oldest);
  }
}

function fetchIcon(bundleId: string): Promise<string | null> {
  // Return from the resolved cache first (fastest path).
  const cached = iconCacheGet(bundleId);
  if (cached !== undefined) {
    return Promise.resolve(cached);
  }
  // Coalesce concurrent fetches for the same bundle id.
  const existing = inflight.get(bundleId);
  if (existing) return existing;

  const p = api
    .getAppIcon(bundleId)
    .then((r) => r.png_b64 ?? null)
    .catch(() => null)
    .then((result) => {
      iconCacheSet(bundleId, result);
      inflight.delete(bundleId);
      return result;
    });

  inflight.set(bundleId, p);
  return p;
}

// ---------------------------------------------------------------------------
// AppIcon
//
// Renders a 14×14 px rounded app icon for the given bundleId.
// - Fetches lazily; shows nothing (no layout shift) while loading.
// - Falls back to nothing if the daemon returns null or fetch fails.
// - Guards against setState-after-unmount via a mounted ref.
// ---------------------------------------------------------------------------

interface AppIconProps {
  bundleId: string | null | undefined;
  /** Icon size in px. Defaults to 14. */
  size?: number;
}

export function AppIcon({ bundleId, size = 14 }: AppIconProps) {
  const [png, setPng] = useState<string | null>(() => {
    // Synchronous fast-path: if already cached, seed initial state directly
    // so there is no loading flicker on subsequent renders.
    if (!bundleId) return null;
    return iconCache.has(bundleId) ? (iconCache.get(bundleId) ?? null) : null;
  });

  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (!bundleId) return;
    // If already in cache from init, skip the fetch.
    if (iconCache.has(bundleId) && png === (iconCache.get(bundleId) ?? null)) return;

    fetchIcon(bundleId).then((result) => {
      if (mountedRef.current) {
        setPng(result);
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bundleId]);

  if (!png) return null;

  return (
    <img
      src={`data:image/png;base64,${png}`}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        borderRadius: 3,
        flexShrink: 0,
        objectFit: "cover",
        display: "inline-block",
        verticalAlign: "middle",
      }}
    />
  );
}
