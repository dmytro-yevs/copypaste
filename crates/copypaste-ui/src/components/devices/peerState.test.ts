/**
 * The six states, and the reason there are six rather than two.
 *
 * Each row here is a different remedy: a code nobody has redeemed, a device
 * this one holds no address for, a device sitting on the network not syncing,
 * and a device that is simply elsewhere were all rendered as one grey dot.
 */
import { describe, expect, it } from "vitest";

import {
  MAX_PAIRINGS,
  STALE_AFTER_MS,
  atPairingCap,
  latestManualAttempt,
  noteSync,
  peerIsStalled,
  peerState,
  unsettledFailure,
} from "@/features/devices/model/peerState";
import type { SyncResult } from "@/lib/ipc";
import { peer } from "@/test/harness";

const NOW = 1_800_000_000_000;

function result(over: Partial<SyncResult> = {}): SyncResult {
  return {
    pairing_id: "pair-1",
    name: "Kitchen Mac",
    sent: 0,
    received: 0,
    error: null,
    ...over,
    duration_ms: over.duration_ms ?? null,
  };
}

describe("what a paired device is doing", () => {
  /** `last_seen_ms === 0` is the store's "never contacted", and it outranks
   *  everything: nothing else has happened yet to describe. */
  it("reads an unredeemed pairing as waiting, whatever discovery says", () => {
    for (const online of [true, false]) {
      const state = peerState(
        peer({ last_seen_ms: 0, last_addr: null, online }),
        undefined,
        NOW,
      );
      expect(state, `online=${online}`).toBe("waiting");
    }
  });

  /** The responder side of a pairing records no address — an inbound ephemeral
   *  port is not dialable — and the daemon's peer loop skips every peer without
   *  one. So this device never starts a sync with it. */
  it("separates a peer this device cannot dial from one that is merely away", () => {
    expect(
      peerState(peer({ last_addr: null, last_seen_ms: NOW, online: true }), undefined, NOW),
    ).toBe("inbound");
    expect(
      peerState(peer({ last_seen_ms: NOW, online: false }), undefined, NOW),
    ).toBe("away");
  });

  /** Manifest 06 §3.8: the badge must not read `synced` while a peer silently
   *  receives nothing. Visible and not syncing is the case that guards. */
  it("calls a device that is on the network and not syncing stalled", () => {
    expect(
      peerState(peer({ last_seen_ms: NOW - STALE_AFTER_MS - 1 }), undefined, NOW),
    ).toBe("stalled");
    expect(
      peerState(peer({ last_seen_ms: NOW - STALE_AFTER_MS + 1 }), undefined, NOW),
    ).toBe("synced");
  });

  /** Manifest 06 §3.2.3 makes current presence tri-state. A stale success is
   *  not evidence that a sleeping or offline device needs intervention. */
  it("keeps an old successful sync neutral when discovery is quiet", () => {
    expect(
      peerState(
        peer({ last_seen_ms: NOW - 30 * STALE_AFTER_MS, online: false }),
        undefined,
        NOW,
      ),
    ).toBe("away");
  });

  it("uses the binding thirty-minute boundary exactly", () => {
    expect(
      peerIsStalled(peer({ last_seen_ms: NOW - STALE_AFTER_MS }), NOW),
    ).toBe(false);
    expect(
      peerIsStalled(peer({ last_seen_ms: NOW - STALE_AFTER_MS - 1 }), NOW),
    ).toBe(true);
  });

});

