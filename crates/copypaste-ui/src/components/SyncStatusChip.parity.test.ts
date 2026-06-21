/**
 * SyncStatusChip — parity tests for CopyPaste-5qbe offline signal unification.
 *
 * CANONICAL RULE (CopyPaste-5qbe):
 *   "Offline" is determined exclusively by daemon/IPC-reported connectivity.
 *   OS-level network state (navigator.onLine, ConnectivityManager) is NOT used
 *   on web — the IPC socket failure is the only offline signal.
 *
 * CMP-7 update: the display model is now SIX states matching IPC SyncBadgeState
 * exactly (1:1 mapping). The three-colour grouping is:
 *   - green  : "synced" or "syncing"     — dot bg-ide-success
 *   - grey   : "idle" or "misconfigured" — dot bg-ide-faint
 *   - red    : "offline" or "error"      — dot bg-ide-danger
 *
 *   Both web and Android must use this colour rule identically. Android uses
 *   DevicesOnlineState (daemon-derived sync connectivity) as its equivalent of
 *   the IPC socket liveness signal, with OS network as a secondary signal ONLY
 *   to distinguish NetworkOffline from DaemonUnreachable — both still show red.
 *   Android additionally now maps IpcSyncBadgeState.IDLE → grey (Idle display
 *   state), matching web's grey "idle" dot, instead of the former red
 *   DaemonUnreachable mapping.
 *
 * These tests assert the WEB side of the canonical rule and serve as
 * documentation of the parity contract between platforms.
 */
import { describe, it, expect } from "vitest";
import { badgeStateToSyncState } from "./SyncStatusChip";

describe("SyncStatusChip offline-signal parity (CopyPaste-5qbe / CMP-7)", () => {
  // ── Canonical rule: daemon badge_state drives the dot colour ──────────────

  it("synced → synced (green): IPC-reported sync working", () => {
    expect(badgeStateToSyncState("synced")).toBe("synced");
  });

  it("syncing → syncing (green): IPC-reported sync in-flight", () => {
    expect(badgeStateToSyncState("syncing")).toBe("syncing");
  });

  it("idle → idle (grey): IPC says configured but no recent activity — NOT red", () => {
    // PARITY: web uses grey for idle, not red.
    // Android IpcSyncBadgeState.IDLE must also map to grey (Idle) after CopyPaste-5qbe fix.
    expect(badgeStateToSyncState("idle")).toBe("idle");
  });

  it("misconfigured → misconfigured (grey): incomplete setup — not a hard failure, NOT red", () => {
    // PARITY: cloudMisconfig chip provides the additional warning;
    // the dot itself is grey. Android must match.
    expect(badgeStateToSyncState("misconfigured")).toBe("misconfigured");
  });

  it("offline → offline (red): daemon cannot reach sync backend", () => {
    expect(badgeStateToSyncState("offline")).toBe("offline");
  });

  it("error → error (red): backend returned auth/RLS/relay error", () => {
    // CMP-7: "error" is now its own state (not folded into "offline").
    // Both "offline" and "error" show red via DOT_CLASS, but the state label
    // is preserved for future label/tooltip differentiation.
    expect(badgeStateToSyncState("error")).toBe("error");
  });

  // ── Colour grouping contracts ─────────────────────────────────────────────

  it("synced and syncing are both green-class states (isConnectedState)", () => {
    // Verify neither maps to a grey or red state.
    expect(badgeStateToSyncState("synced")).not.toBe("idle");
    expect(badgeStateToSyncState("synced")).not.toBe("offline");
    expect(badgeStateToSyncState("syncing")).not.toBe("idle");
    expect(badgeStateToSyncState("syncing")).not.toBe("offline");
  });

  it("idle and misconfigured are both grey-class states — neither is red", () => {
    expect(badgeStateToSyncState("idle")).not.toBe("offline");
    expect(badgeStateToSyncState("idle")).not.toBe("error");
    expect(badgeStateToSyncState("misconfigured")).not.toBe("offline");
    expect(badgeStateToSyncState("misconfigured")).not.toBe("error");
  });

  // ── OS network is NOT the offline signal on web ───────────────────────────

  it("idle state uses IPC-reported state, never navigator.onLine", () => {
    // The web component does not import or check navigator.onLine.
    // "offline" means the IPC socket call itself threw (syncResult.status === 'rejected').
    // This test documents the contract: if badge_state is "idle", the dot is grey
    // regardless of whether the OS has network connectivity.
    expect(badgeStateToSyncState("idle")).not.toBe("offline");
  });

  it("idle state shows grey dot (not red) when daemon is reachable but sync stale", () => {
    // Stale-sync-with-reachable-daemon → grey (idle), never red.
    // This mirrors the fallback deriveSyncStateFallback behaviour:
    // deviceCount > 0 but no recent lastSyncMs → "idle" (grey).
    expect(badgeStateToSyncState("idle")).toBe("idle");
  });
});
