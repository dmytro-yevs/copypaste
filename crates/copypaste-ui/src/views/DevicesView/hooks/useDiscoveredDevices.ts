// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { useState, useEffect, useCallback } from "react";
import {
  api,
  ipcErrorMessage,
  IpcError,
  type DiscoveredDevice,
} from "../../../lib/ipc";

/**
 * How often to refresh the discovered-devices list while the Devices view is open.
 * 3 s is a reasonable balance: fast enough to show a newly-announced peer within
 * a few seconds of it appearing on the LAN, slow enough to avoid busy-looping.
 *
 * Note: mDNS-SD announcement cadence is controlled in copypaste-p2p; if that
 * cadence changes, adjust this constant proportionally so the list stays current.
 */
export const DISCOVERED_POLL_MS = 3000;

/**
 * Loads and polls discovered (unpaired) LAN devices, and provides a manual
 * rescan action (HB-9).
 *
 * `ownFpRef` — ref to this device's fingerprint to filter self out.
 */
export function useDiscoveredDevices({
  ownFpRef,
}: {
  ownFpRef: React.RefObject<string | null>;
}) {
  const [discovered, setDiscovered] = useState<DiscoveredDevice[]>([]);
  // Inline error shown beneath the discovered list (e.g. rate-limited).
  const [discoverError, setDiscoverError] = useState<string | null>(null);
  // HB-9: true while a manual mDNS rescan is in flight (Refresh button).
  const [rescanning, setRescanning] = useState(false);

  const loadDiscovered = useCallback(async () => {
    try {
      const { devices } = await api.listDiscovered();
      const ownFp = ownFpRef.current;
      const unpaired = devices.filter(
        (d) => !d.paired && (ownFp === null || d.device_id !== ownFp)
      );
      setDiscovered(unpaired);
    } catch (e) {
      // Daemon-offline is already surfaced by the peers loader above — stay silent here.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setDiscovered([]);
        return;
      }
      // All other errors (e.g. P2P disabled, socket timeout) are surfaced inline so
      // the user understands why the discovered list is empty (CopyPaste-44rq.27).
      setDiscovered([]);
      const msg = `Could not load nearby devices: ${ipcErrorMessage(e, "unexpected error")}`;
      setDiscoverError(msg);
      console.error("[DevicesView] loadDiscovered error:", e);
    }
  // ownFpRef is a ref — always current without being a dep.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void loadDiscovered();
    const id = setInterval(() => { void loadDiscovered(); }, DISCOVERED_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadDiscovered]);

  // HB-9: manual rescan — restart the daemon's mDNS browse in place and refresh
  // the discovered list with the fresh snapshot it returns.
  const handleRescan = useCallback(async () => {
    if (rescanning) return;
    setRescanning(true);
    setDiscoverError(null);
    try {
      const { devices } = await api.rescanDiscovered();
      const ownFp = ownFpRef.current;
      const unpaired = devices.filter(
        (d) => !d.paired && (ownFp === null || d.device_id !== ownFp)
      );
      setDiscovered(unpaired);
    } catch (e) {
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setDiscovered([]);
      } else {
        // Log raw error for diagnostics only — never render raw IPC/FS strings
        // in the DOM (CopyPaste-j5qg).
         
        console.error("[DevicesView] rescan failed:", e);
        // bdac.36: "clipboard service" is the canonical user-facing term.
        setDiscoverError("Network scan failed. Check that Wi-Fi is on and the clipboard service is running.");
      }
    } finally {
      setRescanning(false);
    }
  // rescanning and ownFpRef are stable; exhaustive-deps would add noise here.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rescanning]);

  return { discovered, setDiscovered, discoverError, setDiscoverError, rescanning, loadDiscovered, handleRescan };
}
