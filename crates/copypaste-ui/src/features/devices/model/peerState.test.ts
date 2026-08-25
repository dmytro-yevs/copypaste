import { describe, expect, it } from "vitest";

import type { PeerInfo } from "@/lib/ipc";
import { peerState, STALE_AFTER_MS, type PeerHealth } from "./peerState";

const NOW = 10_000_000;
const PEER = {
  pairing_id: "peer-1",
  name: "Sleeping phone",
  online: false,
  last_addr: "192.0.2.8:47654",
  last_seen_ms: NOW - STALE_AFTER_MS - 1,
  last_sync_at: null,
  paired_at: NOW - STALE_AFTER_MS * 2,
} as PeerInfo;

function failure(kind: NonNullable<PeerHealth["failure"]>["kind"]): PeerHealth {
  return {
    failure: {
      at: NOW,
      kind,
      retryable: kind === "peer_unreachable" || kind === "peer_failed",
      durationMs: 300,
    },
  };
}

describe("peerState", () => {
  it("treats an offline stale peer as away rather than broken", () => {
    expect(peerState(PEER, undefined, NOW)).toBe("away");
  });

  it("keeps an unreachable result transient while surfacing trust failures", () => {
    expect(peerState(PEER, failure("peer_unreachable"), NOW)).toBe("away");
    expect(peerState(PEER, failure("auth_failed"), NOW)).toBe("failing");
  });

  it("keeps pairing and sync evidence ahead of ordinary offline presence", () => {
    expect(
      peerState({ ...PEER, last_seen_ms: 0 }, undefined, NOW),
    ).toBe("waiting");
    expect(peerState(PEER, failure("peer_failed"), NOW)).toBe("failing");
  });
});
