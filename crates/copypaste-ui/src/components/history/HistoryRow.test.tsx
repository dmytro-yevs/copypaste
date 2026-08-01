/**
 * INV-10 / A11Y-3 / AT-13 — a sensitive item never discloses its content.
 *
 * The bridge drops the plaintext before it crosses (`content: null`), so the
 * strongest thing this file can assert is that the row does not *reconstruct*
 * anything from what it does receive, and that its accessible name is a fixed
 * string rather than a preview.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { HistoryRow, rowLabel } from "@/components/history/HistoryRow";
import { item } from "@/test/harness";
import type { Item } from "@/lib/ipc";

const SECRET = "AKIAIOSFODNN7EXAMPLE";
const noop = () => {};
const userAgent = navigator.userAgent;

afterEach(() => {
  Object.defineProperty(navigator, "userAgent", {
    configurable: true,
    value: userAgent,
  });
});

const props = {
  active: false,
  flashing: false,
  selecting: false,
  checked: false,
  onToggleChecked: noop,
  revealedContent: null,
  revealPending: false,
  previewLines: 2,
  origin: null,
  onSelect: noop,
  onCopy: noop,
  onTogglePin: noop,
  onDelete: noop,
  onReveal: noop,
  onHide: noop,
  onOpen: noop,
};

describe("a sensitive item", () => {
  it("renders a placeholder and no content", () => {
    const { container } = render(
      <HistoryRow {...props} item={item({ is_sensitive: true })} />,
    );
    expect(container.textContent).toContain("Sensitive content hidden");
    expect(container.textContent).not.toContain(SECRET);
  });

  it("labels the row without quoting anything about it", () => {
    const secret = item({ is_sensitive: true });
    // A11Y-3, verbatim from manifest 06 — asserted as the rendered sentence,
    // not as a catalogue key.
    expect(rowLabel(secret)).toBe("Sensitive item, hidden — activate to reveal");
    expect(rowLabel(secret)).not.toContain(SECRET);
  });

  it("offers reveal as a real button with a name", () => {
    const onReveal = vi.fn();
    render(
      <HistoryRow
        {...props}
        item={item({ is_sensitive: true })}
        onReveal={onReveal}
      />,
    );
    const button = screen.getByRole("button", {
      name: "Sensitive content hidden — activate to reveal",
    });
    button.click();
    expect(onReveal).toHaveBeenCalledTimes(1);
  });

  it("shows the plaintext only while it is the revealed row", () => {
    const secret = item({ is_sensitive: true });
    const { container, rerender } = render(
      <HistoryRow {...props} item={secret} />,
    );
    expect(container.textContent).not.toContain(SECRET);

    // Revealed: the plaintext arrives as a prop from useReveal, never from the
    // item, and it goes again when the reveal expires.
    rerender(<HistoryRow {...props} item={secret} revealedContent={SECRET} />);
    expect(container.textContent).toContain(SECRET);

    rerender(<HistoryRow {...props} item={secret} revealedContent={null} />);
    expect(container.textContent).not.toContain(SECRET);
  });

  it("still exposes copy, pin and delete — a hidden item is still operable", () => {
    render(<HistoryRow {...props} item={item({ is_sensitive: true })} />);
    expect(screen.getByRole("button", { name: "Copy to clipboard" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Pin item" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete item" })).toBeTruthy();
  });
});

describe("an ordinary item", () => {
  it("renders its content", () => {
    render(<HistoryRow {...props} item={item({ content: "visible text" })} />);
    expect(screen.getByText(/visible text/)).toBeTruthy();
  });

  it("clamps the preview to the configured number of lines", () => {
    // The clamp and the reserved height read the same number; if they diverge,
    // the row either overlaps its neighbour or leaves a gap (INV-5).
    const { container } = render(
      <HistoryRow
        {...props}
        previewLines={4}
        item={item({ content: "a\nb\nc\nd\ne\nf" })}
      />,
    );
    const preview = container.querySelector("[style*='line-clamp']");
    expect((preview as HTMLElement | null)?.style.webkitLineClamp).toBe("4");
  });

  it("names every control, and its title too (A11Y-9)", () => {
    const { container } = render(<HistoryRow {...props} item={item()} />);
    for (const button of container.querySelectorAll("button")) {
      expect(button.getAttribute("aria-label")?.length ?? 0).toBeGreaterThan(0);
      expect(button.getAttribute("title")?.length ?? 0).toBeGreaterThan(0);
    }
  });

  it("marks a pinned item in its accessible name", () => {
    expect(rowLabel(item({ pinned: true, content: "x" }))).toBe("Pinned. x");
  });
});

/**
 * B-15 / B-26. Both markers are drawn inside the row body rather than beside
 * the actions, which is what keeps the sync warning visible in selection mode
 * (CopyPaste-f72f) — v1 unmounted the whole action cluster there and took the
 * warning with it.
 */
