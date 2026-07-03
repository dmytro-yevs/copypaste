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
    // g27b.29: role="option" flattens descendant interactive semantics
    // (ARIA childrenPresentational), tripping axe's nested-interactive check
    // on the row's checkbox/Pin/Preview/Delete controls — the row is now
    // role="group" instead (see HistoryRow.tsx's g27b.29 comment).
    const row = screen.getByRole("listitem");
    expect(row.getAttribute("aria-label")).toMatch(/hidden/i);
    expect(row.getAttribute("aria-label")).not.toContain("secret-token-xyz");
    // Real text is present (width preserved). CopyPaste-8ebg.55: `.mask` is a
    // real <button> (fixes keyboard-unreachable reveal), so aria-hidden lives
    // on the inner text span only — the button itself must stay in the a11y
    // tree (with its own aria-label) or it would vanish from AT entirely.
    const mask = document.querySelector(".mask");
    expect(mask).toHaveTextContent("secret-token-xyz");
    expect(mask).not.toHaveAttribute("aria-hidden");
    expect(mask?.querySelector("span[aria-hidden='true']")).not.toBeNull();
  });

  it("non-sensitive row: aria-label includes the preview and a tile renders", () => {
    render(
      <HistoryRow {...props(entry({ preview: "hello world", kind: "TEXT" }), { maskSensitive: false })} />,
    );
    const row = screen.getByRole("listitem");
    expect(row.getAttribute("aria-label")).toContain("hello world");
    expect(document.querySelector(".tile")).not.toBeNull();
    expect(row).toHaveClass("row");
  });

  it("pinned row carries the .pinned class; selected carries .sel", () => {
    render(<HistoryRow {...props(entry({ pinned: true }), { multiSelected: true })} />);
    const row = screen.getByRole("listitem");
    expect(row).toHaveClass("pinned");
    expect(row).toHaveClass("sel");
    // aria-selected is not an allowed attribute on role="group" — the
    // selected state is exposed via aria-current instead (g27b.29).
    expect(row).toHaveAttribute("aria-current", "true");
  });
});

describe("HistoryRow — g27b.29 nested-interactive structural guard", () => {
  it("the row's own role has no ARIA childrenPresentational flattening (not option/button/link/etc.) so nested Pin/Preview/Delete buttons and the multi-select checkbox stay individually operable by assistive tech", () => {
    render(<HistoryRow {...props(entry())} />);
    const row = screen.getByRole("listitem");
    expect(row.getAttribute("role")).toBe("listitem");
    // Sanity: the controls this fix is protecting are still real descendants.
    expect(row.querySelectorAll('button, [role="checkbox"]').length).toBeGreaterThan(0);
  });
});
