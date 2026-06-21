/**
 * IconActionButton — unit tests (CopyPaste-bdac.26).
 *
 * Verifies: aria-label, title, click handler, danger variant, hit-target overlay.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { IconActionButton } from "./IconActionButton";

describe("IconActionButton", () => {
  it("renders a button with the given aria-label", () => {
    render(
      <IconActionButton aria-label="Delete" title="Delete" onClick={vi.fn()}>
        <svg />
      </IconActionButton>
    );
    expect(screen.getByRole("button", { name: "Delete" })).toBeInTheDocument();
  });

  it("renders a button with the given title", () => {
    render(
      <IconActionButton aria-label="Preview" title="Preview item" onClick={vi.fn()}>
        <svg />
      </IconActionButton>
    );
    expect(screen.getByTitle("Preview item")).toBeInTheDocument();
  });

  it("calls onClick when clicked", () => {
    const handler = vi.fn();
    render(
      <IconActionButton aria-label="Pin" title="Pin" onClick={handler}>
        <svg />
      </IconActionButton>
    );
    fireEvent.click(screen.getByRole("button", { name: "Pin" }));
    expect(handler).toHaveBeenCalledOnce();
  });

  it("does not propagate the click event (stopPropagation)", () => {
    const parentHandler = vi.fn();
    render(
      <div onClick={parentHandler}>
        <IconActionButton aria-label="Pin" title="Pin" onClick={vi.fn()}>
          <svg />
        </IconActionButton>
      </div>
    );
    fireEvent.click(screen.getByRole("button", { name: "Pin" }));
    // stopPropagation — parent div should NOT receive the click.
    expect(parentHandler).not.toHaveBeenCalled();
  });

  it("applies danger styling when danger=true", () => {
    render(
      <IconActionButton aria-label="Delete" title="Delete" danger onClick={vi.fn()}>
        <svg />
      </IconActionButton>
    );
    const btn = screen.getByRole("button", { name: "Delete" });
    expect(btn.className).toContain("text-ide-danger");
    expect(btn.className).not.toContain("text-ide-dim");
  });

  it("applies dim styling by default (no danger prop)", () => {
    render(
      <IconActionButton aria-label="Preview" title="Preview" onClick={vi.fn()}>
        <svg />
      </IconActionButton>
    );
    const btn = screen.getByRole("button", { name: "Preview" });
    expect(btn.className).toContain("text-ide-dim");
    expect(btn.className).not.toContain("text-ide-danger");
  });

  it("renders the invisible hit-target overlay span", () => {
    const { container } = render(
      <IconActionButton aria-label="Preview" title="Preview" onClick={vi.fn()}>
        <svg />
      </IconActionButton>
    );
    // The overlay is aria-hidden and uses inset: "-12px"
    const overlay = container.querySelector('span[aria-hidden="true"]');
    expect(overlay).not.toBeNull();
    expect((overlay as HTMLElement).style.inset).toBe("-12px");
  });

  it("renders children inside the button", () => {
    render(
      <IconActionButton aria-label="Custom" title="Custom" onClick={vi.fn()}>
        <span data-testid="icon-child" />
      </IconActionButton>
    );
    expect(screen.getByTestId("icon-child")).toBeInTheDocument();
  });
});
