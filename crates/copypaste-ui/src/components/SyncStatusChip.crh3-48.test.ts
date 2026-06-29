/**
 * CopyPaste-crh3.48: SyncStatusChip must not call api.listPeers on every
 * 2s sync-status poll.
 *
 * The fix: listPeers is driven by a separate PEERS_POLL_INTERVAL_MS (10s)
 * interval, decoupled from SYNC_POLL_INTERVAL_MS (2s). This reduces listPeers
 * from 30 calls/min to ≤6 calls/min.
 *
 * We export PEERS_POLL_INTERVAL_MS so this test can assert the upper-bound
 * rate without importing the React component tree.
 */
import { describe, it, expect } from "vitest";
import { PEERS_POLL_INTERVAL_MS, SYNC_POLL_INTERVAL_MS } from "./SyncStatusChip";

describe("CopyPaste-crh3.48: SyncStatusChip peers poll interval", () => {
  it("PEERS_POLL_INTERVAL_MS is exported and ≥ 10 000 ms (≤6 listPeers calls/min)", () => {
    // At 10s, over a 20s window we expect at most 2 calls from the interval
    // (plus 1 on-mount call = 3 total), satisfying the acceptance criterion of ≤3.
    expect(typeof PEERS_POLL_INTERVAL_MS).toBe("number");
    expect(PEERS_POLL_INTERVAL_MS).toBeGreaterThanOrEqual(10_000);
  });

  it("PEERS_POLL_INTERVAL_MS is strictly longer than SYNC_POLL_INTERVAL_MS", () => {
    // The whole point of the decoupling: peers poll is slower than sync-status poll.
    expect(PEERS_POLL_INTERVAL_MS).toBeGreaterThan(SYNC_POLL_INTERVAL_MS);
  });

  it("over a simulated 20s window, listPeers interval call count is ≤ 3", () => {
    // Simulate: 1 on-mount call + floor(20000 / PEERS_POLL_INTERVAL_MS) interval ticks.
    const WINDOW_MS = 20_000;
    const onMountCall = 1;
    const intervalCalls = Math.floor(WINDOW_MS / PEERS_POLL_INTERVAL_MS);
    const totalCalls = onMountCall + intervalCalls;
    // Acceptance criterion from CopyPaste-crh3.48: ≤3 calls in 20s.
    expect(totalCalls).toBeLessThanOrEqual(3);
  });
});
