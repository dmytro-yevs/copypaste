import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import { resolveClipBodyPresentation } from "./clipPresentation";

describe("resolveClipBodyPresentation", () => {
  it.each([
    ["complete", item({ content: "complete" }), null, false, { state: "content", content: "complete", source: "full" }],
    ["pending preview", item({ content: "short preview", truncated: true }), null, false, { state: "content", content: "short preview", source: "preview" }],
    ["resolved full body", item({ content: "short preview", truncated: true }), "complete body", false, { state: "content", content: "complete body", source: "full" }],
    ["failed truncated body", item({ content: "must not render", truncated: true }), null, true, { state: "unavailable" }],
  ] as const)("uses %s", (_name, target, fullContent, fullContentFailed, expected) => {
    expect(resolveClipBodyPresentation({ item: target, fullContent, fullContentFailed })).toEqual(expected);
  });

  it("keeps arbitrary clipboard text visible without classification", () => {
    const body = "api_key=abc123; password=not-a-detector-concern";
    expect(resolveClipBodyPresentation({
      item: item({ content: body }),
      fullContent: null,
      fullContentFailed: false,
    })).toEqual({ state: "content", content: body, source: "full" });
  });
});
