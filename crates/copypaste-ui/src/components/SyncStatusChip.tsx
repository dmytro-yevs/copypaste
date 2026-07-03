import { useCallback, useEffect, useRef, useState } from "react";
import { api, formatWallTime } from "../lib/ipc";
import type { PairedDevice, SyncBadgeState } from "../lib/ipc";

// ---------------------------------------------------------------------------
// SyncState — canonical six-state display model (CMP-7 parity with Android)
// ---------------------------------------------------------------------------

/**
 * CMP-7: SyncState now mirrors the full IPC SyncBadgeState set so each
 * daemon-reported state has a 1:1 internal representation. Previously the
 * web component collapsed six states to three ("connected"/"idle"/"offline"),
 * losing the distinction between syncing vs synced, idle vs misconfigured,
 * and offline vs error.
 *
 * Mapping to dot colours (three colours, six states):
 *   - "synced"        → green  (bg-ide-success)   — recent successful exchange
 *   - "syncing"       → green  (bg-ide-success)   — in-flight (same as synced visually)
 *   - "idle"          → grey   (bg-ide-faint)      — configured, no recent activity
 *   - "misconfigured" → grey   (bg-ide-faint)      — incomplete setup (amber chip separately)
 *   - "offline"       → red    (bg-ide-danger)     — daemon cannot reach sync backend
 *   - "error"         → red    (bg-ide-danger)     — backend returned auth/RLS/relay error
 *
 * Android parity note (CMP-7): Android uses a 4-state sealed class
 * (Connected, Idle, NetworkOffline, DaemonUnreachable). macOS does NOT
 * distinguish NetworkOffline vs DaemonUnreachable because the web side has
 * no reliable OS-network signal (navigator.onLine is not used per CopyPaste-5qbe).
 * Both "offline" and "error" map to red and share the same label semantics as
 * Android's DaemonUnreachable/NetworkOffline. The "syncing" state has no direct
 * Android counterpart (Android uses isSyncing=true to gate the Connected branch);
 * on web it maps to the same green dot as "synced". A future parity pass should
 * reconcile: Android → add "syncing" distinction; web → consider NetworkOffline
 * if a platform API for OS-level connectivity is ever added.
 */
type SyncState =
  | "synced"
  | "syncing"
  | "idle"
  | "misconfigured"
  | "offline"
  | "error";

