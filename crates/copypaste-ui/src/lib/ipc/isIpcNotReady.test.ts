/**
 * isIpcNotReady — truth-table tests.
 *
 * Verifies that the shared helper correctly identifies the two error code
 * variants ("ipc_not_ready" and the legacy uppercase "IPC_NOT_READY") and
 * rejects every other input shape.
 */
import { describe, expect, it } from "vitest";
import { isIpcNotReady } from "./helpers";
import { IpcError } from "./transport";

describe("isIpcNotReady", () => {
  it("returns true for IpcError with code 'ipc_not_ready' (lowercase)", () => {
    expect(isIpcNotReady(new IpcError("daemon not ready", "ipc_not_ready"))).toBe(true);
  });

  it("returns true for IpcError with code 'IPC_NOT_READY' (legacy uppercase)", () => {
    expect(isIpcNotReady(new IpcError("daemon not ready", "IPC_NOT_READY"))).toBe(true);
  });

  it("returns false for IpcError with code 'daemon_offline'", () => {
    expect(isIpcNotReady(new IpcError("offline", "daemon_offline"))).toBe(false);
  });

  it("returns false for IpcError with code 'not_found'", () => {
    expect(isIpcNotReady(new IpcError("item not found", "not_found"))).toBe(false);
  });

  it("returns false for IpcError with null code", () => {
    expect(isIpcNotReady(new IpcError("some error", null))).toBe(false);
  });

  it("returns false for a plain Error (not an IpcError)", () => {
    expect(isIpcNotReady(new Error("random error"))).toBe(false);
  });

  it("returns false for null", () => {
    expect(isIpcNotReady(null)).toBe(false);
  });

  it("returns false for undefined", () => {
    expect(isIpcNotReady(undefined)).toBe(false);
  });

  it("returns false for a plain string", () => {
    expect(isIpcNotReady("ipc_not_ready")).toBe(false);
  });

  it("returns false for a plain object that looks like an error but isn't IpcError", () => {
    expect(isIpcNotReady({ code: "ipc_not_ready", message: "fake" })).toBe(false);
  });
});
