/**
 * INV-12 / AT-24 — no error shown to a user carries a filesystem path.
 *
 * The daemon socket lives under the home directory, so its path spells out the
 * local username. The bridge scrubs paths on its side; this is the frontend
 * half, and the two are deliberately independent — a leak needs both to fail.
 */
import { describe, expect, it, vi } from "vitest";

import { classifyError, friendlyError, isUnavailable, toFriendly } from "./errors";

const LEAKY = [
  "connection refused on /Users/dmitriy/Library/Application Support/CopyPaste/daemon.sock",
  "No such file or directory (os error 2): /home/dmitriy/.local/share/CopyPaste/daemon.sock",
  new Error("failed to open /Users/someone/secret/path"),
  { message: "/home/other/x" },
  "could not open <path>/copypaste-v2.db",
];

describe("no user-facing error carries a path", () => {
  it.each(LEAKY.map((raw, i) => [i, raw] as const))(
    "case %i is mapped to fixed copy, not echoed",
    (_i, raw) => {
      vi.spyOn(console, "error").mockImplementation(() => {});
      const text = toFriendly(raw);
      expect(text).not.toMatch(/\/Users\/|\/home\/|\.sock|<path>/);
      expect(text).not.toContain("dmitriy");
      expect(text.length).toBeGreaterThan(0);
    },
  );

  it("every kind has non-empty copy that names no path", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    const kinds = new Set(LEAKY.map(classifyError));
    for (const kind of kinds) {
      const message = friendlyError(kind);
      expect(message.trim().length).toBeGreaterThan(0);
      expect(message).not.toMatch(/\/Users\/|\/home\//);
    }
  });
});

describe("classification", () => {
  it.each([
    ["CopyPaste isn't running. Start the daemon with `copypaste-daemon`.", "offline"],
    ["CopyPaste is still starting up. Try again in a moment.", "not_ready"],
    ["The app and the daemon are different versions.", "protocol_mismatch"],
    ["That item is no longer there.", "not_found"],
    ["Command start_service not found", "unavailable"],
    ["Pairing is not available in this build.", "unavailable"],
  ] as const)("%s -> %s", (raw, kind) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(classifyError(raw)).toBe(kind);
  });

  it("distinguishes 'this build cannot' from 'the service is down'", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    // The two need different screens: one is structural and retrying will
    // never help; the other is a service the user can start.
    expect(isUnavailable("Command peers not found")).toBe(true);
    expect(isUnavailable("CopyPaste isn't running.")).toBe(false);
  });

  it("keeps the raw text out of the thrown value", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    const kind = classifyError("boom at /Users/dmitriy/x.sock");
    // The raw value goes to the console — a developer surface — and nowhere
    // else. What the view layer receives is a token.
    expect(spy).toHaveBeenCalled();
    expect(kind).toBe("offline");
    expect(friendlyError(kind)).not.toContain("dmitriy");
  });
});
