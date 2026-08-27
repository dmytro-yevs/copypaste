import { describe, expect, it } from "vitest";

import type { PeerInfo } from "@/lib/ipc";
import { observedPresence, peerState, STALE_AFTER_MS, type PeerHealth } from "./peerState";

const NOW = 10_000_000;
const PEER = {
  pairing_id: "peer-1",
  name: "Sleeping phone",
  online: false,
  last_addr: "192.0.2.8:47654",
  last_seen_ms: NOW - STALE_AFTER_MS - 1,
  last_sync_at: null,
  paired_at: NOW - STALE_AFTER_MS * 2,
  details: {
    profile: null,
    endpoint: null,
    latency: null,
    presence: {
      state: "offline",
      last_seen_ms: NOW,
      provenance: "observed",
      trust: "local",
      observed_at_ms: 0,
      fresh_until_ms: Number.MAX_SAFE_INTEGER,
    },
    public_ip: { availability: "unavailable" },
    geo: { availability: "unavailable" },
  },
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
  it.each([
    ["online", 0, 1, "online"],
    ["offline", 0, 1, "offline"],
    ["unknown", 0, 1, "unknown"],
    ["online", 0, 0, "unknown"],
    ["online", 2, 3, "unknown"],
    ["online", 0, null, "unknown"],
  ] as const)("resolves %s observations fail-closed", (state, observed, fresh, expected) => {
    expect(observedPresence({ ...PEER.details!.presence!, state, observed_at_ms: observed, fresh_until_ms: fresh }, 1)).toBe(expected);
  });

  it("treats a missing observation as unknown", () => {
    expect(observedPresence(undefined, NOW)).toBe("unknown");
  });
  it("treats an offline stale peer as away rather than broken", () => {
    expect(peerState(PEER, undefined, NOW)).toBe("away");
  });

  it("keeps reachability failures separate from offline presence", () => {
    expect(peerState(PEER, failure("peer_unreachable"), NOW)).toBe("failing");
    expect(peerState(PEER, failure("auth_failed"), NOW)).toBe("failing");
  });

  it("keeps pairing and sync evidence ahead of ordinary offline presence", () => {
    expect(
      peerState({ ...PEER, last_seen_ms: 0 }, undefined, NOW),
    ).toBe("waiting");
    expect(peerState(PEER, failure("peer_failed"), NOW)).toBe("failing");
  });
});
