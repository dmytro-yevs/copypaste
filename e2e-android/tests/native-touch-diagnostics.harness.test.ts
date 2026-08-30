import { describe, expect, test } from "vitest";

import { parseInputSourceMasks } from "../src/harness/native-touch-diagnostics.js";

describe("Android native touch diagnostics", () => {
  test("converts AOSP InputReader source lines into bounded numeric masks", () => {
    const masks = parseInputSourceMasks(
      [
        "    Sources: TOUCHSCREEN | STYLUS",
        "    Sources: 0x00002002",
      ].join("\n"),
    );
    expect(masks).toEqual([0x00005002, 0x00002002]);
    expect(JSON.stringify(masks)).not.toContain("TOUCHSCREEN");
  });

  test.each([
    ["missing", undefined],
    ["no source line", "Input Reader State"],
    ["unknown source", "Sources: PRIVATE_POINTER"],
    ["malformed source", "Sources: TOUCHSCREEN|STYLUS"],
    ["prototype key", "Sources: __proto__"],
    ["constructor key", "Sources: constructor"],
  ])("keeps %s input source data unknown", (_name, dump) => {
    expect(parseInputSourceMasks(dump)).toBe("unknown");
  });
});
