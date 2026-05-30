import { useEffect, useRef, useState } from "react";
import { api, formatWallTime } from "../lib/ipc";
import type { SyncStatus, PairedDevice } from "../lib/ipc";

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
  sync: SyncStatus,
  lastSyncMs: number | null
): SyncState {
  // Determine "connected" by recency of last_sync_ms — works for both P2P and
  // Supabase paths since both update last_sync_ms on a successful round-trip.
  if (lastSyncMs !== null && Date.now() - lastSyncMs <= RECENT_SYNC_MS) {
    return "connected";
  }
  // Sync is configured but hasn't completed recently → idle.
  // We don't treat "not configured yet" as alarming (idle, not offline).
  if (sync.passphrase_set || sync.signed_in || sync.supabase_configured) {
    return "idle";
  }
  // Nothing configured — still idle, never red, to avoid alarm on fresh install.
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

  const refresh = async () => {
    // Fetch sync status and peer list concurrently; each fails independently.
    const [syncResult, peersResult] = await Promise.allSettled([
      api.getSyncStatus(),
      api.listPeers(),
    ]);

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

    setInfo({
      state: deriveSyncState(sync, lastSyncMs),
      deviceCount: peers.length,
      lastSyncMs,
      email: sync.email ?? null,
    });
  };

  useEffect(() => {
    cancelRef.current = false;
    void refresh();

    const id = setInterval(() => { void refresh(); }, POLL_INTERVAL_MS);
    return () => {
      cancelRef.current = true;
      clearInterval(id);
    };
  // refresh is a stable closure defined outside the effect; no deps required.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
