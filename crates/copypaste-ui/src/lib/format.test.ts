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
    ["#f00", "color"],
    ["function copy() {\n}", "code"],
  ] as const)("decorates text as %s", (content, expected) => {
    expect(kindOf(item({ content_class: "text", content }))).toBe(expected);
  });

  it("keeps sensitive and absent content ahead of the base class", () => {
    expect(kindOf(item({ content_class: "image", is_sensitive: true, content: null }))).toBe("secret");
    expect(kindOf(item({ content_class: "text", content: null }))).toBe("unknown");
  });
});
