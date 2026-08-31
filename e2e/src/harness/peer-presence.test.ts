import { describe, expect, it } from "vitest";

import type {
  DevicePresence,
  PeerInfo,
} from "../../../crates/copypaste-ui/src/generated/ipc.js";
import { peerPresenceSnapshot } from "./peer-presence.js";

function peer(
  state: DevicePresence,
  freshUntilMs: number | null,
): PeerInfo {
  return {
    pairing_id: "peer-1",
    name: "Nearby peer",
    last_addr: null,
    last_seen_ms: 1,
    online: state === "online",
    details: {
      profile: null,
      endpoint: null,
      latency: null,
      presence: {
        state,
        last_seen_ms: 1,
        provenance: "observed",
        trust: "local",
        observed_at_ms: 1,
        fresh_until_ms: freshUntilMs,
      },
      public_ip: { availability: "unavailable" },
      geo: { availability: "unavailable" },
    },
  };
}

describe("peer presence harness", () => {
  it.each([
    ["online", 2],
    ["offline", 2],
    ["unknown", null],
  ] as const)("preserves authoritative %s presence", (state, freshUntilMs) => {
    expect(peerPresenceSnapshot(peer(state, freshUntilMs))).toEqual({
      state,
      freshUntilMs,
    });
  });

  it("does not infer a presence state from the legacy online projection", () => {
    const unknown = peer("unknown", null);
    expect(unknown.online).toBe(false);
    expect(peerPresenceSnapshot(unknown)?.state).toBe("unknown");
  });

  it("does not manufacture presence when the observation is absent", () => {
    expect(
      peerPresenceSnapshot({ ...peer("online", 2), details: undefined }),
    ).toBeNull();
  });
});
