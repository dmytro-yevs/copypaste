/**
 * ContentIcon icon-parity tests (CopyPaste-r3t1 / PG-50).
 *
 * Verifies that the canonical icon choice for TEXT, PATH, and NUMBER matches
 * the correct Lucide components. These tests lock in the agreed set so future
 * refactors cannot silently swap icons:
 *
 *   TEXT   → lucide Type        (NOT ContentCopy)
 *   PATH   → lucide FolderOpen  (NOT AttachFile)
 *   NUMBER → lucide Hash        (NOT Tag)
 *
 * Strategy: render the expected Lucide component and the ContentIcon with the
 * matching content type, then compare the SVG structure to confirm they use the
 * same icon.
 */

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { Type, Hash, FolderOpen } from "lucide-react";
import { ContentIcon } from "./ContentIcon";

// ---------------------------------------------------------------------------
// Helper: collect SVG structural fingerprint for icon identity
// ---------------------------------------------------------------------------

function svgFingerprint(container: HTMLElement): {
  childCount: number;
  firstD: string | null;
} {
  const svg = container.querySelector("svg");
  if (!svg) return { childCount: 0, firstD: null };
  const children = Array.from(svg.children);
  const firstEl = children.find((el) =>
    ["path", "line", "polyline", "rect"].includes(el.tagName.toLowerCase())
  );
  return {
    childCount: children.length,
    firstD:
      firstEl?.getAttribute("d") ??
      firstEl?.getAttribute("points") ??
      null,
  };
}

describe("ContentIcon canonical icon parity (CopyPaste-r3t1 PG-50)", () => {
  it("TEXT → lucide Type (not ContentCopy): same SVG structure", () => {
    const { container: lucide } = render(<Type size={14} />);
    const { container: icon } = render(<ContentIcon contentType="text" size={14} />);
    const fp1 = svgFingerprint(lucide);
    const fp2 = svgFingerprint(icon);
    expect(fp2.childCount).toBe(fp1.childCount);
    expect(fp2.firstD).toBe(fp1.firstD);
  });

  it("TEXT (text/plain) → lucide Type: same SVG structure", () => {
    const { container: lucide } = render(<Type size={14} />);
    const { container: icon } = render(
      <ContentIcon contentType="text/plain" size={14} />
    );
    const fp1 = svgFingerprint(lucide);
    const fp2 = svgFingerprint(icon);
    expect(fp2.childCount).toBe(fp1.childCount);
    expect(fp2.firstD).toBe(fp1.firstD);
  });

  it("NUMBER → lucide Hash (not Tag): same SVG structure", () => {
    const { container: lucide } = render(<Hash size={14} />);
    const { container: icon } = render(
      <ContentIcon contentType="number" size={14} />
    );
    const fp1 = svgFingerprint(lucide);
    const fp2 = svgFingerprint(icon);
    expect(fp2.childCount).toBe(fp1.childCount);
    expect(fp2.firstD).toBe(fp1.firstD);
  });

  it("PATH → lucide FolderOpen (not AttachFile): same SVG structure", () => {
    const { container: lucide } = render(<FolderOpen size={14} />);
    const { container: icon } = render(
      <ContentIcon contentType="path" size={14} />
    );
    const fp1 = svgFingerprint(lucide);
    const fp2 = svgFingerprint(icon);
    expect(fp2.childCount).toBe(fp1.childCount);
    expect(fp2.firstD).toBe(fp1.firstD);
  });
});
