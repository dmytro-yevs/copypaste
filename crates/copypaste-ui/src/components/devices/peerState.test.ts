/**
 * The six states, and the reason there are six rather than two.
 *
 * Each row here is a different remedy: a code nobody has redeemed, a device
 * this one holds no address for, a device sitting on the network not syncing,
 * and a device that is simply elsewhere were all rendered as one grey dot
 * (`ui-parity-audit.md` finding 4).
 */
import { describe, expect, it } from "vitest";

import {
  MAX_PAIRINGS,
  STALE_AFTER_MS,
  atPairingCap,
  noteSync,
  peerState,
  unsettledFailure,
} from "@/components/devices/peerState";
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

  /** A gap while the device is not on the network is the cadence working, not
   *  a fault — so age must not turn `away` into `stalled`. */
  it("does not fault a device that is off the network for being out of date", () => {
    expect(
      peerState(
        peer({ last_seen_ms: NOW - 30 * STALE_AFTER_MS, online: false }),
        undefined,
        NOW,
      ),
    ).toBe("away");
  });
});

describe("a failure this screen watched happen", () => {
  it("outranks the structural states, because it is evidence", () => {
    const state = peerState(
      peer({ last_addr: null, last_seen_ms: NOW - 1000 }),
      { failure: { at: NOW, kind: "peer_unreachable" } },
      NOW,
    );
    expect(state).toBe("failing");
  });

  /**
   * The daemon syncs on its own cadence and reports per-peer failure nowhere
   * the wire can carry it. A remembered failure that a later session has
   * already settled would therefore sit on the row for ever.
   */
  it("is settled by a session that succeeded after it", () => {
    const health = { failure: { at: NOW - 1000, kind: "peer_failed" } } as const;
    expect(peerState(peer({ last_seen_ms: NOW }), health, NOW)).toBe("synced");
    expect(unsettledFailure(peer({ last_seen_ms: NOW }), health)).toBeUndefined();
  });

  /** A run started here settles it as much as the daemon's own cadence does:
   *  `last_seen_ms` only moves on the daemon's next poll. */
  it("is settled by a later run of its own that worked", () => {
    const stale = peer({ last_seen_ms: NOW - 60_000 });
    const health = {
      failure: { at: NOW - 1000, kind: "peer_unreachable" },
      success: { at: NOW, sent: 2, received: 0 },
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
    const first = noteSync({}, [result({ sent: 3, received: 1 })], NOW - 5000);
    const second = noteSync(
      first,
      [result({ error: "the peer stopped responding" })],
      NOW,
    );

    expect(second["pair-1"]).toEqual({
      success: { at: NOW - 5000, sent: 3, received: 1 },
      failure: { at: NOW, kind: "peer_unreachable" },
    });
  });

  /** INV-12: the daemon's per-peer text can name the socket path, so only a
   *  kind is kept — the raw string never reaches a component. */
  it("stores a kind, never the daemon's sentence", () => {
    const health = noteSync(
      {},
      [result({ error: "no such paired device /Users/someone/.copypaste.sock" })],
      NOW,
    );
    expect(health["pair-1"]?.failure).toEqual({ at: NOW, kind: "peer_not_found" });
    expect(JSON.stringify(health)).not.toMatch(/Users/);
  });

  it("records each peer of a partial run on its own terms", () => {
    const health = noteSync(
      {},
      [
        result({ pairing_id: "pair-1", received: 4 }),
        result({ pairing_id: "pair-2", error: "whatever the daemon said" }),
      ],
      NOW,
    );
    expect(health["pair-1"]?.failure).toBeUndefined();
    expect(health["pair-1"]?.success).toEqual({ at: NOW, sent: 0, received: 4 });
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
