import { describe, expect, it } from "vitest";
import { isPeerStalled, PEER_STALL_THRESHOLD_MS } from "./SyncStatusChip";
import type { PairedDevice } from "../lib/ipc";

// ---------------------------------------------------------------------------
// isPeerStalled — CopyPaste-ptgcc: rekey_failures supersedes the 30-minute
// staleness wait so a peer with a broken pairwise key is flagged immediately.
// ---------------------------------------------------------------------------

function makePeer(overrides: Partial<PairedDevice> = {}): PairedDevice {
  return {
    fingerprint: "peer00112233445566778899aabbccddeeff00112233445566778899aabbcc",
    name: "Test Peer",
    added_at: 1700000000,
    address: "192.168.1.10:7878",
    sync_key_b64: null,
    model: "MacBook Pro",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.10",
    public_ip: "203.0.113.5",
    first_sync_at: null,
    last_sync_at: null,
    online: true,
    last_seen_secs: 5,
    latency_ms: 10,
    trust: "verified",
    transport: "p2p",
    supabase_account_id: null,
    ...overrides,
  };
}

describe("isPeerStalled", () => {
  it("flags a peer with rekey_failures > 0 even with a fresh last_sync_at", () => {
    const nowMs = Date.now();
    const peer = makePeer({
      rekey_failures: 1,
      last_sync_at: Math.floor(nowMs / 1000), // synced just now
    });
    expect(isPeerStalled(peer, nowMs)).toBe(true);
  });

  it("does not flag a peer with rekey_failures 0/absent and a fresh last_sync_at", () => {
    const nowMs = Date.now();
    const peerZero = makePeer({
      rekey_failures: 0,
      last_sync_at: Math.floor(nowMs / 1000),
    });
    const peerAbsent = makePeer({
      last_sync_at: Math.floor(nowMs / 1000),
    });
    expect(isPeerStalled(peerZero, nowMs)).toBe(false);
    expect(isPeerStalled(peerAbsent, nowMs)).toBe(false);
  });

  it("still flags a peer stale by 30-minute last_sync_at check with rekey_failures absent", () => {
    const nowMs = Date.now();
    const staleSyncSecs = Math.floor(
      (nowMs - PEER_STALL_THRESHOLD_MS - 1_000) / 1000,
    );
    const peer = makePeer({ last_sync_at: staleSyncSecs });
    expect(isPeerStalled(peer, nowMs)).toBe(true);
  });
});
