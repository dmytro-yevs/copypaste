/**
 * W-C2: ViewShell skin token audit (CopyPaste-un1w)
 *
 * Verifies that ViewShell's header and content panel use --skin-* CSS variable
 * references for radius and shadow instead of hardcoded tailwind tokens
 * (rounded-ide-lg / shadow-ide-sm), so the component responds to skin switching
 * (classic / quiet / vapor) without code changes.
 *
 * Rules from §4 of the skin implementation plan:
 *  - Classic UNCHANGED: surface-glass, card-in, reveal-up are preserved.
 *  - No hardcoded rounded-ide-lg or shadow-ide-sm on ViewShell surfaces.
 *  - Radius driven by --skin-r-card (inline style or Tailwind arbitrary value).
 *  - Shadow driven by --skin-shadow-card (header) / --skin-shadow-float (content).
 *  - All original features (drag region, title, actions slot) preserved.
 */

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ViewShell } from "./ViewShell";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Collect all className strings from the rendered subtree. */
function allClasses(container: HTMLElement): string {
  return Array.from(container.querySelectorAll("*"))
    .map((el) => el.className)
    .filter((c) => typeof c === "string")
    .join(" ");
}

// ---------------------------------------------------------------------------
// §A  Classic look preserved — surface-glass + entrance animations
// ---------------------------------------------------------------------------
describe("§W-C2-A  ViewShell — skin-driven surfaces (classic preserved)", () => {
  it("header has surface-glass class (material driven by skin tokens)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    expect(header).not.toBeNull();
    expect(header!.className).toMatch(/surface-glass/);
  });

  it("content panel has surface-glass class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    // The flex-1 sibling of the header
    const panel = container.querySelector("div.flex-1");
    expect(panel).not.toBeNull();
    expect(panel!.className).toMatch(/surface-glass/);
  });

  it("header has card-in entrance animation class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    expect(header!.className).toMatch(/card-in/);
  });

  it("content panel has reveal-up entrance animation class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1");
    expect(panel!.className).toMatch(/reveal-up/);
  });

  it("renders the title in an h1", () => {
    render(<ViewShell title="My View"><div>child</div></ViewShell>);
    expect(screen.getByRole("heading", { name: /My View/i })).toBeInTheDocument();
  });

  it("renders children in the content panel", () => {
    render(<ViewShell title="T"><span data-testid="child-node">hi</span></ViewShell>);
    expect(screen.getByTestId("child-node")).toBeInTheDocument();
  });

  it("renders the actions slot when provided", () => {
    render(
      <ViewShell title="T" actions={<button data-testid="action-btn">Go</button>}>
        <div>c</div>
      </ViewShell>
    );
    expect(screen.getByTestId("action-btn")).toBeInTheDocument();
  });

  it("header has data-tauri-drag-region", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    expect(header!.hasAttribute("data-tauri-drag-region")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §B  Skin-token radius: --skin-r-card on both surfaces
// ---------------------------------------------------------------------------
describe("§W-C2-B  ViewShell — skin-driven radius (--skin-r-card)", () => {
  it("header does NOT use hardcoded rounded-ide-lg class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    // rounded-ide-lg is hardcoded 14px; skin should drive radius via --skin-r-card
    expect(header!.className).not.toMatch(/\brounded-ide-lg\b/);
  });

  it("content panel does NOT use hardcoded rounded-ide-lg class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1");
    expect(panel!.className).not.toMatch(/\brounded-ide-lg\b/);
  });

  it("header radius references --skin-r-card (inline style or arbitrary value)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header") as HTMLElement;
    // Accept: inline style borderRadius using var(--skin-r-card),
    // OR Tailwind arbitrary class containing skin-r-card.
    const inlineRadius = header.style.borderRadius;
    const classStr = header.className;
    const hasSkinRadius =
      (inlineRadius && inlineRadius.includes("--skin-r-card")) ||
      classStr.includes("skin-r-card");
    expect(hasSkinRadius).toBe(true);
  });

  it("content panel radius references --skin-r-card", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1") as HTMLElement;
    const inlineRadius = panel.style.borderRadius;
    const classStr = panel.className;
    const hasSkinRadius =
      (inlineRadius && inlineRadius.includes("--skin-r-card")) ||
      classStr.includes("skin-r-card");
    expect(hasSkinRadius).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §C  Skin-token shadow: --skin-shadow-card / --skin-shadow-float
// ---------------------------------------------------------------------------
describe("§W-C2-C  ViewShell — skin-driven shadow", () => {
  it("header does NOT use hardcoded shadow-ide-sm class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    // shadow-ide-sm = hardcoded var(--ide-e2); skin should control via --skin-shadow-card
    expect(header!.className).not.toMatch(/\bshadow-ide-sm\b/);
  });

  it("content panel does NOT use hardcoded shadow-ide-sm class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1");
    expect(panel!.className).not.toMatch(/\bshadow-ide-sm\b/);
  });

  it("header shadow references --skin-shadow-card (inline style or arbitrary value)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header") as HTMLElement;
    const inlineShadow = header.style.boxShadow;
    const classStr = header.className;
    const hasSkinShadow =
      (inlineShadow && inlineShadow.includes("--skin-shadow-card")) ||
      classStr.includes("skin-shadow-card");
    expect(hasSkinShadow).toBe(true);
  });

  it("content panel shadow references --skin-shadow-float (inline style or arbitrary value)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1") as HTMLElement;
    const inlineShadow = panel.style.boxShadow;
    const classStr = panel.className;
    const hasSkinShadow =
      (inlineShadow && inlineShadow.includes("--skin-shadow-float")) ||
      classStr.includes("skin-shadow-float");
    expect(hasSkinShadow).toBe(true);
  });
});
