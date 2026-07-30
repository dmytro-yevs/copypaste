/**
 * Paired devices: the peer list, and the four things a user can do to it.
 *
 * The v2 pairing protocol is **not** v1's. Manifest §3.3 describes a PAKE
 * handshake with a six-digit SAS, a QR envelope and a daemon state machine
 * (`pair_get_sas`, `pair_confirm_sas`, `pair_abort`); this daemon has
 * `PairCreate` / `PairAccept` — one device mints a code that is the Noise
 * pre-shared key in transferable form, the other consumes it with an address.
 * There is no SAS to compare and no state machine to abort, so INV-16 and the
 * SAS half of §3.3 have nothing to bind to here.
 *
 * What does carry over, because it is about the secret and not about the
 * mechanism:
 *
 *   - the code is a **credential**: hidden until deliberately revealed
 *     (§3.3.1 / CopyPaste-1jms.2), never logged, never in a toast, and shown
 *     exactly once because the daemon does not keep it;
 *   - peer-reported names are **unverified** and must be labelled as such
 *     (INV-15) — `name` is whatever the other device called itself;
 *   - a failure never renders the raw error (INV-12);
 *   - unpairing is destructive enough to need a confirm (§3.2.6).
 *
 * If the manifest's SAS flow is meant to be the target for v2, that is a daemon
 * change, and the manifest and the protocol should be reconciled in the same
 * commit as it — this file follows the protocol that exists.
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
    // §5.1: 10s, decoupled from the 2s status poll so `peers` stays ≤6
    // calls/min (CopyPaste-crh3.48).
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : PEERS_POLL_MS,
    // A pairing that has never answered is not an error worth retrying at
    // speed; the interval above is the retry.
    retry: false,
  });
}

/**
 * Mint a pairing code. The result is held by the caller and shown once —
 * deliberately **not** written into the query cache, which is inspectable from
 * the devtools and outlives the dialog.
 */
export function usePairCreate() {
  const qc = useQueryClient();
  return useMutation<PairingData, unknown, string>({
    mutationFn: (name: string) => pairCreate(name),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: PEERS_KEY });
    },
    // No toast: a success toast for this would be the one place the code could
    // end up somewhere the dialog does not control.
    onError: (raw) => toast.error(toFriendly(raw)),
  });
}

export function usePairAccept() {
  const qc = useQueryClient();
  return useMutation<PeerInfo[], unknown, { code: string; addr: string }>({
    mutationFn: ({ code, addr }) => pairAccept(code.trim(), addr.trim()),
    onSuccess: (peers) => {
      // The bridge returns the peer list as it stands after pairing, so the
      // cache is seeded rather than invalidated — the new device is on screen
      // before the next 10s poll.
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

/**
 * Sync now, with one peer or with all of them. Partial failure is reported as
 * partial — "2 of 3" rather than a single success or a single error (§3.1.9's
 * `Deleted 3/5 (2 failed)` rule, applied to sync).
 */
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
        // The per-peer error string comes from the daemon and may name a path,
        // so the count is reported and the detail is not (INV-12).
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
