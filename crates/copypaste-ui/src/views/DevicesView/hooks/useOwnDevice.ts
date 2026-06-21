// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { useState, useEffect, useCallback, useRef } from "react";
import { api, IpcError, type OwnDeviceInfo } from "../../../lib/ipc";

type OwnDeviceState =
  | { status: "loading" }
  | { status: "ready"; info: OwnDeviceInfo }
  | { status: "offline" };

const OWN_INFO_POLL_MS = 10_000;

/**
 * Loads and polls this device's own info (fingerprint, IPs, model, etc.).
 * Also exposes `ownFpRef` so sibling hooks can read the latest fingerprint
 * without closing over a stale `ownState` snapshot.
 *
 * A-2: polls every 10 s (same cadence as usePairedDevices) so Public IP
 * (resolved async via STUN) and Local IP (may change on network switch)
 * stay fresh without user interaction.
 */
export function useOwnDevice() {
  const [ownState, setOwnState] = useState<OwnDeviceState>({ status: "loading" });
  // Ref that always holds the latest own fingerprint so loadPeers (a useCallback)
  // can read the current value without closing over a stale ownState snapshot.
  const ownFpRef = useRef<string | null>(null);

  // A-2: extracted to a stable useCallback so it can be called both on mount
  // and on the 10 s polling interval (same cadence as loadPeers) so that
  // Public IP (resolved async via STUN after mount) and Local IP (may change
  // on network switch) stay fresh without requiring a manual refresh.
  const loadOwnInfo = useCallback(async () => {
    try {
      const info = await api.getOwnDeviceInfo();
      // Keep the ref in sync so loadPeers always reads the latest fingerprint
      // without closing over a stale ownState snapshot.
      ownFpRef.current = info.fingerprint ?? null;
      setOwnState({ status: "ready", info });
    } catch (err: unknown) {
      const code = err instanceof IpcError ? err.code : null;
      if (code === "daemon_offline") {
        setOwnState({ status: "offline" });
      } else {
        // Daemon is up but method may not exist on older daemon builds —
        // treat as offline so the UI still shows the fingerprint section.
        setOwnState({ status: "offline" });
      }
    }
  }, []);

  // Initial load on mount — sets "loading" first, then resolves via loadOwnInfo.
  useEffect(() => {
    setOwnState({ status: "loading" });
    void loadOwnInfo();
  }, [loadOwnInfo]);

  // Poll own-device info every 10 s (same interval as loadPeers) so Public IP
  // (resolved async via STUN) and Local IP (may change on network switch)
  // stay fresh without user interaction.
  useEffect(() => {
    const id = setInterval(() => { void loadOwnInfo(); }, OWN_INFO_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadOwnInfo]);

  return { ownState, ownFpRef };
}
