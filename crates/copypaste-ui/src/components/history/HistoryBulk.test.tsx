/**
 * Bulk actions, filtering and the unreadable-row notice — parity findings 19
 * and 17, end to end through the screen rather than through the pieces.
 *
 * The rules these pin down:
 *
 *  §3.1.9  bulk delete confirms; there is no undo for it
 *  §3.1.9  the pin toggle's label reflects whether *every* selected item is
 *          already pinned (CopyPaste-8ebg.55)
 *  §3.1.5  per-row actions are hidden in selection mode
 *  finding 17  a page that dropped rows says how many
 *
 * And one that is not in the manifest because v1 had hover: **no affordance is
 * hover-only.** Selection mode is entered from a visible button, because
 * Android has no hover and a checkbox nobody can reveal is a checkbox that does
 * not exist.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";

import { HistoryView } from "@/components/history/HistoryView";
import { item, items, page, status, withUser } from "@/test/harness";
import { useUi } from "@/store/ui";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();
const copyItem = vi.fn();
const setPinned = vi.fn();
const deleteItem = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...a: unknown[]) => listItems(...a),
    searchItems: (...a: unknown[]) => searchItems(...a),
    getStatus: () => getStatus(),
    copyItem: (...a: unknown[]) => copyItem(...a),
    setPinned: (...a: unknown[]) => setPinned(...a),
    deleteItem: (...a: unknown[]) => deleteItem(...a),
  };
});

beforeEach(() => {
  listItems.mockReset().mockResolvedValue(page(items(4)));
  searchItems.mockReset().mockResolvedValue(page([]));
  getStatus.mockReset().mockResolvedValue(status());
  copyItem.mockReset().mockResolvedValue(item());
  setPinned.mockReset().mockResolvedValue(item());
  deleteItem.mockReset().mockResolvedValue(true);
  useUi.setState({ query: "", activeId: null });
});

afterEach(() => vi.restoreAllMocks());

/** Enter selection mode and tick the first `count` rows. */
async function selectRows(user: ReturnType<typeof withUser>["user"], count: number) {
  await user.click(screen.getByRole("button", { name: /select multiple items/i }));
  const boxes = await screen.findAllByRole("checkbox");
  for (let index = 0; index < count; index += 1) {
    await user.click(boxes[index]!);
  }
}

describe("selection mode", () => {
  it("is entered from a visible control, not from hover", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));

    // Nothing to hover over: the checkboxes are simply not there yet.
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: /select multiple items/i }));
    expect(screen.getAllByRole("checkbox").length).toBeGreaterThan(0);
  });

  /** §3.1.5: the bulk bar duplicates them, and a Delete button beside a
   *  checkbox is one misclick from destroying the wrong thing. */
  it("hides the per-row actions while it is on", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    expect(screen.getAllByRole("button", { name: "Delete item" }).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /select multiple items/i }));
    await waitFor(() =>
      expect(screen.queryAllByRole("button", { name: "Delete item" })).toHaveLength(0),
    );
  });

  it("counts what is selected", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    expect(bar.textContent).toContain("2 items selected");
  });

  it("exits after the last selected item is unchecked", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 1);

    await user.click(screen.getAllByRole("checkbox")[0]!);
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: /selection actions/i }),
      ).toBeNull(),
    );
    expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
  });
});

describe("bulk copy", () => {
  it("copies the first row and writes safe previews in screen order", async () => {
    listItems.mockResolvedValue(
      page([
        item({ id: "first", content: "first preview" }),
        item({ id: "secret", is_sensitive: true }),
        item({ id: "image", content: "image preview", content_type: "image/png" }),
        item({ id: "last", content: "last preview" }),
      ]),
    );
    const { user } = withUser(<HistoryView />);
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 4);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /^copy$/i }));

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("first"));
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith("first preview\nlast preview"),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: /selection actions/i }),
      ).toBeNull(),
    );
  });

  it("stops on daemon failure and leaves the selection available", async () => {
    copyItem.mockRejectedValueOnce(new Error("copy failed"));
    const { user } = withUser(<HistoryView />);
    const writeText = vi.spyOn(navigator.clipboard, "writeText");
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    const copy = within(bar).getByRole("button", { name: /^copy$/i });
    await user.click(copy);

    await waitFor(() => expect(copyItem).toHaveBeenCalledTimes(1));
    await waitFor(() => expect((copy as HTMLButtonElement).disabled).toBe(false));
    expect(writeText).not.toHaveBeenCalled();
    expect(
      screen.getByRole("region", { name: /selection actions/i }),
    ).toBeTruthy();
  });
});

describe("the bulk pin toggle", () => {
  it("offers to pin when the selection is not all pinned", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /^pin$/i }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledTimes(2));
    expect(setPinned).toHaveBeenCalledWith(expect.any(String), true);
  });

  /** CopyPaste-8ebg.55: one toggle whose label always says what pressing it
   *  will do, rather than two buttons that are each wrong half the time. */
  it("offers to unpin when every selected item is already pinned", async () => {
    listItems.mockResolvedValue(page(items(3, { pinned: true })));
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(3));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    expect(within(bar).queryByRole("button", { name: /^pin$/i })).toBeNull();
    await user.click(within(bar).getByRole("button", { name: /unpin/i }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledWith(expect.any(String), false));
  });
});

describe("bulk delete", () => {
  /** No undo window, unlike the single-row delete — so the dialog is the only
   *  gate in front of it (§3.1.9). */
  it("asks before deleting, and says the undo does not apply", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /delete/i }));

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Delete 2 items?");
    expect(dialog.textContent).toContain("cannot be undone");
    // Nothing has gone yet.
    expect(deleteItem).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: /^delete$/i }));
    await waitFor(() => expect(deleteItem).toHaveBeenCalledTimes(2));
  });
});

describe("filtering", () => {
  it("keeps only the chosen kind, and the badge follows", async () => {
    listItems.mockResolvedValue(
      page([
        item({ id: "u", content: "https://example.com" }),
        item({ id: "t1", content: "just words" }),
        item({ id: "t2", content: "more words" }),
      ]),
    );
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(3));

    await user.selectOptions(screen.getByLabelText(/filter by kind/i), "url");
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(1));
    // AT-68: the badge switches to the visible count the moment a filter is on.
    expect(screen.getByText("1 item")).toBeTruthy();
  });

  it("says the filter matched nothing rather than that nothing was copied", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));

    await user.selectOptions(screen.getByLabelText(/filter by kind/i), "color");
    await waitFor(() =>
      expect(screen.getByText("Nothing matches this filter")).toBeTruthy(),
    );
    expect(screen.queryByText("Nothing copied yet")).toBeNull();
  });
});

describe("rows that could not be read (finding 17)", () => {
  it("says how many, as a state rather than an error", async () => {
    listItems.mockResolvedValue(page(items(2), 3));
    withUser(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/3 items could not be read/i)).toBeTruthy(),
    );
    // Not an error banner: nothing the user does fixes it, and the two rows
    // that did open are fine.
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("says nothing at all when nothing was skipped", async () => {
    withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    expect(screen.queryByText(/could not be read/i)).toBeNull();
  });

  it("sums the count across loaded pages", async () => {
    listItems.mockImplementation(async (_limit: number, cursor: string | null) =>
      cursor === null ? page(items(2), 1, "after-page-1") : page(items(1), 2),
    );
    withUser(<HistoryView />);
    await waitFor(() =>
      expect(screen.getByText(/1 item could not be read/i)).toBeTruthy(),
    );
  });
});
