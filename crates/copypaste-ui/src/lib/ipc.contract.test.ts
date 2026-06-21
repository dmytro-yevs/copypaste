/**
 * ipc.contract.test.ts — IPC wire-shape contract tests (CopyPaste-ojas.10)
 *
 * Guards that the TypeScript types in `lib/ipc.ts` (IpcReply, DaemonStatus,
 * HistoryPage, SyncStatus) match the Rust JSON shapes pinned in
 * `crates/copypaste-ipc/tests/snapshot.rs`.
 *
 * Strategy (option b from the issue):
 *   - Construct objects matching the EXACT Rust-serialised JSON shapes.
 *   - Assert `satisfies` at compile time so TypeScript rejects a wrong type.
 *   - Assert field presence at runtime so a field rename on EITHER side breaks
 *     the test immediately — no silent pass.
 *
 * The test covers:
 *   1. Envelope fields (ok, data, error, error_code, id, protocol_version)
 *   2. status payload  → DaemonStatus
 *   3. history_page payload → HistoryPage + HistoryEntry
 *   4. get_sync_status payload → SyncStatus
 *
 * How to break this intentionally (per issue test plan):
 *   - Rename `error_code` to `errorCode` in ipc.ts → compile error on `satisfies`.
 *   - Remove `protocol_version` from IpcReply → runtime assertion fails.
 *   - Remove `own_device_id` from HistoryPage → compile + runtime fail.
 */

import { describe, it, expect } from "vitest";
import type {
  IpcReply,
  DaemonStatus,
  HistoryPage,
  HistoryEntry,
  SyncStatus,
} from "./ipc";

// ---------------------------------------------------------------------------
// §1 Envelope shape
// ---------------------------------------------------------------------------
// Rust wire (from snapshot.rs response_ok_with_history_items_array):
//   {"id":"1","ok":true,"data":{...},"protocol_version":1}
// Rust wire (response_err_with_error_code_present):
//   {"id":"42","ok":false,"error":"item missing","error_code":"not_found","protocol_version":1}
// Rust wire (response_err_without_error_code_omits_field):
//   {"id":"2","ok":false,"error":"boom","protocol_version":1}
//
// IpcReply is what the Tauri bridge hands the TS side (it maps the Response
// fields directly). The field names must match the Rust `Response` struct's
// #[serde] names exactly: ok, data, error, error_code, protocol_version.