interface SyncInfo {
  state: SyncState;
  deviceCount: number;
  lastSyncMs: number | null;
  email: string | null;
  /**
   * PG-44 / CopyPaste-k1jo: true when a Supabase URL is configured but the
   * cloud sync is not working (supabase_configured===false or !signed_in).
   * Android parity: shows a badge chip rather than tooltip-only.
   * Note: when state is "misconfigured", cloudMisconfig will typically also
   * be true, but the amber chip is driven by this field independently.
   */
  cloudMisconfig: boolean;
  /**
   * CopyPaste-8ebg.26: count of paired peers that individually look stalled
   * (see `isPeerStalled`), even though the overall `state` above is derived
   * from the daemon's *global* `badge_state` and can stay green as long as
   * ONE peer is healthy. Surfaced as a separate warning pill so a peer with
   * a broken key (e.g. rekey failures — see `fanout.rs` `RekeyOutcome::Failed`)
   * does not silently hide behind a healthy sibling peer.
   */
  stalledPeerCount: number;
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

/**
 * Polling interval — 2 s so offline is reflected within one poll cycle.
 *
 * CopyPaste-f701: the old 10 s interval caused the chip to show a stale
 * "connected" (green) for up to 10 s after the daemon went offline.
 * Exported so tests can assert the upper bound without importing the React
 * component tree.
 */
export const SYNC_POLL_INTERVAL_MS = 2_000;

/**
 * Peer-count polling interval — 10 s, matching usePairedDevices (PEERS_POLL_MS).
 *
 * CopyPaste-crh3.48: the old implementation called listPeers() on every 2s
 * sync-status poll (30 IPC calls/min), even though peer count changes rarely.
 * Decoupling to a separate 10s poll reduces calls to ≤6/min.
 * Exported so tests can assert the upper bound.
 */
export const PEERS_POLL_INTERVAL_MS = 10_000;

/**
 * CopyPaste-8ebg.26: how stale an individual peer's `last_sync_at` must be
 * (in ms) before it is flagged as "stalled" in the per-peer warning pill.
 *
 * Deliberately much longer than `SYNC_BADGE_RECENT_MS` (5 min, used for the
 * *global* badge dot): brief offline blips are normal and must not spam a
 * warning. This threshold targets the audit scenario — a peer with a broken
 * sync key that silently receives nothing for a long time while the badge
 * stays green because a different peer is healthy.
 */
export const PEER_STALL_THRESHOLD_MS = 30 * 60 * 1000; // 30 minutes

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * CopyPaste-8ebg.26: per-peer stall predicate — the piece the global badge
 * cannot express. A peer counts as stalled when:
 *   - it has synced before but `last_sync_at` is older than
 *     `PEER_STALL_THRESHOLD_MS`, or
 *   - it has NEVER synced (`last_sync_at === null`) despite having been
 *     paired (`added_at`) longer than the same threshold — covers a broken
 *     key/handshake that never produced a single successful exchange.
 *
 * Freshly-paired peers (within the threshold) are never flagged — pairing
 * and the first catch-up replay legitimately take a little while.
 */
export function isPeerStalled(peer: PairedDevice, nowMs: number): boolean {
  if (peer.last_sync_at !== null) {
    return nowMs - peer.last_sync_at * 1000 > PEER_STALL_THRESHOLD_MS;
  }
  if (peer.added_at > 0) {
    return nowMs - peer.added_at * 1000 > PEER_STALL_THRESHOLD_MS;
  }
  // No last_sync_at and no known added_at — nothing to compare against.
  return false;
}

/**
 * Map the daemon's canonical `SyncBadgeState` to the component's internal
 * `SyncState`. CMP-7: now a 1:1 identity mapping — SyncState was expanded to
 * match the full IPC state set, so no folding is needed.
 *
 * Consumers must call this when `badge_state` is present in the IPC response.
 * Do NOT re-derive from raw fields when badge_state is available.
 */
export function badgeStateToSyncState(badge: SyncBadgeState): SyncState {
  // CMP-7: 1:1 — SyncState now mirrors SyncBadgeState exactly.
  // No folding: each IPC state has its own internal representation so future
  // callers can branch on the precise state (e.g. show "Syncing…" label).
  return badge;
}

/**
 * Fallback derivation for daemons that predate `badge_state`.
 *
 * @deprecated Use `badgeStateToSyncState(sync.badge_state)` when `badge_state`
 * is present in the IPC response. This function is only called when `badge_state`
 * is absent (daemon version predating CopyPaste-merc).
 *
 * CMP-7: Returns the expanded SyncState. Fallback path can only distinguish
 * "synced" (recent round-trip) from "idle" (no recent activity) — it cannot
 * produce "syncing", "misconfigured", "offline", or "error" since those require
 * the daemon-computed badge_state. Both the fallback "synced" and "idle" states
 * are valid members of the expanded SyncState type.
 */
function deriveSyncStateFallback(
  lastSyncMs: number | null,
  deviceCount: number
): SyncState {
  // "synced" (green dot) means at least one device is online/syncing. We
  // require BOTH a paired device AND a recent last_sync_ms: a recent sync
  // round-trip is the only liveness signal the daemon exposes today, and it is
  // updated by both the P2P and Supabase paths on a successful exchange. With
  // zero paired devices there is nothing to be "connected" to, so we stay grey.
  const recentSync =
    lastSyncMs !== null && Date.now() - lastSyncMs <= RECENT_SYNC_MS_FALLBACK;
  if (deviceCount > 0 && recentSync) {
    return "synced";
  }
  // Paired/configured but no recent round-trip, or zero devices → idle (grey).
  // We never show red here for a missing sync, to avoid alarm on a fresh
  // install or while a peer is simply offline.
  return "idle";
}

/** Build the tooltip string. */
function buildTooltip(info: SyncInfo): string {
  const parts: string[] = [];

  // CMP-7: "offline" and "error" are now distinct states but both indicate
  // hard sync failures — show "Background service unreachable" for both (same as
  // Android DaemonUnreachable/NetworkOffline; bdac.45: jargon "Daemon" removed).
  // When the component is offline (IPC socket rejected), state is "offline";
  // when daemon reports a backend error (auth/RLS/relay), state is "error".
  if (info.state === "offline" || info.state === "error") {
    parts.push("Background service unreachable");
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

  // CopyPaste-8ebg.26: call out stalled peers explicitly — this is exactly
  // the case the global `state` above cannot express (it can read "synced"
  // while one paired peer has been silently receiving nothing for a while).
  if (info.stalledPeerCount > 0) {
    parts.push(
      `${info.stalledPeerCount} peer${info.stalledPeerCount !== 1 ? "s" : ""} not syncing`
    );
  }

  return parts.join(" · ");
}

/**
 * CMP-7: "connected" in the old 3-state model (synced|syncing → green).
 * Used by the pulse-on-connect logic: we want to pulse when transitioning
 * INTO any green state, not just the old "connected" label.
 */
function isConnectedState(state: SyncState): boolean {
  return state === "synced" || state === "syncing";
}

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
    stalledPeerCount: 0,
  });

  // VISM-12: track previous state so we can detect the transition INTO
  // a "connected" (green) state and trigger the one-shot .online-pulse class.
  // We use a ref (not state) to avoid an extra render cycle on every poll.
  // CMP-7: initial prev is "idle" (not "connected"); green states are synced/syncing.
  const prevStateRef = useRef<SyncState>("idle");
  // Controls whether the one-shot .online-pulse class is applied to the dot.
  // Set to true on connect transition; cleared after the 2 s animation completes.
  const [pulsing, setPulsing] = useState(false);

  // cancelRef prevents setState after unmount when the poll fires mid-flight.
  const cancelRef = useRef(false);

  // CopyPaste-crh3.48: peer count is decoupled from the 2s sync-status poll.
  // peerCountRef caches the last known paired-device count so refreshSync can
  // read it without calling listPeers on every tick. Updated by refreshPeers
  // which runs on its own 10s interval (matching usePairedDevices cadence).
  const peerCountRef = useRef(0);

  // Stable sync-status poll (2s). Reads peerCountRef for deviceCount so it no
  // longer calls listPeers — reducing listPeers from 30/min to ≤6/min.
  // cancelRef is a ref, so it does not need to be listed as a dep.
  const refreshSync = useCallback(async () => {
    const syncResult = await Promise.allSettled([api.getSyncStatus()]);

    // Guard against setting state after the component has unmounted or after
    // the effect has been torn down and re-run (e.g. React StrictMode double
    // invoke). cancelRef is reset to false at the top of each effect run.
    if (cancelRef.current) return;

    // If the sync-status call itself failed → daemon is offline (IPC socket down).
    // CMP-7: use "offline" (IPC unreachable) — not "error" (backend error).
    if (syncResult[0].status === "rejected") {
      setInfo({ state: "offline", deviceCount: 0, lastSyncMs: null, email: null, cloudMisconfig: false, stalledPeerCount: 0 });
      return;
    }

    const sync = syncResult[0].value;
    const lastSyncMs = sync.last_sync_ms ?? null;
    // Use the cached peer count — updated independently by refreshPeers.
    const deviceCount = peerCountRef.current;

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

    // VISM-12: detect transition INTO a connected (green) state so we fire the
    // one-shot .online-pulse only on the leading edge, not every poll tick.
    // CMP-7: "connected" is now "synced" or "syncing" — use isConnectedState().
    const prevState = prevStateRef.current;
    if (isConnectedState(state) && !isConnectedState(prevState)) {
      setPulsing(true);
    }
    prevStateRef.current = state;

    // Functional update: refreshSync (2s) must not clobber stalledPeerCount,
    // which is computed independently by refreshPeers (10s) — same pattern
    // refreshPeers already uses in reverse to avoid clobbering `state`.
    setInfo((prev) => ({
      state,
      deviceCount,
      lastSyncMs,
      email: sync.email ?? null,
      cloudMisconfig,
      stalledPeerCount: prev.stalledPeerCount,
    }));
  }, []); // no external deps — api is module-level stable, setInfo/peerCountRef are stable

  // Stable peer-count poll (10s). Decoupled from the 2s sync-status poll so
  // listPeers is called at most 6×/min instead of 30×/min (CopyPaste-crh3.48).
  // On success, writes peerCountRef and patches info.deviceCount in place so
  // the chip reflects paired-device count changes without waiting for the next
  // refreshSync tick.
  const refreshPeers = useCallback(async () => {
    try {
      const { peers } = await api.listPeers();
      if (cancelRef.current) return;
      const count = peers.length;
      peerCountRef.current = count;
      // CopyPaste-8ebg.26: per-peer stall check, independent of the global
      // badge_state — a single healthy peer must not hide a stalled one.
      const nowMs = Date.now();
      const stalledPeerCount = peers.reduce(
        (n: number, peer: PairedDevice) => (isPeerStalled(peer, nowMs) ? n + 1 : n),
        0
      );
      // Patch only deviceCount/stalledPeerCount — leave all other SyncInfo fields intact.
      setInfo((prev) => ({ ...prev, deviceCount: count, stalledPeerCount }));
    } catch {
      // Keep last known count on error — daemon may be briefly unavailable.
      // refreshSync will flip to "offline" if getSyncStatus also fails.
    }
  }, []); // no external deps

  useEffect(() => {
    cancelRef.current = false;
    // Fire both immediately on mount to populate state before the first interval tick.
    void refreshSync();
    void refreshPeers();

    const syncId = setInterval(() => { void refreshSync(); }, SYNC_POLL_INTERVAL_MS);
    const peersId = setInterval(() => { void refreshPeers(); }, PEERS_POLL_INTERVAL_MS);
    return () => {
      cancelRef.current = true;
      clearInterval(syncId);
      clearInterval(peersId);
    };
  }, [refreshSync, refreshPeers]); // both are stable (useCallback with no deps) → runs once

  const tooltip = buildTooltip(info);

  // Runtime state → status token (the token itself is a design constant; the
  // choice is state-driven, so it is set as an inline CSS-var-backed value).
  const dotColor = isConnectedState(info.state)
    ? "var(--ok)"
    : info.state === "offline" || info.state === "error"
      ? "var(--err)"
      : "var(--faint)";

  // CopyPaste-8ebg.52: the daemon dying mid-session (IPC socket rejected, or
  // the daemon reporting a hard backend error) was previously represented
  // only by a 6px dot flipping to red plus a hover-only tooltip — easy to
  // miss entirely, especially since the chip lives in a corner of the chrome.
  // Surface it as a visible banner too, reusing the existing `.banner`
  // pattern (patterns.css) already used for the same class of message
  // elsewhere (StatusBanners.tsx) — no new CSS.
  const daemonDown = info.state === "offline" || info.state === "error";

  return (
    <>
      <div
        className="chip"
        title={tooltip}
        aria-label={`Sync: ${info.state}. ${tooltip}`}
      >
        {/* Coloured status dot; one-shot .online-pulse on transition INTO a green state (VISM-12).
            CMP-7: green states are "synced" and "syncing" (isConnectedState).
            1jms.27: when state is "syncing", apply .syncing-dot (gentle opacity
            breathe) to distinguish an active in-flight sync from a completed one.
            The .online-pulse class is removed after the 2 s animation ends to allow re-trigger.
            Note: .online-pulse and .syncing-dot share the same element; in the
            brief overlap at connect-time, online-pulse (forwards) takes visual
            precedence then ends, leaving syncing-dot if still syncing. */}
        <span
          className="dot"
          style={{ background: dotColor }}
          data-pulsing={pulsing}
          onAnimationEnd={() => setPulsing(false)}
        />
        {/* Device count — only shown when at least one peer is paired */}
        {info.deviceCount > 0 && (
          <span>
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
          >
            Misconfig
          </span>
        )}
        {/* CopyPaste-8ebg.26: per-peer stall warning — rendered independently
            of `dotColor`/`info.state` so it stays visible even when the
            global badge is green because a DIFFERENT peer is healthy. */}
        {info.stalledPeerCount > 0 && (
          <span
            aria-label={`${info.stalledPeerCount} peer${info.stalledPeerCount !== 1 ? "s" : ""} not syncing`}
          >
            ⚠ {info.stalledPeerCount}
          </span>
        )}
      </div>
      {daemonDown && (
        <div className="banner banner--err" role="alert" data-testid="sync-daemon-down-banner">
          <span className="banner__x">Background service unreachable — clipboard sync paused.</span>
        </div>
      )}
    </>
  );
}
