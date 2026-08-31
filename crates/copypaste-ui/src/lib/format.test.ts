import { describe, expect, it } from "vitest";

import { kindOf } from "@/lib/format";
import { item } from "@/test/harness";

describe("kindOf", () => {
  it.each([
    ["text", "plain text", "text"],
    ["image", "https://example.test", "image"],
    ["file", "#f00", "file"],
    ["other", "https://example.test", "unknown"],
  ] as const)("uses the authoritative %s base class", (content_class, content, expected) => {
    expect(kindOf(item({ content_class, content }))).toBe(expected);
  });

  it.each([
    ["https://example.test", "url"],
    ["person@example.test", "mail"],
    ["/tmp/copypaste", "path"],
    ["#f00", "color"],
    ["123", "num"],
    ['{"item": true}', "json"],
    ["function copy() {\n}", "code"],
  ] as const)("decorates text as %s", (content, expected) => {
    expect(kindOf(item({ content_class: "text", content }))).toBe(expected);
  });

  it.each([
    ["text", "unknown"],
    ["image", "image"],
    ["file", "file"],
    ["other", "unknown"],
  ] as const)("preserves %s precedence when content is absent", (content_class, expected) => {
    expect(kindOf(item({ content_class, content: null }))).toBe(expected);
  });

  it("keeps sensitive content ahead of its authoritative base class", () => {
    expect(kindOf(item({ content_class: "image", is_sensitive: true, content: null }))).toBe("secret");
  });

  it("does not let a contradictory image MIME type override Other", () => {
    expect(
      kindOf(item({ content_class: "other", content_type: "image/png", content: "not an image" })),
    ).toBe("unknown");
  });

  it.each([
    "https://example.test",
    "#f00",
    "function copy() {\n}",
  ])("fails closed for a future runtime content class before decorating %s", (content) => {
    const futureContentClass = "archive" as unknown as ReturnType<typeof item>["content_class"];

    expect(kindOf(item({ content_class: futureContentClass, content }))).toBe("unknown");
  });
});
