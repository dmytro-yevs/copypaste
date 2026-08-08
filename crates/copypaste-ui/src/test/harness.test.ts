import { describe, expect, it } from "vitest";

import { status } from "@/test/harness";

describe("status fixture", () => {
  it("defaults private mode and epoch while preserving overrides", () => {
    expect(status().private_mode).toBe(false);
    expect(status().private_mode_epoch).toBe(0);
    expect(status({ private_mode: true }).private_mode).toBe(true);
    expect(status({ private_mode: false }).private_mode).toBe(false);
    expect(status({ private_mode: undefined }).private_mode).toBe(false);
    expect(status({ private_mode_epoch: 9 }).private_mode_epoch).toBe(9);
    expect(status({ private_mode_epoch: undefined }).private_mode_epoch).toBe(0);
  });
});
