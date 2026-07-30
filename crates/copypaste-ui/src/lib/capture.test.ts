/**
 * The three derivations the view is allowed to make. Everything else it renders
 * arrives finished from `capture::messages`.
 */
import { describe, expect, it } from "vitest";

import { ladderOf, primaryOf, toneOf } from "@/lib/capture";
import type { CaptureHealth } from "@/lib/ipc";
import { captureSnapshot, probe } from "@/test/harness";

describe("tone", () => {
  /**
   * Shizuku stops on every reboot. If that painted the screen the same colour
   * as a real fault, the user would learn to read a normal restart as something
   * broken — and then to ignore the colour that means something is.
   */
  it("does not paint a restart as a fault", () => {
    expect(toneOf({ state: "not_granted", reason: "not_running" })).toBe("attention");
    expect(toneOf({ state: "granted_not_working", reason: "not_armed" })).toBe("attention");
    expect(toneOf({ state: "granted_not_working", reason: "read_refused" })).toBe("fault");
  });

  it("keeps a choice and an unavailable platform out of the warning tones", () => {
    expect(toneOf({ state: "disabled" })).toBe("off");
    expect(toneOf({ state: "not_granted", reason: "unsupported" })).toBe("info");
    expect(toneOf({ state: "not_granted", reason: "not_installed" })).toBe("info");
    expect(toneOf({ state: "granted_not_working", reason: "awaiting_first_copy" })).toBe("info");
    expect(toneOf({ state: "working" })).toBe("ok");
  });
});

describe("the ladder", () => {
  it("marks each step from what the platform reported, not from the step after it", () => {
    const rungs = ladderOf(
      captureSnapshot({
        shizuku: probe({ running: false, permission: false }),
        health: { state: "not_granted", reason: "not_running" },
        nextStep: "start_shizuku",
        headline: "Background capture isn't set up.",
        detail: "Shizuku isn't running.",
      }),
    );
    expect(rungs.map((r) => [r.id, r.done, r.current])).toEqual([
      ["install", true, false],
      ["start", false, true],
      ["permission", false, false],
      ["armed", false, false],
    ]);
  });

  /** `armed` has no field of its own: `not_armed` is exactly the health the
   *  model reports when the listener is not registered, and nothing else is. */
  it("reads armed from the health rather than inventing a flag", () => {
    const armed = (health: CaptureHealth) =>
      ladderOf(captureSnapshot({ health })).find((r) => r.id === "armed")!.done;

    expect(armed({ state: "working" })).toBe(true);
    expect(armed({ state: "granted_not_working", reason: "awaiting_first_copy" })).toBe(true);
    expect(armed({ state: "granted_not_working", reason: "read_refused" })).toBe(true);
    expect(armed({ state: "granted_not_working", reason: "not_armed" })).toBe(false);
  });

  it("has no step for a rung that is not built", () => {
    expect(ladderOf(captureSnapshot()).map((r) => r.id)).toEqual([
      "install",
      "start",
      "permission",
      "armed",
    ]);
  });
});

describe("the one action", () => {
  /** CopyPaste can neither install Shizuku nor start it. A button carrying
   *  either name would be a button that does nothing. */
  it("offers a re-check for the steps it cannot perform", () => {
    expect(primaryOf("install_shizuku")).toBe("recheck");
    expect(primaryOf("start_shizuku")).toBe("recheck");
  });

  it("offers the one call that covers both asking and arming", () => {
    expect(primaryOf("grant_permission")).toBe("permission");
    expect(primaryOf("arm")).toBe("arm");
  });

  it("offers nothing when the model says there is nothing to do", () => {
    expect(primaryOf("none")).toBe("none");
  });
});
