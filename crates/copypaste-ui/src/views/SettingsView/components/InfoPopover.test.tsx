import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InfoPopover } from "./InfoPopover";

// ---------------------------------------------------------------------------
// InfoPopover — flip/collision detection (CopyPaste-g27b.35)
//
// Regression coverage for the "Excluded apps" overlap bug: the popover used
// to always centre itself vertically on the trigger with a fixed offset, so
// its bottom edge could land on top of the control it documents when that
// control sits directly below the label row (fullWidth SettingsRow layout).
// ---------------------------------------------------------------------------

function rect(overrides: Partial<DOMRect>): DOMRect {
  return {
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    width: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON() {
      return this;
    },
    ...overrides,
  } as DOMRect;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("InfoPopover", () => {
  it("opens and shows the text when the trigger is clicked", () => {
    render(<InfoPopover text="Explains the control." />);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "More info" }));
    expect(screen.getByRole("tooltip")).toHaveTextContent("Explains the control.");
  });

  it("closes on outside click", () => {
    render(<InfoPopover text="Explains the control." />);
    fireEvent.click(screen.getByRole("button", { name: "More info" }));
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });

  it("flips ABOVE the trigger when opening below would overlap the row's own control", () => {
    Object.defineProperty(window, "innerHeight", { value: 900, configurable: true });
    Object.defineProperty(window, "innerWidth", { value: 1200, configurable: true });

    render(
      <div className="srow">
        <div className="srow__l">
          <div className="srow__title">
            <span>Excluded apps</span>
            <InfoPopover text="Bundle IDs of apps whose clipboard is never captured." />
          </div>
        </div>
        <div className="srow__c">
          <textarea aria-label="Excluded apps list" />
        </div>
      </div>,
    );

    const btn = screen.getByRole("button", { name: "More info" });
    const control = document.querySelector(".srow__c") as HTMLElement;

    // Reproduces the reported evidence: trigger sits at ~632..652, the
    // control (textarea) starts right after it at 672 — far too close for a
    // 76px-tall popover to fit below without covering it.
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      if (this === btn) {
        return rect({ top: 632, bottom: 652, left: 300, right: 320, width: 20, height: 20 });
      }
      if (this === control) {
        return rect({ top: 672, bottom: 692, left: 20, right: 400, width: 380, height: 20 });
      }
      if (this.getAttribute("role") === "tooltip") {
        return rect({ top: 0, left: 0, right: 260, bottom: 76, width: 260, height: 76 });
      }
      return rect({});
    });

    fireEvent.click(btn);

    const popover = screen.getByRole("tooltip");
    expect(popover).toHaveStyle({ visibility: "visible" });
    const top = parseFloat((popover as HTMLElement).style.top);

    // Placed above the trigger (its bottom edge stays clear of the trigger's
    // top, with the gap) — never extending down into the control at 672.
    expect(top + 76).toBeLessThanOrEqual(632 - 6);
    expect(top + 76).toBeLessThanOrEqual(672);
  });

  it("opens BELOW the trigger (unchanged default) when there is no collision", () => {
    Object.defineProperty(window, "innerHeight", { value: 1200, configurable: true });
    Object.defineProperty(window, "innerWidth", { value: 1200, configurable: true });

    render(<InfoPopover text="Short description." />);
    const btn = screen.getByRole("button", { name: "More info" });

    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      if (this === btn) {
        return rect({ top: 100, bottom: 120, left: 50, right: 70, width: 20, height: 20 });
      }
      if (this.getAttribute("role") === "tooltip") {
        return rect({ top: 0, left: 0, right: 200, bottom: 60, width: 200, height: 60 });
      }
      return rect({});
    });

    fireEvent.click(btn);
    const popover = screen.getByRole("tooltip");
    const top = parseFloat((popover as HTMLElement).style.top);
    expect(top).toBeGreaterThanOrEqual(120);
  });
});
