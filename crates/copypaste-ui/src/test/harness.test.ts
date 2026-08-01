import { describe, expect, it } from "vitest";

import { status } from "@/test/harness";

describe("status fixture", () => {
  it("always supplies private mode while preserving boolean overrides", () => {
    expect(status().private_mode).toBe(false);
    expect(status({ private_mode: true }).private_mode).toBe(true);
    expect(status({ private_mode: false }).private_mode).toBe(false);
    expect(status({ private_mode: undefined }).private_mode).toBe(false);
  });
});
