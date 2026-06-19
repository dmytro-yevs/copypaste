/**
 * SyncStatusChip — badge-state adapter tests (CopyPaste-merc).
 *
 * Verifies the `badgeStateToSyncState` adapter that maps the daemon-computed
 * canonical SyncBadgeState to the component's internal SyncState display model.
 *
 * These tests assert the NEW code path where the daemon provides `badge_state`
 * directly instead of the client re-deriving it from raw fields. The old
 * `deriveSyncStateFallback` is tested implicitly (it is the pre-existing logic)
 * and is NOT the focus here.
 */
import { describe, it, expect } from "vitest";
import { badgeStateToSyncState } from "./SyncStatusChip";
import type { SyncBadgeState } from "../lib/ipc";

describe("badgeStateToSyncState (CopyPaste-merc)", () => {
  it('maps "synced" → "connected" (green dot)', () => {
    expect(badgeStateToSyncState("synced")).toBe("connected");
  });

  it('maps "syncing" → "connected" (also green, actively in-flight)', () => {
    expect(badgeStateToSyncState("syncing")).toBe("connected");
  });

  it('maps "idle" → "idle" (grey dot — configured but no recent sync)', () => {
    expect(badgeStateToSyncState("idle")).toBe("idle");
  });

  it('maps "offline" → "offline" (red dot — no usable sync path)', () => {
    expect(badgeStateToSyncState("offline")).toBe("offline");
  });

  it('maps "error" → "offline" (red dot — backend returned an error)', () => {
    // "error" (auth failure, RLS, etc.) is shown as red, same as offline,
    // because from the user's perspective sync is not working.
    expect(badgeStateToSyncState("error")).toBe("offline");
  });

  it('maps "misconfigured" → "idle" (grey — not a hard error, just incomplete setup)', () => {
    // "misconfigured" keeps the amber chip (cloudMisconfig) separately;
    // the dot itself is grey / idle rather than red (not a hard failure).
    expect(badgeStateToSyncState("misconfigured")).toBe("idle");
  });

  it("covers all known SyncBadgeState variants without falling through to default", () => {
    // Exhaustive check: add any new variant here when it is added to the type.
    const variants: SyncBadgeState[] = [
      "synced",
      "syncing",
      "idle",
      "offline",
      "error",
      "misconfigured",
    ];
    for (const v of variants) {
      // Every known variant must produce a valid SyncState — no undefined / throw.
      const result = badgeStateToSyncState(v);
      expect(["connected", "idle", "offline"]).toContain(result);
    }
  });
});
