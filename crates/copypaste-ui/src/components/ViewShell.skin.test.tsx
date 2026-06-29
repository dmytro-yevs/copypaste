/**
 * Phase 4: ViewShell uses fixed design tokens (--r-card, --sh1).
 * Updated from W-C2: old skin variables replaced.
 */

import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ViewShell } from "./ViewShell";

function allClasses(container: HTMLElement): string {
  return Array.from(container.querySelectorAll("*"))
    .map((el) => el.className)
    .filter((c) => typeof c === "string")
    .join(" ");
}

// ---------------------------------------------------------------------------
// §A  Glass surfaces + entrance animations preserved
// ---------------------------------------------------------------------------
describe("§W-C2-A  ViewShell — glass surfaces (Phase 4)", () => {
  it("header has surface-glass class", () => {
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
// §B  Fixed radius token: --r-card on both surfaces
// ---------------------------------------------------------------------------
describe("§W-C2-B  ViewShell — fixed radius token (--r-card)", () => {
  it("header does NOT use hardcoded rounded-ide-lg class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    expect(header!.className).not.toMatch(/\brounded-ide-lg\b/);
  });

  it("content panel does NOT use hardcoded rounded-ide-lg class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1");
    expect(panel!.className).not.toMatch(/\brounded-ide-lg\b/);
  });

  it("header radius uses var(--r-card)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header") as HTMLElement;
    expect(header.style.borderRadius).toBe("var(--r-card)");
  });

  it("content panel radius uses var(--r-card)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1") as HTMLElement;
    expect(panel.style.borderRadius).toBe("var(--r-card)");
  });
});

// ---------------------------------------------------------------------------
// §C  Fixed shadow token: --sh1 on both surfaces
// ---------------------------------------------------------------------------
describe("§W-C2-C  ViewShell — fixed shadow token (--sh1)", () => {
  it("header does NOT use hardcoded shadow-ide-sm class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header");
    expect(header!.className).not.toMatch(/\bshadow-ide-sm\b/);
  });

  it("content panel does NOT use hardcoded shadow-ide-sm class", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1");
    expect(panel!.className).not.toMatch(/\bshadow-ide-sm\b/);
  });

  it("header shadow uses var(--sh1)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const header = container.querySelector("header") as HTMLElement;
    expect(header.style.boxShadow).toBe("var(--sh1)");
  });

  it("content panel shadow uses var(--sh1)", () => {
    const { container } = render(
      <ViewShell title="Test"><div>content</div></ViewShell>
    );
    const panel = container.querySelector("div.flex-1") as HTMLElement;
    expect(panel.style.boxShadow).toBe("var(--sh1)");
  });
});
