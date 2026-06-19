import { useCallback, useEffect, useRef, useState } from "react";
import { api, formatWallTime } from "../lib/ipc";
import type { PairedDevice, SyncBadgeState } from "../lib/ipc";

// ---------------------------------------------------------------------------
// SyncState — the three display conditions we surface
// ---------------------------------------------------------------------------

type SyncState = "connected" | "idle" | "offline";

interface SyncInfo {
  state: SyncState;
  deviceCount: number;
  lastSyncMs: number | null;
  email: string | null;
  /**
   * PG-44 / CopyPaste-k1jo: true when a Supabase URL is configured but the
   * cloud sync is not working (supabase_configured===false or !signed_in).
   * Android parity: shows a badge chip rather than tooltip-only.
   */
  cloudMisconfig: boolean;
}

/**
 * How recent a last_sync_ms must be (in ms) to count as "connected/working".
 *
 * @deprecated CopyPaste-merc: this constant is a LOCAL FALLBACK ONLY. When the
 * daemon is new enough to emit `badge_state` in get_sync_status, this constant
 * is NOT used — the daemon-computed value is consumed directly. This code path
 * runs only against daemons that predate the `badge_state` field.
 *
 * The canonical threshold is `SYNC_BADGE_RECENT_MS` in copypaste-ipc (5 min),
 * which the daemon uses to compute `badge_state`.
 */
const RECENT_SYNC_MS_FALLBACK = 5 * 60 * 1000; // 5 minutes — FALLBACK ONLY

/** Polling interval. 10 s is light enough to be invisible. */
const POLL_INTERVAL_MS = 10_000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Map the daemon's canonical `SyncBadgeState` to the component's internal
 * `SyncState`. This is a THIN ADAPTER — the mapping is straightforward because
 * the canonical states are richer than the three-way display model.
 *
 * Consumers must call this when `badge_state` is present in the IPC response.
 * Do NOT re-derive from raw fields when badge_state is available.
 */
export function badgeStateToSyncState(badge: SyncBadgeState): SyncState {
  switch (badge) {
    case "synced":
    case "syncing":
      return "connected";
    case "offline":
    case "error":
      return "offline";
    case "idle":
    case "misconfigured":
    default:
      return "idle";
  }
}

/**
 * Fallback derivation for daemons that predate `badge_state`.
 *
 * @deprecated Use `badgeStateToSyncState(sync.badge_state)` when `badge_state`
 * is present in the IPC response. This function is only called when `badge_state`
 * is absent (daemon version predating CopyPaste-merc).
 */
function deriveSyncStateFallback(
  lastSyncMs: number | null,
  deviceCount: number
): SyncState {
  // "connected" (green dot) means at least one device is online/syncing. We
  // require BOTH a paired device AND a recent last_sync_ms: a recent sync
  // round-trip is the only liveness signal the daemon exposes today, and it is
  // updated by both the P2P and Supabase paths on a successful exchange. With
  // zero paired devices there is nothing to be "connected" to, so we stay grey.
  const recentSync =
    lastSyncMs !== null && Date.now() - lastSyncMs <= RECENT_SYNC_MS_FALLBACK;
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
    cloudMisconfig: false,
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
      setInfo({ state: "offline", deviceCount: 0, lastSyncMs: null, email: null, cloudMisconfig: false });
      return;
    }

    const sync = syncResult.value;
    const peers: PairedDevice[] =
      peersResult.status === "fulfilled" ? peersResult.value.peers : [];

    const lastSyncMs = sync.last_sync_ms ?? null;
    const deviceCount = peers.length;

    // PG-44 / CopyPaste-k1jo: cloud misconfig = supabase URL is set but the
    // cloud sync layer is not properly configured (anon key missing, or auth
    // credentials not provided). Daemon surfaces this as supabase_url non-empty
    // but supabase_configured===false. Matches Android badge-chip behaviour.
    const cloudMisconfig =
      !!sync.supabase_url && !sync.supabase_configured;

    // CopyPaste-merc: use the daemon-computed canonical badge state when present.
    // This is the single source of truth — no local re-derivation from raw fields.
    // Fall back to the old derivation only for daemons predating badge_state.
    let state: SyncState;
    if (sync.badge_state != null) {
      // New path: daemon computed the state — consume it directly.
      state = badgeStateToSyncState(sync.badge_state);
    } else {
      // Legacy fallback: daemon is old; derive locally as before.
      state = deriveSyncStateFallback(lastSyncMs, deviceCount);
    }

    setInfo({
      state,
      deviceCount,
      lastSyncMs,
      email: sync.email ?? null,
      cloudMisconfig,
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
      {/* PG-44 / CopyPaste-k1jo: visible cloud-misconfig chip (Android parity).
          Android shows a badge when cloud sync is misconfigured; macOS was
          tooltip-only. Show a compact warning pill so the state is visible
          without hovering. Only rendered when supabase_url is set but the
          daemon reports supabase_configured===false. */}
      {info.cloudMisconfig && (
        <span
          aria-label="Cloud sync misconfigured"
          className="shrink-0 rounded-full border border-ide-warning/30 bg-ide-warning/14 px-1.5 py-0.5 text-[10px] font-medium text-ide-warning"
        >
          Misconfig
        </span>
      )}
    </div>
  );
}
