import { describe, expect, it } from "vitest";

import { devices } from "./devices";
import { pairing } from "./devicesPairing";

function leafPaths(value: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(value)
    .flatMap(([key, child]) => {
      const path = prefix === "" ? key : `${prefix}.${key}`;
      return typeof child === "object" && child !== null
        ? leafPaths(child as Record<string, unknown>, path)
        : [path];
    })
    .sort();
}

describe("devices pairing catalogue", () => {
  it("keeps devices.pairing as the extracted catalogue with its full shape", () => {
    expect(devices.pairing).toBe(pairing);
    expect(leafPaths(devices.pairing)).toEqual(leafPaths(pairing));
  });

  it("retains representative established and semantic pairing copy", () => {
    expect(devices.pairing.confirmLabel).toBe(
      "Codes match — confirm pairing in the native view",
    );
    expect(devices.pairing.state.timedOut.body).toBe(
      "Keep both devices on the same network, show a fresh code, and try again.",
    );
    expect(devices.pairing.semantic.code_mismatch.body).toBe(
      "The security codes did not match. No device was paired.",
    );
  });
});
