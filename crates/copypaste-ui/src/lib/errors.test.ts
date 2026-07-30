/**
 * INV-12 / AT-24 — no error shown to a user carries a filesystem path.
 *
 * The daemon socket lives under the home directory, so its path spells out the
 * local username. The bridge scrubs paths on its side; this is the frontend
 * half, and the two are deliberately independent — a leak needs both to fail.
 */
import { describe, expect, it, vi } from "vitest";

import {
  classifyError,
  friendlyError,
  isRetryable,
  isUnavailable,
  toFriendly,
} from "./errors";

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

/**
 * The two conditions that are unrecoverable, and the one beside them that is
 * not. Collapsing any pair of these is what puts a **Try again** in front of a
 * user whose history is gone.
 */
describe("the states no retry can clear", () => {
  it.each([
    // The daemon's own sentences, which reach the frontend as text.
    [
      "this is a CopyPaste 0.4 history; this version cannot read it and has left it as it was",
      "legacy_database",
    ],
    [
      "this device's key is present and cannot be used, so the history encrypted with it cannot be read by anything",
      "key_unusable",
    ],
    [
      "the key store could not be read, so this history could not be unlocked",
      "key_locked",
    ],
    // The in-process backend wraps the core error instead, so both spellings
    // have to classify the same way.
    ["could not open history: this is a CopyPaste 0.4 history", "legacy_database"],
    [
      "could not open the keystore: stored device secret unusable: the stored device secret is the wrong length",
      "key_unusable",
    ],
    ["could not open the keystore: key store unavailable: the keychain is locked", "key_locked"],
  ] as const)("%s -> %s", (raw, kind) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(classifyError(raw)).toBe(kind);
  });

  it("marks the unrecoverable ones as not retryable, and the locked one as retryable", () => {
    expect(isRetryable("legacy_database")).toBe(false);
    expect(isRetryable("key_unusable")).toBe(false);
    expect(isRetryable("key_locked")).toBe(true);
  });

  it("gives each of them a sentence of its own", () => {
    const sentences = (["legacy_database", "key_locked", "key_unusable"] as const).map(
      friendlyError,
    );
    expect(new Set(sentences).size).toBe(3);
    for (const sentence of sentences) {
      expect(sentence.length).toBeGreaterThan(0);
      expect(sentence).not.toMatch(/\/Users\/|\/home\/|\.sock|\.db\b/);
    }
  });
});

/**
 * `copypaste_p2p::NodeError`, whose sentences the daemon passes through. All
 * eight read as "The background service returned an error" before this, which
 * made a mistyped code, a switched-off device and a full pairing list one
 * event (post-merge review, finding 4).
 */
describe("pairing and sync failures", () => {
  it.each([
    ["that pairing code is not valid", "pairing_code"],
    ["the other device did not accept this pairing code", "pairing_code"],
    ["that address could not be resolved; expected host:port", "pairing_address"],
    [
      "this peer has never been reached and is not visible on the network; sync from the other device, or re-pair with an address",
      "peer_unreachable",
    ],
    ["the other device stopped responding", "peer_unreachable"],
    [
      "this device is already paired with as many devices as it can hold; unpair one first",
      "pairing_limit",
    ],
    ["the sync session with the other device failed", "peer_failed"],
    ["the paired-device list could not be updated", "peer_failed"],
  ] as const)("%s -> %s", (raw, kind) => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(classifyError(raw)).toBe(kind);
  });

  it("gives each one a sentence of its own, and none of them the generic one", () => {
    const kinds = [
      "pairing_code",
      "pairing_address",
      "peer_unreachable",
      "pairing_limit",
      "peer_failed",
    ] as const;
    const sentences = kinds.map(friendlyError);
    expect(new Set(sentences).size).toBe(kinds.length);
    for (const sentence of sentences) {
      expect(sentence).not.toBe(friendlyError("unknown"));
      expect(sentence).not.toMatch(/\/Users\/|\/home\//);
    }
  });

  /** The refusal whose whole point is naming the remedy — it landed with a
   *  commit message saying so, and the remedy was being discarded. */
  it("keeps the remedy in the pairing-cap refusal", () => {
    expect(friendlyError("pairing_limit")).toMatch(/unpair/i);
    expect(isRetryable("pairing_limit")).toBe(false);
  });

  it("retries only what a repeat could answer differently", () => {
    expect(isRetryable("peer_unreachable")).toBe(true);
    expect(isRetryable("peer_failed")).toBe(true);
    expect(isRetryable("pairing_code")).toBe(false);
    expect(isRetryable("pairing_address")).toBe(false);
  });
});
