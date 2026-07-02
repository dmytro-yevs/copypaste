import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { KeyboardEvent } from "react";
import { DisclosureHeader } from "./DisclosureHeader";
import { tabListKeyDown } from "./tabListKeyDown";

afterEach(cleanup);

function keyEvent(key: string): KeyboardEvent {
  return { key, preventDefault: vi.fn() } as unknown as KeyboardEvent;
}

describe("tabListKeyDown", () => {
  it("ArrowRight advances and wraps to the first", () => {
    const onSelect = vi.fn();
    tabListKeyDown({ count: 3, current: 2, onSelect })(keyEvent("ArrowRight"));
    expect(onSelect).toHaveBeenCalledWith(0);
  });

  it("ArrowLeft retreats and wraps to the last", () => {
    const onSelect = vi.fn();
    tabListKeyDown({ count: 3, current: 0, onSelect })(keyEvent("ArrowLeft"));
    expect(onSelect).toHaveBeenCalledWith(2);
  });

  it("Home and End jump to the bounds", () => {
    const onSelect = vi.fn();
    tabListKeyDown({ count: 4, current: 2, onSelect })(keyEvent("Home"));
    tabListKeyDown({ count: 4, current: 2, onSelect })(keyEvent("End"));
    expect(onSelect).toHaveBeenNthCalledWith(1, 0);
    expect(onSelect).toHaveBeenNthCalledWith(2, 3);
  });

  it("vertical orientation uses Up/Down", () => {
    const onSelect = vi.fn();
    tabListKeyDown({ count: 2, current: 0, onSelect, orientation: "vertical" })(
      keyEvent("ArrowDown"),
    );
    expect(onSelect).toHaveBeenCalledWith(1);
  });

  it("ignores unrelated keys", () => {
    const onSelect = vi.fn();
    tabListKeyDown({ count: 2, current: 0, onSelect })(keyEvent("a"));
    expect(onSelect).not.toHaveBeenCalled();
  });
});

describe("DisclosureHeader", () => {
  it("exposes aria-expanded/aria-controls and toggles on click", () => {
    const onToggle = vi.fn();
    render(
      <DisclosureHeader expanded={false} controls="panel-1" onToggle={onToggle}>
        Header
      </DisclosureHeader>,
    );
    const btn = screen.getByRole("button", { name: "Header" });
    expect(btn).toHaveAttribute("aria-expanded", "false");
    expect(btn).toHaveAttribute("aria-controls", "panel-1");
    fireEvent.click(btn);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("reflects the expanded state", () => {
    render(
      <DisclosureHeader expanded controls="p" onToggle={() => {}}>
        Open
      </DisclosureHeader>,
    );
    expect(screen.getByRole("button", { name: "Open" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });
});
