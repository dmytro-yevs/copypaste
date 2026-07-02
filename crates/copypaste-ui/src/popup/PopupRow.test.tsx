/**
 * PopupRow.test.tsx — g27b.29 nested-interactive structural guard.
 *
 * Mirrors HistoryRow.test.tsx's a11y coverage: role="option" has ARIA
 * childrenPresentational:true, which flattens descendant interactive
 * semantics and trips axe's nested-interactive check (serious, WCAG 4.1.2)
 * on the nested Pin <button>. PopupRow now uses role="group" instead — see
 * PopupRow.tsx's g27b.29 comment for the full rationale.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { HistoryEntry } from "../lib/ipc";
import { PopupRow, type PopupRowProps } from "./PopupRow";

afterEach(cleanup);

const item = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: "1",
  content_type: "text/plain",
  preview: "hello world",
  is_sensitive: false,
  wall_time: 1_700_000_000_000,
  pinned: false,
  ...over,
});

const props = (i: HistoryEntry, over: Partial<PopupRowProps> = {}): PopupRowProps => ({
  item: i,
  index: 0,
  selected: false,
  textRowHeight: 34,
  imageMaxHeight: 40,
  maskSensitive: true,
  matchPositions: [],
  previewLines: 1,
  showKeycap: false,
  onMouseEnter: () => {},
  onClick: () => {},
  onPin: () => {},
  ...over,
});

// PopupRow renders an <li> — testing-library needs a <ul>/<ol> ancestor for
// the implicit `list` role tree to be well-formed, mirroring Popup.tsx's
// real `role="listbox"` <ul>.
function renderRow(p: PopupRowProps) {
  return render(
    <ul role="list" aria-label="Clipboard history">
      <PopupRow {...p} />
    </ul>,
  );
}

describe("PopupRow — g27b.29 nested-interactive fix", () => {
  it("the row's role is group, not option — group has no ARIA childrenPresentational flattening, so the nested Pin button stays individually operable by assistive tech", () => {
    renderRow(props(item()));
    const row = screen.getByRole("listitem");
    expect(row.getAttribute("role")).toBe("listitem");
    expect(row.querySelector("button")).not.toBeNull();
  });

  it("selected state is exposed via aria-current (group disallows aria-selected)", () => {
    renderRow(props(item(), { selected: true }));
    const row = screen.getByRole("listitem");
    expect(row).toHaveAttribute("aria-current", "true");
    expect(row).not.toHaveAttribute("aria-selected");
    expect(row).toHaveClass("sel");
  });

  it("unselected row carries no aria-current", () => {
    renderRow(props(item(), { selected: false }));
    const row = screen.getByRole("listitem");
    expect(row).not.toHaveAttribute("aria-current");
  });

  it("Pin button click still fires onPin (behaviour preserved through the role change)", async () => {
    const onPin = vi.fn();
    renderRow(props(item({ pinned: false }), { onPin }));
    const pinBtn = screen.getByRole("button", { name: "Pin" });
    pinBtn.click();
    expect(onPin).toHaveBeenCalledTimes(1);
  });
});