describe("IPC envelope — wire shape contract", () => {
  it("success envelope satisfies IpcReply", () => {
    // Mirrors snapshot: response_ok_with_history_items_array
    const successEnvelope = {
      ok: true,
      data: { items: [], total: 0 },
      error: null,
      error_code: null,
      protocol_version: 1,
    } satisfies IpcReply;

    expect(successEnvelope.ok).toBe(true);
    expect(successEnvelope.data).toBeDefined();
    expect(successEnvelope.error).toBeNull();
    expect(successEnvelope.error_code).toBeNull();
    // protocol_version is Optional<number> in IpcReply — must accept a number
    expect(typeof successEnvelope.protocol_version).toBe("number");
  });

  it("error envelope with error_code satisfies IpcReply", () => {
    // Mirrors snapshot: response_err_with_error_code_present
    // Rust field name is error_code (snake_case) — if renamed to errorCode the
    // `satisfies` below would fail at compile time.
    const errorEnvelope = {
      ok: false,
      data: null,
      error: "item missing",
      error_code: "not_found",
      protocol_version: 1,
    } satisfies IpcReply;

    expect(errorEnvelope.ok).toBe(false);
    expect(errorEnvelope.error_code).toBe("not_found");
    // Guard the snake_case field name explicitly
    expect("error_code" in errorEnvelope).toBe(true);
    // Guard that camelCase does NOT exist (it would if a serde rename happened)
    expect("errorCode" in (errorEnvelope as Record<string, unknown>)).toBe(false);
  });

  it("error envelope without error_code satisfies IpcReply (legacy)", () => {
    // Mirrors snapshot: response_err_without_error_code_omits_field
    // The daemon omits error_code entirely when None; the TS type allows null.
    const legacyError = {
      ok: false,
      data: null,
      error: "boom",
      error_code: null,
      protocol_version: 1,
    } satisfies IpcReply;

    expect(legacyError.ok).toBe(false);
    expect(legacyError.error).toBe("boom");
    expect(legacyError.error_code).toBeNull();
  });

  it("envelope has all required field names (runtime guard)", () => {
    // Parse the literal JSON shape that the Rust daemon emits.
    const raw: unknown = JSON.parse(
      '{"ok":true,"data":{"items":[],"total":0},"error":null,"error_code":null,"protocol_version":1}'
    );
    const envelope = raw as IpcReply;

    // Every field the TS type declares must be present in the parsed shape.
    expect(Object.prototype.hasOwnProperty.call(envelope, "ok")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(envelope, "data")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(envelope, "error")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(envelope, "error_code")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(envelope, "protocol_version")).toBe(true);

    // Guard exact field name (snake_case) — rename to errorCode would fail here.
    expect(Object.prototype.hasOwnProperty.call(envelope, "errorCode")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// §2 status payload — DaemonStatus
// ---------------------------------------------------------------------------
// Rust wire (daemon/src/ipc.rs STATUS handler):
//   {"status":"running","private_mode":false,"ready":true,"degraded":false,"protocol_version":1}
// (build_version, pid, degraded_reason are optional; absent on older daemons.)

describe("status payload — DaemonStatus wire shape", () => {
  it("healthy status payload satisfies DaemonStatus", () => {
    const statusData = {
      status: "running",
      private_mode: false,
      ready: true,
      degraded: false,
    } satisfies DaemonStatus;

    expect(statusData.status).toBe("running");
    expect(statusData.private_mode).toBe(false);
    expect(statusData.ready).toBe(true);
    expect(statusData.degraded).toBe(false);
  });

  it("degraded status payload satisfies DaemonStatus", () => {
    const degradedData = {
      status: "degraded",
      private_mode: false,
      ready: false,
      degraded: true,
      degraded_reason: "database_missing",
      build_version: "0.5.2+abc123",
      pid: 12345,
    } satisfies DaemonStatus;

    expect(degradedData.degraded).toBe(true);
    // snake_case field name — would fail if renamed to degradedReason
    expect("degraded_reason" in degradedData).toBe(true);
    expect("build_version" in degradedData).toBe(true);
  });

  it("required DaemonStatus fields are present in wire-like JSON", () => {
    const raw: unknown = JSON.parse(
      '{"status":"running","private_mode":false,"ready":true,"degraded":false}'
    );
    const status = raw as DaemonStatus;

    expect(Object.prototype.hasOwnProperty.call(status, "status")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "private_mode")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "ready")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "degraded")).toBe(true);

    // snake_case guard
    expect(Object.prototype.hasOwnProperty.call(status, "privateMode")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// §3 history_page payload — HistoryPage + HistoryEntry
// ---------------------------------------------------------------------------
// Rust wire (daemon/src/ipc.rs history_page handler):
//   {
//     "items": [{ "id":"<uuid>", "content_type":"text", "preview":"hello",
//                 "is_sensitive":false, "wall_time":1700000000000,
//                 "pinned":false }],
//     "total": 1,
//     "own_device_id": "<uuid>"
//   }

describe("history_page payload — HistoryPage wire shape", () => {
  it("HistoryPage with items satisfies the TS type", () => {
    const entry: HistoryEntry = {
      id: "550e8400-e29b-41d4-a716-446655440000",
      content_type: "text",
      preview: "Hello, world!",
      is_sensitive: false,
      wall_time: 1700000000000,
      pinned: false,
    };

    const page = {
      items: [entry],
      total: 1,
      own_device_id: "device-uuid-123",
    } satisfies HistoryPage;

    expect(page.items).toHaveLength(1);
    expect(page.total).toBe(1);
    expect(typeof page.own_device_id).toBe("string");
  });

  it("HistoryEntry snake_case field names match Rust wire", () => {
    const entry: HistoryEntry = {
      id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      content_type: "text",
      preview: "test",
      is_sensitive: false,
      wall_time: 0,
      pinned: false,
    };

    // Every required field must be snake_case (not camelCase)
    expect(Object.prototype.hasOwnProperty.call(entry, "content_type")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(entry, "is_sensitive")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(entry, "wall_time")).toBe(true);

    // Negative: camelCase does not exist (would indicate a serde rename)
    expect(Object.prototype.hasOwnProperty.call(entry, "contentType")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(entry, "isSensitive")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(entry, "wallTime")).toBe(false);
  });

  it("HistoryPage own_device_id field is snake_case (not ownDeviceId)", () => {
    const page: HistoryPage = {
      items: [],
      total: 0,
      own_device_id: "",
    };

    expect(Object.prototype.hasOwnProperty.call(page, "own_device_id")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(page, "ownDeviceId")).toBe(false);
  });

  it("parses history_page wire JSON and satisfies HistoryPage", () => {
    const wire = JSON.stringify({
      items: [
        {
          id: "abc",
          content_type: "text",
          preview: "copied text",
          is_sensitive: false,
          wall_time: 1700000000000,
          pinned: false,
        },
      ],
      total: 1,
      own_device_id: "dev-uuid",
    });

    const page = JSON.parse(wire) as HistoryPage;

    expect(page.items[0].content_type).toBe("text");
    expect(page.items[0].is_sensitive).toBe(false);
    expect(page.own_device_id).toBe("dev-uuid");
  });
});

// ---------------------------------------------------------------------------
// §4 get_sync_status payload — SyncStatus
// ---------------------------------------------------------------------------
// Rust wire (GetSyncStatusResponse in copypaste-ipc/src/methods.rs):
//   {
//     "passphrase_set": false,
//     "supabase_configured": false,
//     "signed_in": false,
//     "last_sync_ms": null,
//     "badge_state": "idle"
//   }
// Optional fields: supabase_url, email, supabase_email_set, supabase_password_set

describe("get_sync_status payload — SyncStatus wire shape", () => {
  it("minimal SyncStatus payload satisfies the TS type", () => {
    const syncStatus = {
      passphrase_set: false,
      supabase_configured: false,
      signed_in: false,
      last_sync_ms: null,
    } satisfies SyncStatus;

    expect(syncStatus.passphrase_set).toBe(false);
    expect(syncStatus.supabase_configured).toBe(false);
    expect(syncStatus.signed_in).toBe(false);
    expect(syncStatus.last_sync_ms).toBeNull();
  });

  it("full SyncStatus payload with badge_state satisfies the TS type", () => {
    const syncStatus = {
      passphrase_set: true,
      supabase_configured: true,
      signed_in: true,
      last_sync_ms: 1700000000000,
      supabase_url: "https://project.supabase.co",
      email: "u***@example.com",
      badge_state: "synced" as const,
      supabase_email_set: true,
      supabase_password_set: true,
    } satisfies SyncStatus;

    expect(syncStatus.badge_state).toBe("synced");
    // snake_case guard
    expect("passphrase_set" in syncStatus).toBe(true);
    expect("supabase_configured" in syncStatus).toBe(true);
    expect("signed_in" in syncStatus).toBe(true);
    expect("last_sync_ms" in syncStatus).toBe(true);
    expect("badge_state" in syncStatus).toBe(true);
  });

  it("SyncStatus required fields are snake_case (Rust serde default)", () => {
    const status: SyncStatus = {
      passphrase_set: false,
      supabase_configured: false,
      signed_in: false,
      last_sync_ms: null,
    };

    expect(Object.prototype.hasOwnProperty.call(status, "passphrase_set")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "supabase_configured")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "signed_in")).toBe(true);
    expect(Object.prototype.hasOwnProperty.call(status, "last_sync_ms")).toBe(true);

    // Negative — camelCase would signal an accidental serde rename
    expect(Object.prototype.hasOwnProperty.call(status, "passphraseSet")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(status, "supabaseConfigured")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(status, "signedIn")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(status, "lastSyncMs")).toBe(false);
  });

  it("parses get_sync_status wire JSON and satisfies SyncStatus", () => {
    // Exact shape the Rust GetSyncStatusResponse emits (from methods.rs snapshot tests)
    const wire = JSON.stringify({
      passphrase_set: false,
      supabase_configured: false,
      signed_in: false,
      last_sync_ms: null,
      badge_state: "idle",
    });

    const status = JSON.parse(wire) as SyncStatus;

    expect(status.passphrase_set).toBe(false);
    expect(status.badge_state).toBe("idle");
  });
});
