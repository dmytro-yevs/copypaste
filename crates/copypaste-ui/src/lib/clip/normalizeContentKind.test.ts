import { describe, expect, it } from "vitest";
import { normalizeContentKind } from "./normalizeContentKind";

describe("normalizeContentKind", () => {
  it("maps each of the 11 named kinds to its normalized kind", () => {
    const cases: Array<[string, string]> = [
      ["TEXT", "text"],
      ["URL", "url"],
      ["EMAIL", "mail"],
      ["PHONE", "num"],
      ["NUMBER", "num"],
      ["COLOR", "color"],
      ["JSON", "json"],
      ["CODE", "code"],
      ["PATH", "file"],
      ["FILE", "file"],
      ["IMAGE", "image"],
    ];
    for (const [kind, expected] of cases) {
      expect(normalizeContentKind({ kind, content_type: "text/plain" })).toBe(expected);
    }
  });

  it("is case-insensitive and trims the kind", () => {
    expect(normalizeContentKind({ kind: "url" })).toBe("url");
    expect(normalizeContentKind({ kind: "  Email " })).toBe("mail");
  });

  it("collapses the documented alias pairs", () => {
    expect(normalizeContentKind({ kind: "PATH" })).toBe("file");
    expect(normalizeContentKind({ kind: "FILE" })).toBe("file");
    expect(normalizeContentKind({ kind: "PHONE" })).toBe("num");
    expect(normalizeContentKind({ kind: "NUMBER" })).toBe("num");
  });

  it("returns unknown for an unrecognized string", () => {
    expect(normalizeContentKind({ kind: "AUDIO", content_type: "audio/mp3" })).toBe("unknown");
    expect(normalizeContentKind({ kind: "totally-made-up" })).toBe("unknown");
  });

  it("returns unknown for undefined kind and undefined content_type", () => {
    expect(normalizeContentKind({})).toBe("unknown");
    expect(normalizeContentKind({ kind: undefined, content_type: undefined })).toBe("unknown");
  });

  it("image MIME with absent kind normalizes to image", () => {
    expect(normalizeContentKind({ content_type: "image/png" })).toBe("image");
    expect(normalizeContentKind({ content_type: "IMAGE/JPEG" })).toBe("image");
  });

  it("image MIME wins over a contradictory kind", () => {
    // kind says URL but content_type is an image MIME → image (MIME wins).
    expect(normalizeContentKind({ kind: "URL", content_type: "image/png" })).toBe("image");
    // kind says IMAGE and content_type is an image MIME → still image.
    expect(normalizeContentKind({ kind: "IMAGE", content_type: "image/gif" })).toBe("image");
  });

  it("derives from content_type when kind is absent", () => {
    expect(normalizeContentKind({ content_type: "file" })).toBe("file");
    expect(normalizeContentKind({ content_type: "text/plain" })).toBe("text");
    expect(normalizeContentKind({ content_type: "text" })).toBe("text");
    expect(normalizeContentKind({ content_type: "application/octet-stream" })).toBe("unknown");
  });
});
