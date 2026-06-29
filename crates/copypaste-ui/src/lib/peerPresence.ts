/**
 * Global peer-presence store.
 *
 * Holds a live map from peer fingerprint → online status, updated by polling
 * `poll_peer_events` roughly every second.  The polling loop runs once,
 * app-globally, started by `startPeerPresencePolling()` which is called from
 * `App.tsx` on mount.  Individual components (`DevicesView`) subscribe via
 * `usePeerPresence()` so the online dots update without reopening the Devices
 * page.
 *
 * Design:
 * - The daemon's `poll_peer_events` IPC method returns events queued since the
 *   last drain.  A `connected` event sets the fingerprint online; a
 *   `disconnected` event sets it offline.
 * - Presence entries expire after PEER_PRESENCE_TTL_MS without a fresh
 *   `connected` event — stale peers flip to offline even if no explicit
 *   `disconnected` was received (e.g. after a daemon restart/outage).
 *   SCRD-3 / SYNC-5 fix: the old append-only design left entries permanently
 *   green after a daemon outage.
 * - On error (daemon offline), the loop backs off silently; the fallback is
 *   the existing 10 s `list_peers` poll in `DevicesView`.
 */

import { create } from "zustand";
import { api, IpcError } from "./ipc";

// ---------------------------------------------------------------------------
// Freshness window
// ---------------------------------------------------------------------------

/**
 * How long a peer's `connected` event stays fresh before we consider it
 * offline again.  Chosen as 3× POLL_INTERVAL_MS (5 s) so a single missed
 * poll tick does not immediately flip the dot, but a daemon restart (which
 * stops emitting events) causes a flip within ~15 s.
 *
 * SCRD-3 / SYNC-5: prevents stale `online=true` entries surviving indefinitely
 * after a daemon outage or restart.
 */
export const PEER_PRESENCE_TTL_MS = 15_000;

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface PeerPresenceState {
  /**
   * Map from canonical fingerprint → tri-state:
   *   true   — explicit `connected` event received within PEER_PRESENCE_TTL_MS.
   *   false  — explicit `disconnected` event received (or daemon restart via resetAllOffline).
   *   absent — either never seen, OR stale (TTL expired via expireStale).
   *            Consumers MUST fall back to the daemon's `list_peers` truth (`peer.online`)
   *            when the key is absent.
   *
   * Only read this through the fall-back expression used in DevicesView:
   *   `const live = presenceOnline[fp]; return live !== undefined ? live : peer.online === true;`
   */
  online: Record<string, boolean>;
  /**
   * Map from canonical fingerprint → wall-clock ms when the last `connected`
   * event was received.  Used to expire stale online entries.
   */
  seenAt: Record<string, number>;
  /** Apply a batch of events from `poll_peer_events`. */
  applyEvents: (
    events: Array<{ kind: "connected" | "disconnected"; fingerprint: string }>,
  ) => void;
  /**
   * Expire any peer whose last-seen timestamp is older than PEER_PRESENCE_TTL_MS.
   * Called periodically by the polling loop so stale entries don't stay green
   * forever after a daemon outage or restart.
   *
   * 5917.11 tri-state fix: expired `connected` entries are DELETED from the map
   * (not set to `false`) so consumers fall back to the daemon's `list_peers` truth
   * (`peer.online`) instead of lying Offline.  An explicit `disconnected` event
   * (false) is never expired — only `true` entries are eligible.
   */
  expireStale: () => void;
  /**
   * Reset all peers to offline — called when the polling loop re-initialises
   * after detecting a daemon restart so the dots flip immediately rather than
   * waiting for TTL expiry.
   */
  resetAllOffline: () => void;
}

export const usePeerPresence = create<PeerPresenceState>()((set) => ({
  online: {},
  seenAt: {},
  applyEvents(events) {
    if (events.length === 0) return;
    const now = Date.now();
    set((state) => {
      const nextOnline = { ...state.online };
      const nextSeenAt = { ...state.seenAt };
      for (const ev of events) {
        nextOnline[ev.fingerprint] = ev.kind === "connected";
        if (ev.kind === "connected") {
          nextSeenAt[ev.fingerprint] = now;
        }
      }
      return { online: nextOnline, seenAt: nextSeenAt };
    });
  },
  expireStale() {
    const now = Date.now();
    set((state) => {
      let changed = false;
      const nextOnline = { ...state.online };
      const nextSeenAt = { ...state.seenAt };
      for (const fp of Object.keys(nextOnline)) {
        // Only expire entries that are explicitly true (connected).
        // false = explicit disconnect — never expire; absent = already unknown.
        if (nextOnline[fp] === true) {
          const last = state.seenAt[fp] ?? 0;
          if (now - last > PEER_PRESENCE_TTL_MS) {
            // 5917.11: DELETE the entry (tri-state unknown/absent) so consumers
            // fall back to daemon list_peers truth rather than forcing Offline.
            delete nextOnline[fp];
            delete nextSeenAt[fp];
            changed = true;
          }
        }
      }
      return changed ? { online: nextOnline, seenAt: nextSeenAt } : state;
    });
  },
  resetAllOffline() {
    set((state) => {
      const hasAnyOnline = Object.values(state.online).some(Boolean);
      if (!hasAnyOnline) return state;
      const nextOnline: Record<string, boolean> = {};
      for (const fp of Object.keys(state.online)) {
        nextOnline[fp] = false;
      }
      return { online: nextOnline };
    });
  },
}));

