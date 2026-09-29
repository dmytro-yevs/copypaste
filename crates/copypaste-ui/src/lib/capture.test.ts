import { describe, expect, it } from "vitest";

import {
  capturePresentationOf,
} from "@/features/capture/model";

describe("capture presentation", () => {
  /**
   * Shizuku stops on every reboot. If that painted the screen the same colour
   * as a real fault, the user would learn to read a normal restart as something
   * broken — and then to ignore the colour that means something is.
   */
  it("does not paint a restart as a fault", () => {
    expect(
      capturePresentationOf({ state: "not_granted", reason: "not_running" }),
    ).toEqual({ tone: "attention", role: "status", urgency: "polite" });
    expect(
      capturePresentationOf({
        state: "granted_not_working",
        reason: "not_armed",
      }),
    ).toEqual({ tone: "attention", role: "status", urgency: "polite" });
  });

  it.each([
    [{ state: "disabled" } as const, "off"],
    [{ state: "not_granted", reason: "unsupported" } as const, "info"],
    [{ state: "not_granted", reason: "not_installed" } as const, "info"],
    [
      {
        state: "granted_not_working",
        reason: "awaiting_first_copy",
      } as const,
      "info",
    ],
    [{ state: "working" } as const, "positive"],
  ])("maps %o to polite %s presentation", (health, tone) => {
    expect(capturePresentationOf(health)).toEqual({
      tone,
      role: "status",
      urgency: "polite",
    });
  });

  it("makes a refused read an assertive fault everywhere", () => {
    expect(
      capturePresentationOf({
        state: "granted_not_working",
        reason: "read_refused",
      }),
    ).toEqual({ tone: "danger", role: "alert", urgency: "assertive" });
  });
});
