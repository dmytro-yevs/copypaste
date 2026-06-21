import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Panel } from "./Panel";

// ---------------------------------------------------------------------------
// Panel — extracted shared component tests (bdac.3)
// ---------------------------------------------------------------------------

describe("Panel", () => {
  it("renders children inside the card wrapper", () => {
    render(<Panel><span>hello panel</span></Panel>);
    expect(screen.getByText("hello panel")).toBeInTheDocument();
  });

  it("applies surface-card class to the outer wrapper", () => {
    const { container } = render(<Panel><span>content</span></Panel>);
    // The outermost div must carry surface-card for skin-aware material.
    const outer = container.firstElementChild as HTMLElement;
    expect(outer.classList.contains("surface-card")).toBe(true);
  });

  it("applies overflow-hidden on the inner wrapper to clip row borders", () => {
    const { container } = render(<Panel><span>content</span></Panel>);
    const inner = container.firstElementChild?.firstElementChild as HTMLElement;
    expect(inner.classList.contains("overflow-hidden")).toBe(true);
  });

  it("passes --skin-r-card to both wrappers via inline style", () => {
    const { container } = render(<Panel><span>content</span></Panel>);
    const outer = container.firstElementChild as HTMLElement;
    const inner = outer.firstElementChild as HTMLElement;
    expect(outer.style.borderRadius).toBe("var(--skin-r-card)");
    expect(inner.style.borderRadius).toBe("var(--skin-r-card)");
  });

  it("renders multiple children", () => {
    render(
      <Panel>
        <div>row one</div>
        <div>row two</div>
      </Panel>,
    );
    expect(screen.getByText("row one")).toBeInTheDocument();
    expect(screen.getByText("row two")).toBeInTheDocument();
  });
});
