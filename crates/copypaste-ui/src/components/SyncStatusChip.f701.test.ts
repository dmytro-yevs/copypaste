/**
 * CopyPaste-f701: SyncStatusChip must reflect offline faster (shorter poll interval).
 *
 * The fix: POLL_INTERVAL_MS reduced from 10 000 ms to 2 000 ms so the chip stops
 * showing a stale "connected" (green) for up to 10 s after the daemon goes offline.
 *
 * We export the constant so this test can assert the new upper bound without
 * importing the whole React component tree.
 */
import { describe, it, expect } from "vitest";
import { SYNC_POLL_INTERVAL_MS } from "./SyncStatusChip";

describe("CopyPaste-f701: SyncStatusChip poll interval", () => {
  it("SYNC_POLL_INTERVAL_MS is exported and ≤ 3 000 ms so offline is reflected quickly", () => {
    // The old value was 10 000 ms, which meant stale "connected" for up to 10 s.
    // The new value must be ≤ 3 000 ms so the worst-case stale window is acceptable.
    expect(typeof SYNC_POLL_INTERVAL_MS).toBe("number");
    expect(SYNC_POLL_INTERVAL_MS).toBeGreaterThan(0);
    expect(SYNC_POLL_INTERVAL_MS).toBeLessThanOrEqual(3000);
  });
});
