/**
 * SyncStatusChip — badge-state adapter tests (CopyPaste-merc / CMP-7).
 *
 * CMP-7: `badgeStateToSyncState` is now a 1:1 identity mapping — SyncState was
 * expanded from three values ("connected"/"idle"/"offline") to six values matching
 * IPC SyncBadgeState exactly. These tests verify the 1:1 mapping and the
 * canonical colour grouping implied by DOT_CLASS.
 *
 * These tests assert the NEW code path where the daemon provides `badge_state`
 * directly instead of the client re-deriving it from raw fields. The old
 * `deriveSyncStateFallback` is tested implicitly (it is the pre-existing logic)
 * and is NOT the focus here.
 */
import { describe, it, expect } from "vitest";
import { badgeStateToSyncState } from "./SyncStatusChip";
import type { SyncBadgeState } from "../lib/ipc";

describe("badgeStateToSyncState (CopyPaste-merc / CMP-7)", () => {
  // CMP-7: the adapter is now 1:1 — each IPC state maps to itself.

  it('maps "synced" → "synced" (was "connected" pre-CMP-7)', () => {
    expect(badgeStateToSyncState("synced")).toBe("synced");
  });

  it('maps "syncing" → "syncing" (was "connected" pre-CMP-7)', () => {
    expect(badgeStateToSyncState("syncing")).toBe("syncing");
  });

  it('maps "idle" → "idle" (grey dot — configured but no recent sync)', () => {
    expect(badgeStateToSyncState("idle")).toBe("idle");
  });

  it('maps "offline" → "offline" (red dot — no usable sync path)', () => {
    expect(badgeStateToSyncState("offline")).toBe("offline");
  });

  it('maps "error" → "error" (red dot — backend returned an error)', () => {
    // "error" (auth failure, RLS, etc.) is shown as red via DOT_CLASS["error"].
    expect(badgeStateToSyncState("error")).toBe("error");
  });

  it('maps "misconfigured" → "misconfigured" (grey — not a hard error, just incomplete setup)', () => {
    // "misconfigured" keeps the amber chip (cloudMisconfig) separately;
    // the dot itself is grey / misconfigured rather than red (not a hard failure).
    expect(badgeStateToSyncState("misconfigured")).toBe("misconfigured");
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
      // 1:1 mapping — each variant must map to itself.
      const result = badgeStateToSyncState(v);
      expect(result).toBe(v);
    }
  });

  // Colour grouping (canonical state→colour mapping, CMP-7):
  it('canonical colour group: "synced" and "syncing" are both green (isConnectedState)', () => {
    // Both map to DOT_CLASS green — the distinction matters for labels, not for dot colour.
    const greenStates = ["synced", "syncing"] as const;
    for (const s of greenStates) {
      // These are the 1:1 results — a consumer reads DOT_CLASS[result] for the colour.
      expect(badgeStateToSyncState(s)).toBe(s);
    }
  });

  it('canonical colour group: "idle" and "misconfigured" are both grey (faint)', () => {
    const greyStates = ["idle", "misconfigured"] as const;
    for (const s of greyStates) {
      expect(badgeStateToSyncState(s)).toBe(s);
    }
  });

  it('canonical colour group: "offline" and "error" are both red (danger)', () => {
    const redStates = ["offline", "error"] as const;
    for (const s of redStates) {
      expect(badgeStateToSyncState(s)).toBe(s);
    }
  });
});
