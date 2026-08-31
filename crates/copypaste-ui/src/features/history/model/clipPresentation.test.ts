import { describe, expect, it } from "vitest";

import { clipTypeMetadata, fileDisplayName } from "@/lib/clipPresentation";
import type { Kind } from "@/lib/format";
import type { Item } from "@/lib/ipc";
import { item } from "@/test/harness";
import { clipCopyAction, historyKindFilterLabel, rowLabel } from "./clipPresentation";

const translate = (key: string) => key;

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

  it.each([
    { contentClass: "image", kind: "image" },
    { contentClass: "file", kind: "file" },
    { contentClass: "other", kind: "unknown" },
    { contentClass: "future", kind: "unknown" },
  ] as const satisfies ReadonlyArray<{ contentClass: string; kind: Kind }>)(
    "uses the $kind type label for a $contentClass item without text",
    ({ contentClass, kind }) => {
      expect(
        rowLabel(
          item({
            content: null,
            content_class: contentClass as Item["content_class"],
          }),
          null,
          undefined,
          translate,
        ),
      ).toBe(clipTypeMetadata(kind).label);
    },
  );

  it("keeps sensitive and finding content out of row labels", () => {
    const raw = "secret clipboard value";
    const redacted = "secret •••• value";
    const finding = {
      label: "secret",
      spans: [],
      spans_truncated: false,
      redacted_preview: redacted,
    };

    const sensitiveLabel = rowLabel(
      item({ content: raw, is_sensitive: true, sensitive_finding: finding }),
      null,
      undefined,
      translate,
    );
    expect(sensitiveLabel).toBe("history.row.sensitiveName");
    expect(sensitiveLabel).not.toContain(raw);
    expect(sensitiveLabel).not.toContain(redacted);

    const findingLabel = rowLabel(
      item({ content: raw, sensitive_finding: finding }),
      null,
      undefined,
      translate,
    );
    expect(findingLabel).toBe(
      `history.row.potentialSensitiveWarning. ${redacted}`,
    );
    expect(findingLabel).not.toContain(raw);
  });

  it("keeps an ordinary text item without content empty", () => {
    expect(
      rowLabel(
        item({ content: null, content_class: "text" }),
        null,
        undefined,
        translate,
      ),
    ).toBe("history.row.empty");
  });
});
