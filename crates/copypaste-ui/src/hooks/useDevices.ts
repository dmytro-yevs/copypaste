/**
 * Pair creation and acceptance are deliberately absent from the UI: the
 * current daemon protocol persists a pairing before it can complete the
 * manifest-required SAS confirmation and has no abort state machine. Known
 * devices remain manageable below; see ADR-0007.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { PEERS_POLL_MS, POLL_BACKOFF_MS } from "@/lib/layout";
import {
  type DiscoveredDevice,
  type PeerInfo,
  type SyncResult,
  listDiscovered,
  listPeers,
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
