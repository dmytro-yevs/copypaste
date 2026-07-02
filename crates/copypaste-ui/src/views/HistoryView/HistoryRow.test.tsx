import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { HistoryEntry } from "../../lib/ipc";
import { HistoryRow, type RowProps } from "./HistoryRow";

afterEach(cleanup);

const entry = (over: Partial<HistoryEntry> = {}): HistoryEntry => ({
  id: "1",
  content_type: "text/plain",
  preview: "hello world",
  is_sensitive: false,
  wall_time: 1_700_000_000_000,
  pinned: false,
  ...over,
});

const props = (e: HistoryEntry, over: Partial<RowProps> = {}): RowProps => ({
  entry: e,
  selected: false,
  multiSelected: false,
  selectionMode: false,
  previewLines: 1,
  previewSize: 28,
  imageMaxHeight: 40,
  maskSensitive: true,
  showSensitiveWarnings: true,
  density: "compact",
  ownDeviceId: "dev-1",
  onSelect: () => {},
  onToggleMultiSelect: () => {},
  onCopy: () => {},
  onPin: () => {},
  onDelete: () => {},
  onPreview: () => {},
  ...over,
});

describe("HistoryRow — sensitive masking a11y (X6 / P0 A11Y-1 fix)", () => {
  it("masked row: aria-label is the placeholder, never the plaintext", () => {
    render(<HistoryRow {...props(entry({ preview: "secret-token-xyz", is_sensitive: true }))} />);
    const row = screen.getByRole("option");
    expect(row.getAttribute("aria-label")).toMatch(/hidden/i);
    expect(row.getAttribute("aria-label")).not.toContain("secret-token-xyz");
    // Real text is present (width preserved) but aria-hidden.
    const mask = document.querySelector(".mask");
    expect(mask).toHaveTextContent("secret-token-xyz");
    expect(mask).toHaveAttribute("aria-hidden", "true");
  });

  it("non-sensitive row: aria-label includes the preview and a tile renders", () => {
    render(
      <HistoryRow {...props(entry({ preview: "hello world", kind: "TEXT" }), { maskSensitive: false })} />,
    );
    const row = screen.getByRole("option");
    expect(row.getAttribute("aria-label")).toContain("hello world");
    expect(document.querySelector(".tile")).not.toBeNull();
    expect(row).toHaveClass("row");
  });

  it("pinned row carries the .pinned class; selected carries .sel", () => {
    render(<HistoryRow {...props(entry({ pinned: true }), { multiSelected: true })} />);
    const row = screen.getByRole("option");
    expect(row).toHaveClass("pinned");
    expect(row).toHaveClass("sel");
    expect(row).toHaveAttribute("aria-selected", "true");
  });
});
