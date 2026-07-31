/**
 * B-17: the way to read a clipping the list clamped.
 *
 * The rules worth pinning down are the ones a later edit could quietly undo:
 * a sensitive item is withheld here exactly as it is in the row (INV-10) and
 * goes back behind the placeholder when the reveal expires (INV-11), the text
 * scrolls inside its own box rather than stretching the screen, and copying
 * does not require closing the view first.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";

import { HistoryView } from "@/components/history/HistoryView";
import { item, items, page, status, withUser } from "@/test/harness";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const LONG = Array.from({ length: 40 }, (_, i) => `line ${i}`).join("\n");
const SECRET = "AKIAIOSFODNN7EXAMPLE";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();
const copyItem = vi.fn();
const revealItem = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...a: unknown[]) => listItems(...a),
    searchItems: (...a: unknown[]) => searchItems(...a),
    getStatus: () => getStatus(),
    copyItem: (...a: unknown[]) => copyItem(...a),
    revealItem: (...a: unknown[]) => revealItem(...a),
  };
});

beforeEach(() => {
  listItems.mockReset().mockResolvedValue(page([item({ id: "long", content: LONG })]));
  searchItems.mockReset().mockResolvedValue(page([]));
  getStatus.mockReset().mockResolvedValue(status());
  copyItem.mockReset().mockResolvedValue(item());
  revealItem.mockReset().mockResolvedValue(SECRET);
  useUi.setState({ query: "", activeId: null });
  usePrefs.setState({ warnBeforeReveal: false });
});

afterEach(() => vi.restoreAllMocks());

async function open(user: ReturnType<typeof withUser>["user"]) {
  await waitFor(() => expect(screen.getAllByRole("listitem").length).toBeGreaterThan(0));
  await user.click(screen.getAllByRole("button", { name: /show full contents/i })[0]!);
  return screen.findByRole("dialog");
}

describe("reading a long clipping", () => {
  it("shows the whole of it, not the clamped preview", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    expect(dialog.textContent).toContain("line 0");
    expect(dialog.textContent).toContain("line 39");
  });

  it("scrolls the text inside its own box", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    const region = within(dialog).getByRole("region", { name: /item contents/i });
    expect(region.className).toContain("overflow-y-auto");
    expect(region.className).toContain("max-h-");
  });

  it("puts focus on the text, so a keyboard can scroll it at once", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    const region = within(dialog).getByRole("region", { name: /item contents/i });
    await waitFor(() => expect(document.activeElement).toBe(region));
  });

  it("closes on Escape and hands focus back to the list", async () => {
    const { user } = withUser(<HistoryView />);
    await open(user);
    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByRole("list", { name: "Clipboard history" }).parentElement).toBe(
      document.activeElement,
    );
  });

  it("copies from inside the view", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    await user.click(within(dialog).getByRole("button", { name: /^copy$/i }));
    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("long"));
  });
});

describe("a sensitive item in the detail view", () => {
  beforeEach(() => {
    listItems.mockResolvedValue(page([item({ id: "secret", is_sensitive: true })]));
  });

  it("withholds it exactly as the row does", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    expect(dialog.textContent).toContain("Sensitive content hidden");
    expect(dialog.textContent).not.toContain(SECRET);
  });

  it("reveals through the same fetch the row uses, and re-masks when it ends", async () => {
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    await user.click(within(dialog).getByRole("button", { name: /reveal/i }));

    await waitFor(() =>
      expect(
        within(screen.getByRole("dialog")).getByText(SECRET),
      ).toBeTruthy(),
    );
    expect(revealItem).toHaveBeenCalledWith("secret");

    // INV-11's other expiry: the window losing focus drops the plaintext, and
    // the view holds no copy of it to show afterwards.
    window.dispatchEvent(new Event("blur"));
    await waitFor(() => expect(screen.queryAllByText(SECRET)).toHaveLength(0));
    expect(screen.getByRole("dialog").textContent).toContain(
      "Sensitive content hidden",
    );
  });

  it("asks first when the warning preference is on", async () => {
    usePrefs.setState({ warnBeforeReveal: true });
    const { user } = withUser(<HistoryView />);
    const dialog = await open(user);
    await user.click(within(dialog).getByRole("button", { name: /reveal/i }));

    const confirm = await screen.findByRole("alertdialog");
    expect(confirm.textContent).toContain("Reveal sensitive content?");
    expect(revealItem).not.toHaveBeenCalled();
  });
});

describe("the view is a window onto the list, not a copy of it", () => {
  it("closes when the item it was showing leaves the history", async () => {
    listItems.mockResolvedValueOnce(page(items(2)));
    const { user } = withUser(<HistoryView />);
    await open(user);

    listItems.mockResolvedValue(page([]));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull(), {
      timeout: 5000,
    });
  });
});