describe("what the row says about where an item came from", () => {
  it("says nothing at all for an item this history cannot place elsewhere", () => {
    const { container } = render(<HistoryRow {...props} item={item()} />);
    expect(container.textContent).not.toMatch(/from|won't sync/i);
  });

  it("names the device when the item earns a marker", () => {
    render(<HistoryRow {...props} item={item()} origin="Kitchen Mac" />);
    expect(screen.getByText("Kitchen Mac")).toBeTruthy();
    expect(rowLabel(item({ content: "x" }), "Kitchen Mac")).toBe(
      "x · From Kitchen Mac",
    );
  });

  it("says an item will never leave this device, in its name too", () => {
    const stranded = { ...item({ content: "x" }), too_large_to_sync: true } as Item;
    render(<HistoryRow {...props} item={stranded} />);
    expect(screen.getByText("Won't sync")).toBeTruthy();
    expect(rowLabel(stranded)).toBe(
      "x · Too large to sync — this item stays on this device",
    );
  });

  it("keeps the sync warning while selecting, when the actions are gone", () => {
    const stranded = { ...item(), too_large_to_sync: true } as Item;
    render(<HistoryRow {...props} item={stranded} selecting />);
    expect(screen.queryByRole("button", { name: "Delete item" })).toBeNull();
    expect(screen.getByText("Won't sync")).toBeTruthy();
  });
});

describe("selecting is not copying", () => {
  it("selects on a single click and copies nothing", async () => {
    const onSelect = vi.fn();
    const onCopy = vi.fn();
    const user = userEvent.setup();
    render(
      <HistoryRow {...props} item={item()} onSelect={onSelect} onCopy={onCopy} />,
    );

    await user.click(screen.getByRole("button", { name: /ordinary clipboard/ }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    // A click that silently overwrites the system clipboard is a destructive
    // default; copying needs its own gesture.
    expect(onCopy).not.toHaveBeenCalled();
  });

  it("copies on a double click", () => {
    const onCopy = vi.fn();
    render(<HistoryRow {...props} item={item()} onCopy={onCopy} />);
    fireEvent.doubleClick(screen.getByRole("button", { name: /ordinary clipboard/ }));
    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("gives touch and screen-reader users an explicit Copy control", async () => {
    // Not a hover affordance: Android has no hover, so a hover-revealed
    // control does not exist there.
    const onCopy = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <HistoryRow {...props} item={item()} onCopy={onCopy} />,
    );
    const copy = screen.getByRole("button", { name: "Copy to clipboard" });
    expect(container.innerHTML).not.toContain("opacity-0");
    await user.click(copy);
    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("keeps the row body reachable from the keyboard when it is selected", () => {
    render(<HistoryRow {...props} item={item()} active />);
    for (const button of screen.getAllByRole("button")) {
      expect(button.tabIndex).toBe(0);
    }
  });

  it("keeps unselected rows out of the tab order", () => {
    render(<HistoryRow {...props} item={item()} />);
    for (const button of screen.getAllByRole("button")) {
      expect(button.tabIndex).toBe(-1);
    }
  });
});

describe("Android row actions", () => {
  it("selects on tap and keeps copying behind an explicit action", async () => {
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Linux; Android 15)",
    });
    const onCopy = vi.fn();
    const onSelect = vi.fn();
    const onTogglePin = vi.fn();
    const clip = item({ content: "mobile clipboard" });
    const user = userEvent.setup();

    render(
      <HistoryRow
        {...props}
        item={clip}
        onCopy={onCopy}
        onSelect={onSelect}
        onTogglePin={onTogglePin}
      />,
    );

    await user.click(screen.getByRole("button", { name: /mobile clipboard/ }));
    expect(onSelect).toHaveBeenCalledWith(clip);
    expect(onCopy).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Item actions" }));
    expect(screen.getByRole("dialog", { name: "Item actions" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Copy to clipboard" }));
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(onCopy).toHaveBeenCalledWith(clip);

    await user.click(screen.getByRole("button", { name: "Item actions" }));
    expect(screen.getByRole("dialog", { name: "Item actions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show full contents" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Pin item" }));
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(onTogglePin).toHaveBeenCalledWith(clip);
  });
});
