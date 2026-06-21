// Extracted from DevicesView.tsx (CopyPaste-g06m.15).
// Cut/paste only — NO behavior changes.
import { useState, useEffect, useCallback, useRef } from "react";
import {
  api,
  ipcErrorMessage,
  IpcError,
  isIpcNotReady,
  probeStatus,
  type PairedDevice,
} from "../../../lib/ipc";
import { type DeviceRowState } from "../../../components/DeviceCard";

// Devices load outcomes. `degraded` (daemon up, DB unavailable), `not_ready`
// (daemon up but still initialising), and `error` (some other failure) are split
// out from `offline` so each condition gets its own friendly message.
export type LoadState = "loading" | "offline" | "not_ready" | "degraded" | "error" | "ready";

const PEERS_POLL_MS = 10_000;

/**
 * Loads and polls paired peers, tracks per-row state (pending/error/revokedAt),
 * and exposes revoke/unpair/revokeAndRotate actions.
 *
 * `ownFpRef` — a ref to the current device's fingerprint (from useOwnDevice)
 * used to filter this device out of the peer list without closing over stale state.
 */
export function usePairedDevices({
  ownFpRef,
  onShowToast,
}: {
  ownFpRef: React.RefObject<string | null>;
  onShowToast: (msg: string, opts: { kind: "success" | "error"; duration: number }) => void;
}) {
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [peers, setPeers] = useState<PairedDevice[]>([]);
  const [rowState, setRowState] = useState<Record<string, DeviceRowState>>({});
  const [revokeAllPending, setRevokeAllPending] = useState(false);
  const [revokeAllConfirm, setRevokeAllConfirm] = useState(false);
  // C-P0-4: the device a Revoke confirm dialog is open for (null = closed).
  const [revokePrompt, setRevokePrompt] = useState<{ fingerprint: string; name: string } | null>(
    null,
  );
  // New sync-key passphrase entered in the "Revoke & rotate" path.
  const [rotatePassphrase, setRotatePassphrase] = useState("");
  const [revokeBusy, setRevokeBusy] = useState(false);

  // Ref also tracks when peers data was last fetched (for live elapsed seconds).
  const peersFetchedAtRef = useRef<number>(Math.floor(Date.now() / 1000));

  // Unmount guard for handleUnpair / handleRevoke — prevents setState after
  // the component unmounts if the user navigates away mid-request (P2 finding).
  const peerActionCancelledRef = useRef(false);
  useEffect(() => {
    peerActionCancelledRef.current = false;
    return () => { peerActionCancelledRef.current = true; };
  }, []);

  const loadPeers = useCallback(async () => {
    setLoadState("loading");
    try {
      const { peers: fetched } = await api.listPeers();
      // Deduplicate by fingerprint (daemon-side fix may not always be deployed).
      const seen = new Set<string>();
      const deduped = fetched.filter((p) => {
        if (seen.has(p.fingerprint)) return false;
        seen.add(p.fingerprint);
        return true;
      });
      // Read from ref so we always use the latest fingerprint even if ownState
      // hasn't re-rendered yet — avoids the stale-closure bug (P1 audit finding).
      const ownFp = ownFpRef.current;
      // Don't show this device in the peers list.
      const filteredPeers = deduped.filter(
        (p) => ownFp === null || p.fingerprint !== ownFp
      );
      setPeers(filteredPeers);
      // Preserve transient per-row state across reloads; discard rows no longer
      // in the peer list so stale state doesn't accumulate.
      const liveFingerprints = new Set(filteredPeers.map((p) => p.fingerprint));
      setRowState((prev) => {
        const next: Record<string, DeviceRowState> = {};
        for (const fp of liveFingerprints) {
          if (prev[fp]) next[fp] = prev[fp];
        }
        return next;
      });

      // A-4: stamp when we last got fresh data so the 1 s clock can compute
      // live elapsed seconds since the daemon snapshot.
      peersFetchedAtRef.current = Math.floor(Date.now() / 1000);
      setLoadState("ready");
    } catch (e) {
      // A transport failure means the daemon is genuinely unreachable.
      if (e instanceof IpcError && e.code === "daemon_offline") {
        setLoadState("offline");
        return;
      }
      // Daemon is up but still initialising its database — show a friendly
      // "starting up" state rather than a hard error.
      if (isIpcNotReady(e)) {
        setLoadState("not_ready");
        return;
      }
      // The daemon answered but list_peers failed. Probe `status` to tell a
      // DB-degraded daemon (recoverable via History → Reset database) apart from
      // a generic error — previously both fell through to "offline" and the
      // degraded case was mislabeled "Daemon not running."
      const probe = await probeStatus();
      if (probe.kind === "offline") {
        setLoadState("offline");
      } else if (probe.kind === "degraded") {
        setLoadState("degraded");
      } else {
        setLoadState("error");
      }
    }
  // ownFpRef is a ref — reading it inside the callback is always current.
  // No dep on ownState avoids a stale-closure re-creation on every render.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void loadPeers();
  }, [loadPeers]);

  // Poll peers every 10 s so the online dot refreshes without user interaction.
  // Clears the interval on unmount to avoid timer leaks (matches existing pattern).
  useEffect(() => {
    const id = setInterval(() => { void loadPeers(); }, PEERS_POLL_MS);
    return () => { clearInterval(id); };
  }, [loadPeers]);

  // --- Row helpers ---
  const setRowPending = (fingerprint: string, pending: boolean) => {
    setRowState((prev) => ({
      ...prev,
      [fingerprint]: {
        revokedAt: prev[fingerprint]?.revokedAt ?? null,
        error: prev[fingerprint]?.error ?? null,
        pending,
      },
    }));
  };

  const setRowError = (fingerprint: string, error: string) => {
    setRowState((prev) => ({
      ...prev,
      [fingerprint]: {
        revokedAt: prev[fingerprint]?.revokedAt ?? null,
        pending: false,
        error,
      },
    }));
  };

  const handleUnpair = async (fingerprint: string) => {
    setRowPending(fingerprint, true);
    try {
      await api.unpairPeer(fingerprint);
      if (peerActionCancelledRef.current) return;
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Unpair failed");
      setRowError(fingerprint, msg);
    }
  };

  // Plain revoke: P2P-only (mTLS allowlist + denylist). Does NOT cut off
  // cloud/relay — that requires a sync-key rotation (see handleRevokeAndRotate).
  const handleRevoke = async (fingerprint: string) => {
    setRevokePrompt(null);
    setRowPending(fingerprint, true);
    try {
      const { revoked_at } = await api.revokePeer(fingerprint);
      if (peerActionCancelledRef.current) return;
      setRowState((prev) => ({
        ...prev,
        [fingerprint]: { revokedAt: revoked_at, pending: false, error: null },
      }));
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Revoke failed");
      setRowError(fingerprint, msg);
    }
  };

  // C-P0-4: revoke from P2P AND rotate the sync key so the revoked device is
  // also cut off from cloud/relay sync. `revoke_and_rotate` derives the new key
  // first, so a too-short passphrase fails before any revocation is applied.
  const handleRevokeAndRotate = async (fingerprint: string) => {
    if (revokeBusy) return;
    setRevokeBusy(true);
    setRowPending(fingerprint, true);
    try {
      const { revoked_at } = await api.revokeAndRotate(fingerprint, rotatePassphrase);
      if (peerActionCancelledRef.current) return;
      setRevokePrompt(null);
      setRotatePassphrase("");
      setRowState((prev) => ({
        ...prev,
        [fingerprint]: { revokedAt: revoked_at, pending: false, error: null },
      }));
      onShowToast("Revoked & rotated sync key — re-provision remaining devices", {
        kind: "success",
        duration: 5000,
      });
      await loadPeers();
    } catch (err) {
      if (peerActionCancelledRef.current) return;
      const msg = ipcErrorMessage(err, "Revoke & rotate failed");
      setRowError(fingerprint, msg);
    } finally {
      if (!peerActionCancelledRef.current) setRevokeBusy(false);
    }
  };

  const handleRevokeAllConfirmed = async () => {
    setRevokeAllConfirm(false);
    setRevokeAllPending(true);
    try {
      const result = await api.revokeAllPeers();
      const n = result.revoked ?? 0;
      onShowToast(`Revoked ${n} device${n === 1 ? "" : "s"}`, { kind: "success", duration: 3000 });
      await loadPeers();
    } catch (err) {
      const msg = ipcErrorMessage(err, "Revoke all failed");
      onShowToast(msg, { kind: "error", duration: 4000 });
    } finally {
      setRevokeAllPending(false);
    }
  };

  return {
    loadState,
    peers,
    rowState,
    revokeAllPending,
    setRevokeAllPending,
    revokeAllConfirm,
    setRevokeAllConfirm,
    revokePrompt,
    setRevokePrompt,
    rotatePassphrase,
    setRotatePassphrase,
    revokeBusy,
    peersFetchedAtRef,
    loadPeers,
    handleUnpair,
    handleRevoke,
    handleRevokeAndRotate,
    handleRevokeAllConfirmed,
  };
}
