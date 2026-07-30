/**
 * **The v2 pairing protocol is not v1's.** Manifest §3.3 describes a PAKE
 * handshake with a six-digit SAS and a daemon state machine; this daemon has
 * `PairCreate` / `PairAccept` and neither exists, so INV-16 and the SAS half of
 * §3.3 have nothing here to bind to. Reconciling the two is a daemon decision,
 * not a frontend one.
 *
 * What carries over is about the secret rather than the mechanism: the code is
 * a credential (hidden until revealed, never logged, never toasted, minted
 * once — CopyPaste-1jms.2), peer names are self-reported and must read as
 * unverified (INV-15), and no failure renders a raw error (INV-12).
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { toFriendly } from "@/lib/errors";
import { PEERS_POLL_MS, POLL_BACKOFF_MS } from "@/lib/layout";
import {
  type PairingData,
  type PeerInfo,
  type SyncResult,
  listPeers,
  pairAccept,
  pairCreate,
  syncNow,
  unpair,
} from "@/lib/ipc";
import { HISTORY_KEY } from "@/hooks/useHistory";

export const PEERS_KEY = ["peers"] as const;

export function usePeers() {
  return useQuery<PeerInfo[]>({
    queryKey: PEERS_KEY,
    queryFn: listPeers,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : PEERS_POLL_MS,
    retry: false,
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
      toast.success("Device paired");
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
      toast.success(`Unpaired ${peer.name}`);
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
        toast("Nothing to sync — no paired devices");
      } else if (failed.length === 0) {
        toast.success(`Synced ${results.length} device${results.length === 1 ? "" : "s"} — sent ${sent}, received ${received}`);
      } else {
        // The per-peer error may name a path: report the count, not the text
        // (INV-12).
        toast.warning(
          `Synced ${results.length - failed.length} of ${results.length} devices — ${failed.length} failed`,
        );
      }
      void qc.invalidateQueries({ queryKey: HISTORY_KEY });
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}
