/**
 * Three field names in `peers()` mean something other than they say, and every
 * state below turns on that (`ui-parity-audit.md` finding 4):
 *
 *  - `last_seen_ms` is a last-*synced* time — `Node::touch_peer` writes it at
 *    the end of a successful session. `0` means never contacted.
 *  - `last_addr` is set only by the side that dialled, so a device paired by
 *    handing out a code here is never dialled from here.
 *  - `online` is an mDNS record: `false` means "not seen", not "unreachable".
 */
import type { PeerInfo } from "@/lib/ipc";

/**
 * `copypaste_p2p::peers::MAX_PAIRINGS`, which is not on the wire.
 *
 * A refusal rather than an eviction, so the screen has to say so before the
 * user spends a code discovering it.
 */
export const MAX_PAIRINGS = 16;

/**
 * How long a device may be visible and not syncing before that is a fault.
 *
 * Three of the peer loop's ceiling rounds (`MAX_POLL_INTERVAL`, 300 s). Below
 * that, a gap is the cadence doing what it is meant to.
 */
export const STALE_AFTER_MS = 15 * 60_000;

export const PEER_STATES = [
  "waiting",
  "failing",
  "inbound",
  "away",
  "stalled",
  "synced",
] as const;

export type PeerState = (typeof PEER_STATES)[number];

/**
 * The outcome of a sync this screen started, remembered until a later session
 * with that peer succeeds.
 *
 * Scoped to what the window observed: the daemon syncs on its own cadence and
 * does not record per-peer failures anywhere the wire can carry, so this is
 * the only honest evidence of a handshake that is failing.
 */
export interface SyncAttempt {
  readonly at: number;
  readonly failed: boolean;
}

export type SyncAttempts = Readonly<Record<string, SyncAttempt>>;

export function peerState(
  peer: PeerInfo,
  attempt: SyncAttempt | undefined,
  now: number = Date.now(),
): PeerState {
  if (peer.last_seen_ms <= 0) return "waiting";
  // A session that succeeded after the failed attempt settles it; the daemon
  // syncs on its own cadence, so a stale failure would outlive its cause.
  if (attempt?.failed && attempt.at > peer.last_seen_ms) return "failing";
  if (peer.last_addr === null) return "inbound";
  if (!peer.online) return "away";
  return now - peer.last_seen_ms > STALE_AFTER_MS ? "stalled" : "synced";
}

/** Record what one `sync_now` run said, per peer. */
export function noteAttempts(
  previous: SyncAttempts,
  results: ReadonlyArray<{ pairing_id: string; error: string | null }>,
  at: number = Date.now(),
): SyncAttempts {
  const next = { ...previous };
  for (const result of results) {
    next[result.pairing_id] = { at, failed: result.error !== null };
  }
  return next;
}

export function atPairingCap(count: number): boolean {
  return count >= MAX_PAIRINGS;
}