// ---------------------------------------------------------------------------
// Polling loop (started once, app-globally)
// ---------------------------------------------------------------------------

/**
 * How often to drain the daemon's peer-event queue when peers are paired.
 * s7ia B3: backed down from 1 s to 5 s — peers reconnect fast enough at 5 s
 * and the previous 1 s rate fired 60 IPC calls/min when no devices were paired.
 */
const POLL_INTERVAL_MS = 5_000;

/**
 * Back-off interval when no peers are known (online map is empty).
 * s7ia B3: 30 s avoids hammering the daemon when the user has never paired
 * a device or all pairings have been removed.
 */
const POLL_INTERVAL_IDLE_MS = 30_000;

let pollingStarted = false;
let pollingTimer: ReturnType<typeof setTimeout> | null = null;
/**
 * Tracks whether the last poll cycle reached the daemon successfully.
 * Used to detect daemon restarts: a transition from `false → true` means the
 * daemon just came back up, so we reset all peers to offline immediately
 * (rather than waiting for PEER_PRESENCE_TTL_MS to expire).
 * SCRD-3 / SYNC-5: ensures stale green dots clear on daemon restart.
 */
let lastPollSucceeded = false;

/**
 * Start the global peer-presence polling loop.
 *
 * Safe to call multiple times — subsequent calls are no-ops.  Call from
 * `App.tsx` on mount; call `stopPeerPresencePolling()` on unmount (or just
 * let it run for the app's lifetime, which is fine for a desktop app).
 *
 * s7ia B3: uses adaptive interval — 5 s when peers are known, 30 s when no
 * peers are in the online map (i.e. no devices are paired or ever connected).
 *
 * SCRD-3 / SYNC-5: after each successful poll, `expireStale()` is called so
 * peers whose `connected` event is older than PEER_PRESENCE_TTL_MS flip to
 * offline without waiting for a `disconnected` event.  On daemon restart
 * (previous poll failed, this one succeeded), `resetAllOffline()` is called
 * first for an immediate flip.
 */
export function startPeerPresencePolling(): void {
  if (pollingStarted) return;
  pollingStarted = true;

  const schedule = (delayMs: number) => {
    pollingTimer = setTimeout(() => { void tick(); }, delayMs);
  };

  const tick = async () => {
    if (!pollingStarted) return; // stopped while we were awaiting
    try {
      const { events } = await api.pollPeerEvents();
      const store = usePeerPresence.getState();
      // Daemon restart detection: if the previous poll failed but this one
      // succeeded, the daemon just restarted — clear stale online dots
      // immediately so they don't stay green until TTL expiry.
      if (!lastPollSucceeded) {
        store.resetAllOffline();
      }
      lastPollSucceeded = true;
      if (events.length > 0) {
        store.applyEvents(events);
      }
      // Expire any peer whose last connected event is older than TTL, so
      // stale green dots flip to offline even without an explicit disconnect
      // event (e.g. peer went offline without a clean TCP close).
      store.expireStale();
    } catch (e) {
      // Daemon offline or not yet started — silently skip. The 10 s
      // list_peers poll in DevicesView is the fallback.
      if (!(e instanceof IpcError)) {
        // Unexpected error — log once so it is visible in dev builds.
        console.warn("[peerPresence] poll error:", e);
      }
      lastPollSucceeded = false;
    }
    if (!pollingStarted) return;
    // Use idle interval when no peers have ever been seen, active interval otherwise.
    const hasPeers = Object.keys(usePeerPresence.getState().online).length > 0;
    schedule(hasPeers ? POLL_INTERVAL_MS : POLL_INTERVAL_IDLE_MS);
  };

  // Fire immediately on start (no delay) to populate presence ASAP.
  void tick();
}

/**
 * Stop the polling loop (e.g. in tests or when App unmounts).
 * Resets lastPollSucceeded so the next `startPeerPresencePolling()` call
 * treats its first successful poll as a "fresh start" and resets stale dots.
 */
export function stopPeerPresencePolling(): void {
  pollingStarted = false; // signals the in-flight tick() to exit early
  lastPollSucceeded = false; // reset so re-init clears stale presence
  if (pollingTimer !== null) {
    clearTimeout(pollingTimer);
    pollingTimer = null;
  }
}
