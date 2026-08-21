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
import { act, screen, waitFor, within } from "@testing-library/react";

import { HistoryView } from "@/components/history/HistoryView";
import { item, items, page, status, withUser } from "@/test/harness";
import { useUi } from "@/store/ui";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();
const copyItem = vi.fn();
const copyItems = vi.fn();
const setPinned = vi.fn();
const deleteItem = vi.fn();
const toastSuccess = vi.fn();
const toastWarning = vi.fn();
const toastError = vi.fn();
const toastDismiss = vi.fn();

vi.mock("sonner", async (importOriginal) => {
  const actual = await importOriginal<typeof import("sonner")>();
  return {
    ...actual,
    toast: Object.assign((...a: unknown[]) => toastSuccess(...a), {
      dismiss: (...a: unknown[]) => toastDismiss(...a),
      success: (...a: unknown[]) => toastSuccess(...a),
      warning: (...a: unknown[]) => toastWarning(...a),
      error: (...a: unknown[]) => toastError(...a),
      info: (...a: unknown[]) => toastSuccess(...a),
    }),
  };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...a: unknown[]) => listItems(...a),
    searchItems: (...a: unknown[]) => searchItems(...a),
    getStatus: () => getStatus(),
    copyItem: (...a: unknown[]) => copyItem(...a),
    copyItems: (...a: unknown[]) => copyItems(...a),
    setPinned: (...a: unknown[]) => setPinned(...a),
    deleteItem: (...a: unknown[]) => deleteItem(...a),
  };
});

beforeEach(() => {
  listItems.mockReset().mockResolvedValue(page(items(4)));
  searchItems.mockReset().mockResolvedValue(page([]));
  getStatus.mockReset().mockResolvedValue(status());
  copyItem.mockReset().mockResolvedValue(item());
  // Answers the copied count, which is the selection size unless the backend
  // left sensitive or binary rows out.
  copyItems.mockReset().mockImplementation(async (ids: string[]) => ids.length);
  setPinned.mockReset().mockResolvedValue(item());
  deleteItem.mockReset().mockResolvedValue(true);
  toastSuccess.mockReset();
  toastWarning.mockReset();
  toastError.mockReset();
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
  const mixed = () =>
    page([
      item({ id: "first", content: "first prev", truncated: true }),
      item({ id: "secret", is_sensitive: true }),
      item({ id: "image", content: "image preview", content_type: "image/png" }),
      item({ id: "last", content: "last prev", truncated: true }),
    ]);

  async function copySelection(count: number) {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, count);
    const bar = screen.getByRole("region", { name: /selection actions/i });
    const copy = within(bar).getByRole("button", { name: /^copy$/i });
    await user.click(copy);
    return { copy };
  }

  /** §3.1.9. v1 copied the first item through the daemon and *then* wrote the
   *  joined text, so two writes raced for one clipboard slot and the toast
   *  fired on the first of them. One command, one write, one report. */
  it("hands the whole selection to one command and does not copy the first row twice", async () => {
    listItems.mockResolvedValue(mixed());
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);

    await copySelection(4);

    await waitFor(() =>
      expect(copyItems).toHaveBeenCalledWith(["first", "secret", "image", "last"]),
    );
    expect(copyItems).toHaveBeenCalledTimes(1);
    // The 1+N path: a whole extra daemon copy and clipboard write per bulk copy.
    expect(copyItem).not.toHaveBeenCalled();
    expect(writeText).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: /selection actions/i }),
      ).toBeNull(),
    );
  });

  it("leaves a single selected row to the daemon copy", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 1);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /^copy$/i }));

    await waitFor(() => expect(copyItem).toHaveBeenCalledTimes(1));
    expect(copyItems).not.toHaveBeenCalled();
  });

  /** The race the whole change exists for: a row committed by the deferred
   *  delete between the selection and the copy. The backend writes nothing, so
   *  the view must not claim it copied anything. */
  it("says nothing was copied when a selected row has gone", async () => {
    listItems.mockResolvedValue(mixed());
    copyItems.mockRejectedValueOnce(new Error("no such item"));

    const { copy } = await copySelection(2);

    await waitFor(() => expect(copyItems).toHaveBeenCalledTimes(1));
    await waitFor(() => expect((copy as HTMLButtonElement).disabled).toBe(false));
    expect(toastSuccess).not.toHaveBeenCalled();
    expect(toastError).toHaveBeenCalled();
    // Still selectable, so the user can retry against what is actually there.
    expect(
      screen.getByRole("region", { name: /selection actions/i }),
    ).toBeTruthy();
  });

  it("reports a partial rather than a plain success when rows were left out", async () => {
    listItems.mockResolvedValue(mixed());
    // Two of the four are sensitive or an image, so the backend copies two.
    copyItems.mockResolvedValueOnce(2);

    await copySelection(4);

    await waitFor(() =>
      expect(toastWarning).toHaveBeenCalledWith(
        expect.stringContaining("Copied 2 of 4"),
      ),
    );
    expect(toastSuccess).not.toHaveBeenCalled();
  });

  it("does not report a copy when every selected row was withheld", async () => {
    listItems.mockResolvedValue(mixed());
    copyItems.mockResolvedValueOnce(0);

    await copySelection(2);

    await waitFor(() =>
      expect(toastError).toHaveBeenCalledWith(
        expect.stringContaining("Nothing was copied"),
      ),
    );
    expect(toastSuccess).not.toHaveBeenCalled();
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

/** The Toaster is not mounted under test, so the undo control is read off the
 *  toast call rather than the DOM — as in useDeferredDelete.test.tsx. */
function undoAction(): { label: string; onClick: () => void } {
  const call = [...toastSuccess.mock.calls]
    .reverse()
    .find(([, options]) => (options as { action?: { label?: string } })?.action?.label === "Undo");
  if (call === undefined) throw new Error("no toast offered an Undo action");
  return (call[1] as { action: { label: string; onClick: () => void } }).action;
}

describe("bulk delete", () => {
  /** DMY-168: the dialog is no longer the only gate, but it stays — the window
   *  is five seconds and the blast radius is the whole selection. */
  it("asks before deleting, and offers an undo", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /delete/i }));

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Delete 2 items?");
    expect(dialog.textContent).toContain("undo it");
    expect(deleteItem).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: /^delete$/i }));

    // The rows go at once; the delete itself does not.
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(2));
    expect(deleteItem).not.toHaveBeenCalled();
    expect(undoAction()).toBeTruthy();
  });

  /** The whole point of DMY-168: pressing undo means the rows were never
   *  deleted, because a committed delete NULLs the ciphertext and cannot be
   *  reconstructed. */
  it("restores the selection and never calls the delete when undone", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await selectRows(user, 2);

    const bar = screen.getByRole("region", { name: /selection actions/i });
    await user.click(within(bar).getByRole("button", { name: /delete/i }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: /^delete$/i }));
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(2));

    await act(async () => undoAction().onClick());

    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    expect(deleteItem).not.toHaveBeenCalled();
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

    await user.click(screen.getByRole("combobox", { name: /filter by kind/i }));
    await user.click(await screen.findByRole("option", { name: "Links" }));
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(1));
    // AT-68: the badge switches to the visible count the moment a filter is on.
    expect(screen.getByText("1 item")).toBeTruthy();
  });

  it("says the filter matched nothing rather than that nothing was copied", async () => {
    const { user } = withUser(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));

    await user.click(screen.getByRole("combobox", { name: /filter by kind/i }));
    await user.click(await screen.findByRole("option", { name: "Colors" }));
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
