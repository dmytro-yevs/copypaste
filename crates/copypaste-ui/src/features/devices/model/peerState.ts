/**
 * Three field names in `peers()` mean something other than they say:
 *
 *  - `last_seen_ms` is a last-*synced* time — `Node::touch_peer` writes it at
 *    the end of a successful session. `0` means never contacted.
 *  - `last_addr` is set only by the side that dialled, so a device paired by
 *    handing out a code here is never dialled from here.
 *  - `online` is an mDNS record: `false` means "not seen", not "unreachable".
 */
import { classifyError, type ErrorKind } from "@/lib/errors";
import type { PeerInfo, SyncResult } from "@/lib/ipc";

/**
 * `copypaste_p2p::peers::MAX_PAIRINGS`, which is not on the wire.
 *
 * A refusal rather than an eviction, so the screen has to say so before the
 * user spends a code discovering it.
 */
export const MAX_PAIRINGS = 16;

/**
 * Manifest 06's peer-specific threshold. It is deliberately much longer than
 * the global badge threshold so a brief network gap does not become a warning.
 */
export const STALE_AFTER_MS = 30 * 60_000;

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
 * What a run this screen started did with one peer.
 *
 * `PeerInfo` carries a last-*success* time and nothing else about the outcome:
 * the daemon syncs on its own cadence and records no per-peer result the wire
 * can carry. So a run started here is the only evidence of a handshake that is
 * failing, and `sent`/`received` the only evidence one is moving anything.
 */
export interface SyncSuccess {
  readonly at: number;
  readonly sent: number;
  readonly received: number;
  readonly durationMs: number | null;
}

/** A kind, never the daemon's sentence: a per-peer error can name the socket
 *  path, which spells out the local username (INV-12). */
export interface SyncFailure {
  readonly at: number;
  readonly kind: ErrorKind;
  readonly retryable: boolean;
  readonly durationMs: number | null;
}

/** Both halves are kept, because a row has to say when syncing last *worked*
 *  as well as when it last broke, and one slot answers only whichever came
 *  last. */
export interface PeerHealth {
  readonly success?: SyncSuccess;
  readonly failure?: SyncFailure;
}

export type PeerHealthMap = Readonly<Record<string, PeerHealth>>;

export interface ManualSyncAttempt {
  readonly at: number;
  readonly sent: number;
  readonly received: number;
  readonly durationMs: number | null;
}

export function latestManualAttempt(
  health: PeerHealth | undefined,
): ManualSyncAttempt | null {
  const success = health?.success;
  const failure = health?.failure;
  if (!success && !failure) return null;
  if (!failure || (success !== undefined && success.at >= failure.at)) {
    return success ?? null;
  }
  return {
    at: failure.at,
    sent: 0,
    received: 0,
    durationMs: failure.durationMs,
  };
}

export function peerLastSyncAt(peer: PeerInfo): number | null {
  return peer.last_seen_ms > 0 ? peer.last_seen_ms : null;
}

/** A never-synced peer has no durable age on the current wire, so it cannot be
 *  classified as stalled. */
export function peerIsStalled(
  peer: PeerInfo,
  now: number = Date.now(),
): boolean {
  const lastSyncAt = peerLastSyncAt(peer);
  return lastSyncAt !== null && now - lastSyncAt > STALE_AFTER_MS;
}

/**
 * The recorded failure, unless something later settled it.
 *
 * `last_seen_ms` settles one as much as a later run does: the daemon keeps
 * syncing on its own cadence, so a failure held past a successful session would
 * sit on the row for ever with nothing left to fix.
 */
export function unsettledFailure(
  peer: PeerInfo,
  health: PeerHealth | undefined,
): SyncFailure | undefined {
  const failure = health?.failure;
  if (!failure) return undefined;
  const settled = Math.max(peerLastSyncAt(peer) ?? 0, health?.success?.at ?? 0);
  return failure.at > settled ? failure : undefined;
}

export function peerState(
  peer: PeerInfo,
  health: PeerHealth | undefined,
  now: number = Date.now(),
): PeerState {
  const failure = unsettledFailure(peer, health);
  if (
    failure &&
    (failure.kind === "auth_failed" ||
      failure.kind === "pairing_code" ||
      failure.kind === "peer_version" ||
      failure.kind === "protocol_mismatch" ||
      failure.kind === "peer_not_found" ||
      failure.kind === "key_unusable" ||
      failure.kind === "peer_failed")
  ) {
    return "failing";
  }
  if (
    health?.success !== undefined &&
    health.success.at > (peerLastSyncAt(peer) ?? 0)
  ) {
    return "synced";
  }
  if (peerLastSyncAt(peer) === null) return "waiting";
  if (!peer.online || failure?.kind === "peer_unreachable") return "away";
  if (peerIsStalled(peer, now)) return "stalled";
  if (peer.last_addr === null) return "inbound";
  return "synced";
}

/** Record what one `sync_now` run said, per peer. */
export function noteSync(
  previous: PeerHealthMap,
  results: readonly SyncResult[],
  at: number = Date.now(),
): PeerHealthMap {
  const next = { ...previous };
  for (const result of results) {
    const before = previous[result.pairing_id];
    next[result.pairing_id] =
      result.error === null
        ? {
            ...before,
            success: {
              at,
              sent: result.sent,
              received: result.received,
              durationMs: result.duration_ms,
            },
          }
        : {
            ...before,
            failure: {
              at,
              kind: classifyError(result.error),
              retryable: result.error.retryable,
              durationMs: result.duration_ms,
            },
          };
  }
  return next;
}

export function atPairingCap(count: number): boolean {
  return count >= MAX_PAIRINGS;
}
