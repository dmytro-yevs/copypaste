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
  noteAttempts,
  peerState,
} from "@/components/devices/peerState";
import { peer } from "@/test/harness";

const NOW = 1_800_000_000_000;

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
      { at: NOW, failed: true },
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
    expect(
      peerState(peer({ last_seen_ms: NOW }), { at: NOW - 1000, failed: true }, NOW),
    ).toBe("synced");
  });

  it("is not recorded for a run that succeeded", () => {
    const attempts = noteAttempts(
      {},
      [
        { pairing_id: "pair-1", error: null },
        { pairing_id: "pair-2", error: "whatever the daemon said" },
      ],
      NOW,
    );
    expect(attempts["pair-1"]).toEqual({ at: NOW, failed: false });
    expect(attempts["pair-2"]).toEqual({ at: NOW, failed: true });
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
