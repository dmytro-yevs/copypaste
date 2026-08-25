import { describe, expect, it } from "vitest";

import { isAndroid, isWindows } from "@/lib/platform";

describe("isAndroid", () => {
  it("uses the native platform contract", () => {
    expect(isAndroid("android")).toBe(true);
    expect(isAndroid("macos")).toBe(false);
    expect(isAndroid("browser")).toBe(false);
  });
});

describe("isWindows", () => {
  it("uses the native platform contract", () => {
    expect(isWindows("windows")).toBe(true);
    expect(isWindows("android")).toBe(false);
    expect(isWindows("browser")).toBe(false);
  });
});