describe("a failure this screen watched happen", () => {
  it("keeps reachability transient but preserves a failed sync session", () => {
    const offlinePeer = peer({
      last_addr: null,
      last_seen_ms: NOW - 1000,
      online: false,
    });
    expect(
      peerState(
        offlinePeer,
        {
          failure: {
            at: NOW,
            kind: "peer_unreachable",
            retryable: true,
            durationMs: null,
          },
        },
        NOW,
      ),
    ).toBe("failing");
    expect(
      peerState(
        offlinePeer,
        {
          failure: {
            at: NOW,
            kind: "peer_failed",
            retryable: true,
            durationMs: null,
          },
        },
        NOW,
      ),
    ).toBe("failing");
  });

  it("does not let offline presence erase a trust failure", () => {
    expect(
      peerState(
        peer({ last_seen_ms: NOW - 1000, online: false }),
        {
          failure: {
            at: NOW,
            kind: "auth_failed",
            retryable: false,
            durationMs: null,
          },
        },
        NOW,
      ),
    ).toBe("failing");
  });

  /**
   * The daemon syncs on its own cadence and reports per-peer failure nowhere
   * the wire can carry it. A remembered failure that a later session has
   * already settled would therefore sit on the row for ever.
   */
  it("is settled by a session that succeeded after it", () => {
    const health = {
      failure: {
        at: NOW - 1000,
        kind: "peer_failed",
        retryable: true,
        durationMs: null,
      },
    } as const;
    expect(peerState(peer({ last_seen_ms: NOW }), health, NOW)).toBe("synced");
    expect(unsettledFailure(peer({ last_seen_ms: NOW }), health)).toBeUndefined();
  });

  /** A run started here settles it as much as the daemon's own cadence does:
   *  `last_seen_ms` only moves on the daemon's next poll. */
  it("is settled by a later run of its own that worked", () => {
    const stale = peer({ last_seen_ms: NOW - 60_000 });
    const health = {
      failure: {
        at: NOW - 1000,
        kind: "peer_unreachable",
        retryable: true,
        durationMs: null,
      },
      success: { at: NOW, sent: 2, received: 0, durationMs: null },
    } as const;
    expect(unsettledFailure(stale, health)).toBeUndefined();
    expect(peerState(stale, health, NOW)).toBe("synced");
  });
});

describe("what one run recorded, per peer", () => {
  /**
   * Both halves survive the round. Keeping only the latest outcome answers
   * "did the last run work" and loses "is this device syncing at all", which
   * is the question the row exists to answer.
   */
  it("keeps the last success and the last failure apart", () => {
    const first = noteSync(
      {},
      [result({ sent: 3, received: 1, duration_ms: 84 })],
      NOW - 5000,
    );
    const second = noteSync(
      first,
      [
        result({
          duration_ms: 125,
          error: { code: "peer_unreachable", retryable: true },
        }),
      ],
      NOW,
    );

    expect(second["pair-1"]).toEqual({
      success: {
        at: NOW - 5000,
        sent: 3,
        received: 1,
        durationMs: 84,
      },
      failure: {
        at: NOW,
        kind: "peer_unreachable",
        retryable: true,
        durationMs: 125,
      },
    });
    expect(latestManualAttempt(second["pair-1"])).toEqual({
      at: NOW,
      sent: 0,
      received: 0,
      durationMs: 125,
    });
  });

  /** INV-12: the Tauri boundary has already dropped the daemon's sentence; the
   *  row retains only its localized kind and Rust-owned retry decision. */
  it("stores the structured failure without reconstructing text", () => {
    const health = noteSync(
      {},
      [result({ error: { code: "peer_not_found", retryable: false } })],
      NOW,
    );
    expect(health["pair-1"]?.failure).toEqual({
      at: NOW,
      kind: "peer_not_found",
      retryable: false,
      durationMs: null,
    });
    expect(JSON.stringify(health)).not.toMatch(/Users/);
  });

  it("records each peer of a partial run on its own terms", () => {
    const health = noteSync(
      {},
      [
        result({ pairing_id: "pair-1", received: 4 }),
        result({
          pairing_id: "pair-2",
          error: { code: "future_failure", retryable: true },
        }),
      ],
      NOW,
    );
    expect(health["pair-1"]?.failure).toBeUndefined();
    expect(health["pair-1"]?.success).toEqual({
      at: NOW,
      sent: 0,
      received: 4,
      durationMs: null,
    });
    expect(health["pair-2"]?.success).toBeUndefined();
    expect(health["pair-2"]?.failure?.at).toBe(NOW);
  });
});

describe("the pairing cap", () => {
  /** `copypaste_p2p::peers::MAX_PAIRINGS`. It is a refusal and not an
   *  eviction, so the screen has to say so before a code is minted. */
  it("is the sixteen the peer store enforces", () => {
    expect(MAX_PAIRINGS).toBe(16);
    expect(atPairingCap(15)).toBe(false);
    expect(atPairingCap(16)).toBe(true);
    expect(atPairingCap(17)).toBe(true);
  });
});
