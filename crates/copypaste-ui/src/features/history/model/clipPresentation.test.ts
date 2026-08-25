import { describe, expect, it } from "vitest";

import { item } from "@/test/harness";
import {
  clipCopyAction,
  clipTypeMetadata,
  fileDisplayName,
  historyKindFilterLabel,
  resolveClipBodyPresentation,
} from "./clipPresentation";

describe("resolveClipBodyPresentation", () => {
  it.each([
    {
      name: "uses a complete item body directly",
      target: item({ content: "complete", truncated: false }),
      fullContent: null,
      fullContentFailed: false,
      revealedContent: null,
      expected: { state: "content", content: "complete", source: "full" },
    },
    {
      name: "identifies a truncated body while the full read is pending",
      target: item({ content: "short preview", truncated: true }),
      fullContent: null,
      fullContentFailed: false,
      revealedContent: null,
      expected: {
        state: "content",
        content: "short preview",
        source: "preview",
      },
    },
    {
      name: "uses the resolved full body",
      target: item({ content: "short preview", truncated: true }),
      fullContent: "complete body",
      fullContentFailed: false,
      revealedContent: null,
      expected: {
        state: "content",
        content: "complete body",
        source: "full",
      },
    },
    {
      name: "fails closed when a truncated body's read fails",
      target: item({ content: "must not render", truncated: true }),
      fullContent: null,
      fullContentFailed: true,
      revealedContent: null,
      expected: { state: "unavailable" },
    },
    {
      name: "masks a sensitive item without retaining plaintext",
      target: item({ is_sensitive: true, truncated: true }),
      fullContent: null,
      fullContentFailed: true,
      revealedContent: null,
      expected: { state: "masked" },
    },
    {
      name: "uses only the ephemeral reveal input for a sensitive item",
      target: item({ is_sensitive: true, truncated: true }),
      fullContent: null,
      fullContentFailed: true,
      revealedContent: "revealed once",
      expected: {
        state: "content",
        content: "revealed once",
        source: "reveal",
      },
    },
  ])("$name", ({ target, expected, ...body }) => {
    expect(resolveClipBodyPresentation({ item: target, ...body })).toEqual(
      expected,
    );
  });

  const potential = item({
    content: "token content",
    sensitive_finding: {
      label: "possible token",
      spans: [{ start: 0, end: 5 }],
      spans_truncated: false,
      redacted_preview: "••••• content",
    },
  });

  it.each([
    [false, "••••• content", "redacted"],
    [true, "token content", "full"],
  ] as const)(
    "resolves a potential-sensitive item with disclosure=%s",
    (showPotentialSensitiveOriginal, content, source) => {
      expect(
        resolveClipBodyPresentation({
          item: potential,
          fullContent: null,
          fullContentFailed: false,
          revealedContent: null,
          showPotentialSensitiveOriginal,
        }),
      ).toEqual({ state: "content", content, source });
    },
  );

  it("does not use a redacted preview after a truncated body read fails", () => {
    expect(
      resolveClipBodyPresentation({
        item: { ...potential, truncated: true },
        fullContent: null,
        fullContentFailed: true,
        revealedContent: null,
      }),
    ).toEqual({ state: "unavailable" });
  });
});

describe("History clip presentation", () => {
  it("keeps singular item labels distinct from filter labels", () => {
    expect(clipTypeMetadata("image").label).toBe("Image");
    expect(historyKindFilterLabel("image")).toBe("Images");
    expect(clipTypeMetadata("file").label).toBe("File");
    expect(historyKindFilterLabel("file")).toBe("Files");
  });

  it("uses one copy action for every History surface", () => {
    expect(clipCopyAction("image")).toEqual({
      icon: "image",
      label: "Copy image",
    });
    expect(clipCopyAction("text")).toEqual({
      icon: "copy",
      label: "Copy",
    });
  });

  it.each([
    ["/Users/alice/Documents/report.pdf", "report.pdf"],
    [String.raw`C:\Users\alice\Documents\report.pdf`, "report.pdf"],
    ["folder/report.pdf/", "report.pdf"],
    ["", ""],
  ])("formats the display name for %j", (value, expected) => {
    expect(fileDisplayName(value)).toBe(expected);
  });
});
