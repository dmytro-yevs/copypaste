import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ClipBodyPreview } from "./ClipBodyPreview";
import type { Kind } from "@/lib/format";

const cases: ReadonlyArray<readonly [Kind, string]> = [
  ["text", "plain text"],
  ["text", "first paragraph\nsecond paragraph\nthird paragraph"],
  ["text", "<article>clipboard markup</article>"],
  ["image", ""],
  ["file", "C:\\Users\\Avery\\report.pdf"],
  ["url", "https://example.test/path"],
  ["mail", "person@example.test"],
  ["path", "/tmp/copypaste"],
  ["json", '{"clip":true}'],
  ["code", "function copy() {}"],
  ["color", "#4a90e2"],
  ["num", "123,456"],
  ["unknown", "unsupported payload"],
];

describe("ClipBodyPreview", () => {
  it.each(cases)("renders the shared %s clip presentation", (kind, content) => {
    const { container } = render(
      <ClipBodyPreview
        kind={kind}
        content={content}
        previewLines={3}
        imagePreview={kind === "image" ? <img alt="fixture image" /> : undefined}
      />,
    );

    if (kind === "image") expect(screen.getByRole("img", { name: "fixture image" })).toBeTruthy();
    else expect(container.textContent).toContain(content === "C:\\Users\\Avery\\report.pdf" ? "report.pdf" : content);
  });

});
