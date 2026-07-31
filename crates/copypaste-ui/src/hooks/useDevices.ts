/**
 * The v2 pairing protocol is not v1's: manifest §3.3 describes a PAKE handshake
 * with a six-digit SAS, this daemon has `PairCreate` / `PairAccept`, so INV-16
 * and the SAS half of §3.3 have nothing here to bind to.
 *
 * What carries over is the secret rather than the mechanism: the code is a
 * credential (hidden until revealed, never logged, never toasted, minted once —
 * CopyPaste-1jms.2), peer names are self-reported (INV-15), and no failure
 * renders a raw error (INV-12).
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { PEERS_POLL_MS, POLL_BACKOFF_MS } from "@/lib/layout";
import {
  type DiscoveredDevice,
  type PairingData,
  type PeerInfo,
  type SyncResult,
  listDiscovered,
  listPeers,
  pairAccept,
  pairCreate,
  rescanDiscovered,
  revokeDevice,
  syncNow,
  unpair,
} from "@/lib/ipc";
import { HISTORY_KEY } from "@/hooks/useHistory";

export const PEERS_KEY = ["peers"] as const;
export const DISCOVERED_KEY = ["discovered"] as const;

export function usePeers() {
  return useQuery<PeerInfo[]>({
    queryKey: PEERS_KEY,
    queryFn: listPeers,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : PEERS_POLL_MS,
    retry: false,
  });
}

/** Only while the pairing dialog is open: a clipboard manager that browses mDNS
 *  continuously announces its presence to every network its owner joins.
 *  Nothing here is authenticated (INV-15). */
export function useDiscovered(enabled: boolean) {
  return useQuery<DiscoveredDevice[]>({
    queryKey: DISCOVERED_KEY,
    queryFn: listDiscovered,
    enabled,
    refetchInterval: PEERS_POLL_MS,
    // A build with no discovery answers `unavailable`, which is a fact rather
    // than a transient failure.
    retry: false,
  });
}

export function useRescan() {
  const qc = useQueryClient();
  return useMutation<DiscoveredDevice[], unknown, void>({
    mutationFn: () => rescanDiscovered(),
    onSuccess: (devices) => qc.setQueryData(DISCOVERED_KEY, devices),
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** The code is held by the caller, never written to the query cache: a cache
 *  entry outlives the dialog and is inspectable from the devtools. */
export function usePairCreate() {
  const qc = useQueryClient();
  return useMutation<PairingData, unknown, string>({
    mutationFn: (name: string) => pairCreate(name),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    // No success toast: it would be the one place the code could leave the
    // dialog's control.
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export function usePairAccept() {
  const qc = useQueryClient();
  return useMutation<PeerInfo[], unknown, { code: string; addr: string }>({
    mutationFn: ({ code, addr }) => pairAccept(code.trim(), addr.trim()),
    onSuccess: (peers) => {
      qc.setQueryData(PEERS_KEY, peers);
      toast.success(t("devices.toast.paired"));
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
    },
    // The dialog renders the failure itself; a toast as well would double it.
  });
}

export function useUnpair() {
  const qc = useQueryClient();
  return useMutation<void, unknown, PeerInfo>({
    mutationFn: (peer) => unpair(peer.pairing_id),
    onSuccess: (_data, peer) => {
      toast.success(t("devices.toast.unpaired", { name: peer.name }));
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** `unpair` stops this device syncing and leaves the pairing code working; this
 *  bars the pairing id for good. The confirmation lives in the view, and
 *  reaching here *is* the confirmation. */
export function useRevoke() {
  const qc = useQueryClient();
  return useMutation<void, unknown, PeerInfo>({
    mutationFn: (peer) => revokeDevice(peer.pairing_id),
    onSuccess: (_data, peer) => {
      toast.success(t("devices.toast.revoked", { name: peer.name }));
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Partial failure is reported as partial — "2 of 3", not one success or one
 *  error (§3.1.9). */
export function useSyncNow() {
  const qc = useQueryClient();
  return useMutation<SyncResult[], unknown, string | undefined>({
    mutationFn: (pairingId) => syncNow(pairingId),
    onSuccess: (results) => {
      const failed = results.filter((r) => r.error !== null);
      const received = results.reduce((n, r) => n + r.received, 0);
      const sent = results.reduce((n, r) => n + r.sent, 0);

      if (results.length === 0) {
        toast(t("devices.toast.nothingToSync"));
      } else if (failed.length === 0) {
        toast.success(
          t("devices.toast.synced", { count: results.length, sent, received }),
        );
      } else {
        // The per-peer error may name a path: report the count, not the text
        // (INV-12).
        toast.warning(
          t("devices.toast.syncedPartial", {
            done: results.length - failed.length,
            total: results.length,
            failed: failed.length,
          }),
        );
      }
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
