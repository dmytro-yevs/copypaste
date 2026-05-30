import { useEffect, useRef, useState } from "react";
import { api } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Module-level cache: bundleId → base64 PNG string or null (null = no icon).
// Shared across all AppIcon instances so each bundle id is fetched exactly once.
// ---------------------------------------------------------------------------
const iconCache = new Map<string, string | null>();

// In-flight promise cache — prevents parallel duplicate fetches for the same id.
const inflight = new Map<string, Promise<string | null>>();

function fetchIcon(bundleId: string): Promise<string | null> {
  // Return from the resolved cache first (fastest path).
  if (iconCache.has(bundleId)) {
    return Promise.resolve(iconCache.get(bundleId) ?? null);
  }
  // Coalesce concurrent fetches for the same bundle id.
  const existing = inflight.get(bundleId);
  if (existing) return existing;

  const p = api
    .getAppIcon(bundleId)
    .then((r) => r.png_b64 ?? null)
    .catch(() => null)
    .then((result) => {
      iconCache.set(bundleId, result);
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
