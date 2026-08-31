/** INV-12 / AT-24: only structured codes and retry policy cross the view boundary. */
import { describe, expect, it, vi } from "vitest";

import {
  IpcFailure,
  classifyError,
  friendlyError,
  ipcFailure,
  isRetryable,
  isUnavailable,
  toFriendly,
} from "./errors";

describe("structured IPC failures", () => {
  it.each([
    ["not_ready", true],
    ["protocol_mismatch", false],
    ["key_locked", true],
    ["key_unusable", false],
    ["peer_unreachable", true],
    ["pairing_limit", false],
    ["peer_not_found", false],
    ["auth_failed", false],
    ["content_too_large", false],
  ] as const)("uses the %s code and Rust retry flag", (code, retryable) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const failure = ipcFailure({ code, retryable });
    expect(failure.code).toBe(code);
    expect(failure.kind).toBe(code);
    expect(isRetryable(failure)).toBe(retryable);
    expect(friendlyError(failure.kind).trim().length).toBeGreaterThan(0);
  });

  it("does not reconstruct a code from an English sentence", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    for (const raw of [
      "CopyPaste is still starting up",
      "ENOENT /Users/alice/Library/Application Support/com.copypaste.CopyPaste/daemon.sock",
      "connection refused on /home/bob/.copypaste.sock",
      new Error("no such paired device"),
    ]) {
      expect(classifyError(raw)).toBe("unknown");
    }
  });

  it("uses the shared bounded-content refusal copy", () => {
    expect(friendlyError("content_too_large")).toBe(
      "This content is too large for this operation. Your history is unchanged.",
    );
  });

  it("keeps paths, usernames and an injected message out of the failure", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const failure = ipcFailure({
      code: "internal",
      retryable: false,
      message: "open /Users/alice/private/private.db failed",
    });
    const exposed = `${failure.message} ${JSON.stringify(failure)} ${toFriendly(failure)}`;
    expect(exposed).not.toMatch(/alice|\/Users\/|history\.db|private/);
    expect(exposed).toContain("internal");
  });

  it("preserves a safe future code without guessing its retry policy", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const failure = ipcFailure({ code: "future_state_2", retryable: true });
    expect(failure.code).toBe("future_state_2");
    expect(failure.kind).toBe("unknown");
    expect(failure.retryable).toBe(false);
    expect(toFriendly(failure)).toBe(friendlyError("unknown"));
  });

  it("does not retain a path-like value masquerading as a future code", () => {
    const failure = new IpcFailure(
      "ENOENT_/Users/alice/.copypaste.sock",
      true,
    );
    expect(failure.code).toBe("unknown");
    expect(failure.message).toBe("unknown");
    expect(JSON.stringify(failure)).not.toMatch(/alice|sock|Users/);
  });

  it("uses an explicit app-owned unavailable condition", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(isUnavailable({ code: "unavailable", retryable: false })).toBe(true);
    expect(isUnavailable({ code: "offline", retryable: true })).toBe(false);
  });

  it("turns a malformed rejection into a safe unknown failure", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const failure = ipcFailure({ code: "not_ready", retryable: "yes" });
    expect(failure).toEqual(expect.objectContaining({
      code: "unknown",
      kind: "unknown",
      retryable: false,
    }));
  });
});
