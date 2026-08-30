import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { t } from "@/i18n";
import { toFriendly } from "@/lib/errors";
import { PEERS_POLL_MS, POLL_BACKOFF_MS } from "@/lib/scheduling";
import {
  type DiscoveredDevice,
  type PeerInfo,
  type SyncResult,
  listDiscovered,
  listPeers,
  rescanDiscovered,
  revokeDevice,
  setDeviceName,
  syncNow,
  unpair,
} from "@/lib/ipc";
import { invalidateHistoryQueries, STATUS_KEY } from "@/hooks/historyRefresh";

export const PEERS_KEY = ["peers"] as const;
export const DISCOVERED_KEY = ["discovered"] as const;
export const SYNC_KEY = ["peer-sync"] as const;
export const DISCOVERED_POLL_MS = 3_000;

function uniqueDevices<T extends { readonly pairing_id: string }>(devices: readonly T[]): T[] {
  return [...new Map(devices.map((device) => [device.pairing_id, device])).values()];
}

function uniqueDiscovered(devices: readonly DiscoveredDevice[]): DiscoveredDevice[] {
  return [...new Map(devices.map((device) => [device.discovery_id, device])).values()];
}

export function usePeers() {
  return useQuery<PeerInfo[]>({
    queryKey: PEERS_KEY,
    queryFn: listPeers,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : PEERS_POLL_MS,
    retry: false,
    select: uniqueDevices,
  });
}

/** Nothing here is authenticated (INV-15). The Devices view owns this poll so
 *  discovery stops when the user navigates away. */
export function useDiscovered() {
  return useQuery<DiscoveredDevice[]>({
    queryKey: DISCOVERED_KEY,
    queryFn: listDiscovered,
    refetchInterval: DISCOVERED_POLL_MS,
    // A build with no discovery answers `unavailable`, which is a fact rather
    // than a transient failure.
    retry: false,
    select: uniqueDiscovered,
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

export function useRenameDevice() {
  const qc = useQueryClient();
  return useMutation<void, unknown, string>({
    mutationFn: setDeviceName,
    onSuccess: async () => {
      toast.success(t("devices.own.rename.saved"));
      await qc.invalidateQueries({ queryKey: STATUS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

/** Partial failure is reported as partial — "2 of 3", not one success or one
 *  error (§3.1.9). */
export function useSyncNow() {
  const qc = useQueryClient();
  return useMutation<SyncResult[], unknown, string | undefined>({
    mutationKey: SYNC_KEY,
    mutationFn: (pairingId) => syncNow(pairingId),
    onSuccess: (results) => {
      const failed = results.filter((r) => r.error !== null);
      const received = results.reduce((n, r) => n + r.received, 0);
      const sent = results.reduce((n, r) => n + r.sent, 0);

      if (results.length === 0) {
        toast(t("devices.toast.nothingToSync"));
      } else if (failed.length > 0) {
        // Per-peer errors are structured; the toast reports aggregate work and
        // each row localizes its own code (INV-12).
        toast.warning(
          t("devices.toast.syncedPartial", {
            done: results.length - failed.length,
            total: results.length,
            failed: failed.length,
          }),
        );
      } else {
        const missingSizeCounts = results.some(
          (result) => result.skipped_too_large === undefined,
        );
        const skippedTooLarge = results.reduce(
          (count, result) => count + (result.skipped_too_large ?? 0),
          0,
        );

        if (missingSizeCounts && skippedTooLarge > 0) {
          toast.warning(
            t("devices.toast.syncedWithKnownSizeRefusals", {
              count: results.length,
              sent,
              received,
              skipped: skippedTooLarge,
            }),
          );
        } else if (missingSizeCounts) {
          toast.warning(
            t("devices.toast.syncedWithUnknownSizeRefusals", {
              count: results.length,
              sent,
              received,
            }),
          );
        } else if (skippedTooLarge > 0) {
          toast.warning(
            t("devices.toast.syncedWithSizeRefusals", {
              count: results.length,
              sent,
              received,
              skipped: skippedTooLarge,
            }),
          );
        } else {
          toast.success(
            t("devices.toast.synced", { count: results.length, sent, received }),
          );
        }
      }
      void invalidateHistoryQueries(qc);
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
