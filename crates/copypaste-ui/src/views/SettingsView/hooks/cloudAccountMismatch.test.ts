/**
 * CopyPaste-yw2k: cloudAccountMismatch detection logic.
 *
 * Tests the mismatch-detection predicate that `useSettingsState` uses to set
 * `cloudAccountMismatch`. The logic is:
 *   - false when localId is null/absent (cloud-sync off or not signed in)
 *   - false when no peers carry a supabase_account_id (legacy builds)
 *   - false when all peers with ids match the local id
 *   - true when ANY peer's supabase_account_id differs from the local id
 *
 * We test the raw logic (not the hook) so there's no React context dependency.
 */
import { describe, it, expect } from "vitest";
import type { PairedDevice } from "../../../lib/ipc";

/**
 * Mirror of the mismatch-detection predicate in `useSettingsState`.
 *
 * Extracted here to keep the test portable: if the hook inline is ever
 * factored out to a helper, update this to import it instead.
 */
function detectMismatch(
  localId: string | null | undefined,
  peers: PairedDevice[] | null,
): boolean {
  if (localId == null) return false;
  if (peers == null) return false;
  return peers.some(
    (p) => p.supabase_account_id != null && p.supabase_account_id !== localId,
  );
}

// ---------------------------------------------------------------------------
// Minimal PairedDevice fixture factory.
// ---------------------------------------------------------------------------
function makePeer(overrides: Partial<PairedDevice> = {}): PairedDevice {
  return {
    fingerprint: "aa:bb:cc:dd:ee:ff:00:11",
    name: "Test Device",
    added_at: 0,
    address: null,
    sync_key_b64: null,
    model: null,
    os_version: null,
    app_version: null,
    local_ip: null,
    public_ip: null,
    first_sync_at: null,
    last_sync_at: null,
    online: false,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("cloudAccountMismatch detection (CopyPaste-yw2k)", () => {
  it("returns false when local account id is null (cloud-sync off)", () => {
    const peers = [makePeer({ supabase_account_id: "proj_abc/uid_1" })];
    expect(detectMismatch(null, peers)).toBe(false);
  });

  it("returns false when local account id is undefined (absent)", () => {
    const peers = [makePeer({ supabase_account_id: "proj_abc/uid_1" })];
    expect(detectMismatch(undefined, peers)).toBe(false);
  });

  it("returns false when peers list is null (not yet loaded)", () => {
    expect(detectMismatch("proj_abc/uid_local", null)).toBe(false);
  });

  it("returns false when peers list is empty", () => {
    expect(detectMismatch("proj_abc/uid_local", [])).toBe(false);
  });

  it("returns false when no peer carries a supabase_account_id (legacy builds)", () => {
    const peers = [
      makePeer({ supabase_account_id: null }),
      makePeer({ supabase_account_id: undefined }),
      makePeer({}), // field absent → undefined
    ];
    expect(detectMismatch("proj_abc/uid_local", peers)).toBe(false);
  });

  it("returns false when all peers with ids match the local id", () => {
    const id = "proj_shared/uid_same";
    const peers = [
      makePeer({ supabase_account_id: id }),
      makePeer({ supabase_account_id: id }),
      makePeer({ supabase_account_id: null }), // legacy — ignored
    ];
    expect(detectMismatch(id, peers)).toBe(false);
  });

  it("returns true when one peer's supabase_account_id differs from local id", () => {
    const peers = [
      makePeer({ supabase_account_id: "proj_other/uid_99" }),
    ];
    expect(detectMismatch("proj_abc/uid_local", peers)).toBe(true);
  });

  it("returns true when ANY peer differs even if others match", () => {
    const localId = "proj_abc/uid_local";
    const peers = [
      makePeer({ supabase_account_id: localId }), // matches
      makePeer({ supabase_account_id: "proj_other/uid_99" }), // differs
    ];
    expect(detectMismatch(localId, peers)).toBe(true);
  });

  it("returns false when local id matches the only peer that has an id", () => {
    const localId = "proj_abc/uid_local";
    const peers = [
      makePeer({ supabase_account_id: null }),
      makePeer({ supabase_account_id: localId }),
    ];
    expect(detectMismatch(localId, peers)).toBe(false);
  });
});
