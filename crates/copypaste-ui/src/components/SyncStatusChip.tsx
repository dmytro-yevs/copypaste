import { useCallback, useEffect, useRef, useState } from "react";
import { api, formatWallTime } from "../lib/ipc";
import type { PairedDevice } from "../lib/ipc";

// ---------------------------------------------------------------------------
// SyncState — the three conditions we surface
// ---------------------------------------------------------------------------

type SyncState = "connected" | "idle" | "offline";

interface SyncInfo {
  state: SyncState;
  deviceCount: number;
  lastSyncMs: number | null;
  email: string | null;
}

/** How recent a last_sync_ms must be (in ms) to count as "connected/working". */
const RECENT_SYNC_MS = 5 * 60 * 1000; // 5 minutes

/** Polling interval. 10 s is light enough to be invisible. */
const POLL_INTERVAL_MS = 10_000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function deriveSyncState(
  lastSyncMs: number | null,
  deviceCount: number
): SyncState {
  // "connected" (green dot) means at least one device is online/syncing. We
  // require BOTH a paired device AND a recent last_sync_ms: a recent sync
  // round-trip is the only liveness signal the daemon exposes today, and it is
  // updated by both the P2P and Supabase paths on a successful exchange. With
  // zero paired devices there is nothing to be "connected" to, so we stay grey.
  const recentSync =
    lastSyncMs !== null && Date.now() - lastSyncMs <= RECENT_SYNC_MS;
  if (deviceCount > 0 && recentSync) {
    return "connected";
  }
  // Paired/configured but no recent round-trip, or zero devices → idle (grey).
  // We never show red here for a missing sync, to avoid alarm on a fresh
  // install or while a peer is simply offline.
  return "idle";
}

/** Build the tooltip string. */
function buildTooltip(info: SyncInfo): string {
  const parts: string[] = [];

  if (info.state === "offline") {
    parts.push("Daemon unreachable");
  } else if (info.lastSyncMs !== null) {
    parts.push(`Last sync: ${formatWallTime(info.lastSyncMs)}`);
  } else {
    parts.push("No sync yet");
  }

  if (info.deviceCount > 0) {
    parts.push(
      `${info.deviceCount} paired device${info.deviceCount !== 1 ? "s" : ""}`
    );
  } else {
    parts.push("No paired devices");
  }

  if (info.email) {
    parts.push(info.email);
  }

  return parts.join(" · ");
}

// ---------------------------------------------------------------------------
// Dot colours — keyed to the ide-* Tailwind palette
// ---------------------------------------------------------------------------

const DOT_CLASS: Record<SyncState, string> = {
  connected: "bg-ide-success",  // #5fad65 green
  idle:      "bg-ide-faint",    // #6f737a grey
  offline:   "bg-ide-danger",   // #db5c5c red
};

// ---------------------------------------------------------------------------
// SyncStatusChip
// ---------------------------------------------------------------------------

export function SyncStatusChip() {
  const [info, setInfo] = useState<SyncInfo>({
    state: "idle",
    deviceCount: 0,
    lastSyncMs: null,
    email: null,
  });

  // cancelRef prevents setState after unmount when the poll fires mid-flight.
  const cancelRef = useRef(false);

  // Stable callback — useCallback ensures the effect dep array sees a constant
  // reference, so the interval is set up exactly once while the component is
  // mounted. cancelRef is a ref, so it doesn't need to be listed as a dep.
  const refresh = useCallback(async () => {
    // Fetch sync status and peer list concurrently; each fails independently.
    const [syncResult, peersResult] = await Promise.allSettled([
      api.getSyncStatus(),
      api.listPeers(),
    ]);

    // Guard against setting state after the component has unmounted or after
    // the effect has been torn down and re-run (e.g. React StrictMode double
    // invoke). cancelRef is reset to false at the top of each effect run.
    if (cancelRef.current) return;

    // If the sync-status call itself failed → daemon is offline.
    if (syncResult.status === "rejected") {
      setInfo({ state: "offline", deviceCount: 0, lastSyncMs: null, email: null });
      return;
    }

    const sync = syncResult.value;
    const peers: PairedDevice[] =
      peersResult.status === "fulfilled" ? peersResult.value.peers : [];

    const lastSyncMs = sync.last_sync_ms ?? null;
    const deviceCount = peers.length;

    setInfo({
      state: deriveSyncState(lastSyncMs, deviceCount),
      deviceCount,
      lastSyncMs,
      email: sync.email ?? null,
    });
  }, []); // no external deps — api is module-level stable, setInfo is stable

  useEffect(() => {
    cancelRef.current = false;
    void refresh();

    const id = setInterval(() => { void refresh(); }, POLL_INTERVAL_MS);
    return () => {
      cancelRef.current = true;
      clearInterval(id);
    };
  }, [refresh]); // refresh is stable (useCallback with no deps) → runs once

  const tooltip = buildTooltip(info);

  return (
    <div
      title={tooltip}
      aria-label={`Sync: ${info.state}. ${tooltip}`}
      className="flex items-center gap-1.5 cursor-default select-none"
    >
      {/* Coloured status dot; pulses when actively syncing */}
      <span
        className={[
          "h-2 w-2 shrink-0 rounded-full",
          DOT_CLASS[info.state],
          info.state === "connected" ? "animate-pulse" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      />
      {/* Device count — only shown when at least one peer is paired */}
      {info.deviceCount > 0 && (
        <span className="text-[10px] leading-none text-ide-faint tabular-nums">
          {info.deviceCount}
        </span>
      )}
    </div>
  );
}
