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
 * - The store is append-only per fingerprint — we never remove entries
 *   (pairing removes peers through `listPeers`, not here).
 * - On error (daemon offline), the loop backs off silently; the fallback is
 *   the existing 10 s `list_peers` poll in `DevicesView`.
 */

import { create } from "zustand";
import { api, IpcError } from "./ipc";

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface PeerPresenceState {
  /** Map from canonical fingerprint → true (online) | false (offline). */
  online: Record<string, boolean>;
  /** Apply a batch of events from `poll_peer_events`. */
  applyEvents: (
    events: Array<{ kind: "connected" | "disconnected"; fingerprint: string }>,
  ) => void;
}

export const usePeerPresence = create<PeerPresenceState>()((set) => ({
  online: {},
  applyEvents(events) {
    if (events.length === 0) return;
    set((state) => {
      const next = { ...state.online };
      for (const ev of events) {
        next[ev.fingerprint] = ev.kind === "connected";
      }
      return { online: next };
    });
  },
}));

// ---------------------------------------------------------------------------
// Polling loop (started once, app-globally)
// ---------------------------------------------------------------------------

/** How often to drain the daemon's peer-event queue (milliseconds). */
const POLL_INTERVAL_MS = 1_000;

let pollingStarted = false;
let pollingTimer: ReturnType<typeof setInterval> | null = null;

/**
 * Start the global peer-presence polling loop.
 *
 * Safe to call multiple times — subsequent calls are no-ops.  Call from
 * `App.tsx` on mount; call `stopPeerPresencePolling()` on unmount (or just
 * let it run for the app's lifetime, which is fine for a desktop app).
 */
export function startPeerPresencePolling(): void {
  if (pollingStarted) return;
  pollingStarted = true;

  const poll = async () => {
    try {
      const { events } = await api.pollPeerEvents();
      if (events.length > 0) {
        usePeerPresence.getState().applyEvents(events);
      }
    } catch (e) {
      // Daemon offline or not yet started — silently skip. The 10 s
      // list_peers poll in DevicesView is the fallback.
      if (!(e instanceof IpcError)) {
        // Unexpected error — log once so it is visible in dev builds.
        console.warn("[peerPresence] poll error:", e);
      }
    }
  };

  // Fire immediately on start, then every POLL_INTERVAL_MS.
  void poll();
  pollingTimer = setInterval(() => { void poll(); }, POLL_INTERVAL_MS);
}

/**
 * Stop the polling loop (e.g. in tests or when App unmounts).
 */
export function stopPeerPresencePolling(): void {
  if (pollingTimer !== null) {
    clearInterval(pollingTimer);
    pollingTimer = null;
  }
  pollingStarted = false;
}
