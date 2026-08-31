import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import { resolveClipBodyPresentation } from "./clipPresentation";

describe("resolveClipBodyPresentation", () => {
  it.each([
    ["complete", item({ content: "complete" }), null, false, null, { state: "content", content: "complete", source: "full" }],
    ["pending preview", item({ content: "short preview", truncated: true }), null, false, null, { state: "content", content: "short preview", source: "preview" }],
    ["resolved full body", item({ content: "short preview", truncated: true }), "complete body", false, null, { state: "content", content: "complete body", source: "full" }],
    ["failed truncated body", item({ content: "must not render", truncated: true }), null, true, null, { state: "unavailable" }],
    ["masked sensitive body", item({ is_sensitive: true, truncated: true }), null, true, null, { state: "masked" }],
    ["ephemeral sensitive reveal", item({ is_sensitive: true, truncated: true }), null, true, "revealed once", { state: "content", content: "revealed once", source: "reveal" }],
  ] as const)("uses %s", (_name, target, fullContent, fullContentFailed, revealedContent, expected) => {
    expect(resolveClipBodyPresentation({ item: target, fullContent, fullContentFailed, revealedContent })).toEqual(expected);
  });

  const potential = item({
    content: "token content",
    sensitive_finding: { label: "possible token", spans: [{ start: 0, end: 5 }], spans_truncated: false, redacted_preview: "••••• content" },
  });

  it("keeps a potential-sensitive preview redacted until disclosure", () => {
    expect(resolveClipBodyPresentation({ item: potential, fullContent: null, fullContentFailed: false, revealedContent: null })).toEqual({ state: "content", content: "••••• content", source: "redacted" });
    expect(resolveClipBodyPresentation({ item: potential, fullContent: null, fullContentFailed: false, revealedContent: null, showPotentialSensitiveOriginal: true })).toEqual({ state: "content", content: "token content", source: "full" });
  });

  it("does not use a redacted preview after a truncated body read fails", () => {
    expect(resolveClipBodyPresentation({ item: { ...potential, truncated: true }, fullContent: null, fullContentFailed: true, revealedContent: null })).toEqual({ state: "unavailable" });
  });
});
