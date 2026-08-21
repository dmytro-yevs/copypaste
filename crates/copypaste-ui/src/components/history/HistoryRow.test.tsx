/**
 * INV-10 / A11Y-3 / AT-13 — a sensitive item never discloses its content.
 *
 * The bridge drops the plaintext before it crosses (`content: null`), so the
 * strongest thing this file can assert is that the row does not *reconstruct*
 * anything from what it does receive, and that its accessible name is a fixed
 * string rather than a preview.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { CSSProperties, ReactElement } from "react";

import { HistoryRow, rowLabel } from "@/components/history/HistoryRow";
import { absoluteTime } from "@/lib/format";
import { imagePreviewHeight } from "@/lib/layout";
import { item, withClient } from "@/test/harness";
import type { Item } from "@/lib/ipc";

// The row's source-app icon is a React Query leaf now (F-UI-6), so every mount
// of a row needs a client.
const render = (ui: ReactElement) => withClient(ui);

// Thumbnail decoding has its own contract tests. Here the row tests the
// layout contract: image rows keep the leading type icon and pass the image
// preview height through to the body preview.
vi.mock("@/components/history/HistoryImagePreview", () => ({
  HistoryImagePreview: ({ style }: { style?: CSSProperties }) => (
    <span data-testid="image-preview" aria-hidden="true" style={style} />
  ),
}));

const SECRET = "AKIAIOSFODNN7EXAMPLE";
const POTENTIAL_ORIGINAL = "Email alice@example.com about the release";
const REDACTED_PREVIEW = "Email ***REDACTED*** about the release";
const FINDING = {
  label: "email",
  spans: [{ start: 6, end: 23 }],
  spans_truncated: false,
  redacted_preview: REDACTED_PREVIEW,
};
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
  onOpen: noop,
};

describe("a sensitive item", () => {
  it("renders a masked preview and no content", () => {
    const { container } = render(
      <HistoryRow {...props} item={item({ is_sensitive: true })} />,
    );
    expect(
      screen.getByRole("button", {
        name: "Sensitive item, hidden — activate to reveal",
      }),
    ).toBeTruthy();
    expect(container.textContent).not.toContain(SECRET);
    expect(container.querySelector("[aria-hidden='true'].flex.flex-col")).toBeTruthy();
  });

  it("labels the row without quoting anything about it", () => {
    const secret = item({ is_sensitive: true });
    // A11Y-3, verbatim from manifest 06 — asserted as the rendered sentence,
    // not as a catalogue key.
    expect(rowLabel(secret)).toBe("Sensitive item, hidden — activate to reveal");
    expect(rowLabel(secret)).not.toContain(SECRET);
  });

  it("reveals directly from the masked row without copying it", () => {
    const onReveal = vi.fn();
    const onCopy = vi.fn();
    render(
      <HistoryRow
        {...props}
        item={item({ is_sensitive: true })}
        onReveal={onReveal}
        onCopy={onCopy}
      />,
    );
    const button = screen.getByRole("button", {
      name: "Sensitive item, hidden — activate to reveal",
    });
    button.click();
    expect(onReveal).toHaveBeenCalledTimes(1);
    expect(onCopy).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: "Show original content" }),
    ).toBeNull();
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

describe("an item with a potential-sensitive finding", () => {
  const potential = item({
    content: POTENTIAL_ORIGINAL,
    sensitive_finding: FINDING,
  });

  it("renders only the backend-redacted preview and an accessible warning", () => {
    const { container } = render(<HistoryRow {...props} item={potential} />);

    expect(screen.getByText(REDACTED_PREVIEW)).toBeTruthy();
    expect(
      screen.getByRole("status", {
        name: "Potentially sensitive content",
      }),
    ).toBeTruthy();
    expect(container.textContent).not.toContain(POTENTIAL_ORIGINAL);
    expect(container.innerHTML).not.toContain(POTENTIAL_ORIGINAL);
    expect(potential.is_sensitive).toBe(false);
    expect(rowLabel(potential)).not.toContain(POTENTIAL_ORIGINAL);
  });

  it("shows and hides the original from an explicit keyboard action", async () => {
    const onReveal = vi.fn();
    const user = userEvent.setup();
    render(
      <HistoryRow
        {...props}
        item={potential}
        active
        onReveal={onReveal}
      />,
    );

    const show = screen.getByRole("button", { name: "Show original content" });
    show.focus();
    await user.keyboard("{Enter}");

    expect(screen.getByText(POTENTIAL_ORIGINAL)).toBeTruthy();
    expect(onReveal).not.toHaveBeenCalled();
    const hide = screen.getByRole("button", { name: "Hide original content" });
    expect(hide.getAttribute("aria-pressed")).toBe("true");

    hide.focus();
    await user.keyboard(" ");
    expect(screen.queryByText(POTENTIAL_ORIGINAL)).toBeNull();
    expect(screen.getByText(REDACTED_PREVIEW)).toBeTruthy();
  });

  it("copies only after the user activates a copy control", async () => {
    const onCopy = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <HistoryRow {...props} item={potential} active onCopy={onCopy} />,
    );

    expect(onCopy).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Copy to clipboard" }));
    expect(onCopy).toHaveBeenCalledWith(potential);
    expect(container.textContent).not.toContain(POTENTIAL_ORIGINAL);
  });
});

describe("an ordinary item", () => {
  it("renders its content", () => {
    render(<HistoryRow {...props} item={item({ content: "visible text" })} />);
    expect(screen.getByText(/visible text/)).toBeTruthy();
  });

  it("shows the clip type as an icon, then source app and copied time below content", () => {
    const { container } = render(
      <HistoryRow
        {...props}
        item={item({
          content_type: "image/png",
          source_app_bundle_id: "com.google.Chrome",
        })}
      />,
    );

    expect(screen.getByTitle("Image")).toBeTruthy();
    expect(screen.queryByText("Image")).toBeNull();
    expect(screen.getByText("Google Chrome")).toBeTruthy();
    const time = container.querySelector("button time");
    expect(time?.textContent).toMatch(/\d+[mhdw]|now/);
    expect(time?.className).not.toContain("md:block");
  });

  it("keeps the image type icon in the leading slot and its preview in the row body", () => {
    render(
      <HistoryRow
        {...props}
        item={item({ content_type: "image/png", source_app_bundle_id: "com.google.Chrome" })}
      />,
    );

    const icon = document.querySelector("span[role='img'][title='Image']");
    const body = screen.getByRole("button", { name: "Image" });
    expect(icon?.parentElement).not.toBe(body);
    expect(body.querySelector("[data-testid='image-preview']")).toBeTruthy();
  });

  it("allows an image preview to use 3.75× the configured text-preview height", () => {
    const { container } = render(
      <HistoryRow
        {...props}
        previewLines={4}
        item={item({ content_type: "image/png" })}
      />,
    );

    const preview = container.querySelector("[data-testid='image-preview']");
    expect((preview as HTMLElement | null)?.style.maxHeight).toBe(
      `${imagePreviewHeight(4)}px`,
    );
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

describe("desktop activation", () => {
  it("selects and copies once on a single click", async () => {
    const onSelect = vi.fn();
    const onCopy = vi.fn();
    const user = userEvent.setup();
    render(
      <HistoryRow {...props} item={item()} onSelect={onSelect} onCopy={onCopy} />,
    );

    await user.click(screen.getByRole("button", { name: /ordinary clipboard/ }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("does not copy twice when a user double-clicks", async () => {
    const onSelect = vi.fn();
    const onCopy = vi.fn();
    const user = userEvent.setup();
    render(
      <HistoryRow
        {...props}
        item={item()}
        onSelect={onSelect}
        onCopy={onCopy}
      />,
    );

    await user.dblClick(
      screen.getByRole("button", { name: /ordinary clipboard/ }),
    );
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("selects and copies once when the row button is keyboard-activated", async () => {
    const onSelect = vi.fn();
    const onCopy = vi.fn();
    const user = userEvent.setup();
    render(
      <HistoryRow
        {...props}
        item={item()}
        active
        onSelect={onSelect}
        onCopy={onCopy}
      />,
    );
    screen.getByRole("button", { name: /ordinary clipboard/ }).focus();

    await user.keyboard("{Enter}");
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onCopy).toHaveBeenCalledTimes(1);
  });

  it("toggles selection mode without copying", async () => {
    const onToggleChecked = vi.fn();
    const onCopy = vi.fn();
    const user = userEvent.setup();
    const clip = item();
    render(
      <HistoryRow
        {...props}
        item={clip}
        selecting
        onToggleChecked={onToggleChecked}
        onCopy={onCopy}
      />,
    );

    await user.click(screen.getByRole("button", { name: /ordinary clipboard/ }));
    expect(onToggleChecked).toHaveBeenCalledWith(clip);
    expect(onCopy).not.toHaveBeenCalled();
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

  it("toggles copied time to the exact local time without copying the item", async () => {
    const onCopy = vi.fn();
    const clip = item({ created_at: new Date("2026-08-01T20:35:24Z").valueOf() });
    const user = userEvent.setup();
    render(<HistoryRow {...props} item={clip} active onCopy={onCopy} />);

    const time = screen.getByRole("button", { name: /Activate to show exact time/ });
    await user.click(time);

    expect(time.textContent).toBe(absoluteTime(clip.created_at));
    expect(time.getAttribute("aria-pressed")).toBe("true");
    expect(onCopy).not.toHaveBeenCalled();
    await user.click(time);
    expect(time.getAttribute("aria-pressed")).toBe("false");
  });

  it("keeps unselected rows out of the tab order", () => {
    render(<HistoryRow {...props} item={item()} />);
    for (const button of screen.getAllByRole("button")) {
      expect(button.tabIndex).toBe(-1);
    }
  });
});

describe("Android row actions", () => {
  it("selects and copies once on tap, while keeping explicit actions", async () => {
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
    expect(onCopy).toHaveBeenCalledTimes(1);
    expect(onCopy).toHaveBeenLastCalledWith(clip);

    await user.click(screen.getByRole("button", { name: "Item actions" }));
    const actions = screen.getByRole("dialog", { name: "Item actions" });
    expect(actions.className).toContain("bottom-[calc(var(--inset-bottom)+var(--s-2))]");
    expect(actions.className).toContain("bg-card");
    await user.click(screen.getByRole("button", { name: "Copy to clipboard" }));
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(onCopy).toHaveBeenCalledTimes(2);
    expect(onCopy).toHaveBeenLastCalledWith(clip);

    await user.click(screen.getByRole("button", { name: "Item actions" }));
    expect(screen.getByRole("dialog", { name: "Item actions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show full contents" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Pin item" }));
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(onTogglePin).toHaveBeenCalledWith(clip);
  });
});
