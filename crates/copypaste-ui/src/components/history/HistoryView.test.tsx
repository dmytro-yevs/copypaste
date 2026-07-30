/**
 * The History screen's states (manifest §3.1.11) and its count badge (AT-68).
 *
 * The one that matters most: when the service is not running the screen is an
 * **offer to start it**, not an empty list. An empty list says "you have copied
 * nothing", which is false, and it is the neighbour of the defect bdac.2
 * records.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { HistoryView } from "@/components/history/HistoryView";
import { historyCount } from "@/components/history/SearchBar";
import { IpcFailure } from "@/lib/errors";
import { items, page, status, withClient, withUser } from "@/test/harness";
import { useUi } from "@/store/ui";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...a: unknown[]) => listItems(...a),
    searchItems: (...a: unknown[]) => searchItems(...a),
    getStatus: () => getStatus(),
  };
});

beforeEach(() => {
  listItems.mockReset();
  searchItems.mockReset();
  getStatus.mockReset().mockResolvedValue(status());
  useUi.setState({ query: "", activeId: null });
});

afterEach(() => vi.restoreAllMocks());

describe("the service is not running", () => {
  it("hands over to the start-the-service screen instead of an empty list", async () => {
    listItems.mockRejectedValue(new IpcFailure("offline"));
    withClient(<HistoryView />);

    // Which variant of the offer is shown depends on whether the bridge is
    // there to start it — see ServiceOffline.test.tsx. Either way it is an
    // offer with an action, and emphatically not "Nothing copied yet".
    await waitFor(() =>
      expect(screen.getByText(/clipboard service/i)).toBeTruthy(),
    );
    expect(screen.getByRole("button", { name: /check again/i })).toBeTruthy();
    expect(screen.queryByText("Nothing copied yet")).toBeNull();
  });

  it("never names a path in what it shows", async () => {
    listItems.mockRejectedValue(
      "connection refused on /Users/dmitriy/Library/CopyPaste/daemon.sock",
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { container } = withClient(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/clipboard service/i)).toBeTruthy(),
    );
    expect(container.innerHTML).not.toMatch(/\/Users\/|dmitriy|\.sock/);
  });
});

describe("empty and filtered states", () => {
  it("says nothing has been copied when the list is genuinely empty", async () => {
    listItems.mockResolvedValue(page([]));
    withClient(<HistoryView />);
    await waitFor(() => expect(screen.getByText("Nothing copied yet")).toBeTruthy());
  });

  it("reports a search with no matches as a search result, not as empty", async () => {
    listItems.mockResolvedValue(page([]));
    searchItems.mockResolvedValue(page([]));
    useUi.setState({ query: "needle" });
    withClient(<HistoryView />);
    await waitFor(() =>
      expect(screen.getByText('No results for "needle"')).toBeTruthy(),
    );
  });

  it("renders rows when there are some", async () => {
    listItems.mockResolvedValue(page(items(3)));
    withClient(<HistoryView />);
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(3));
  });
});

describe("the count badge follows the filter (AT-68)", () => {
  it("shows the service total when nothing is filtered", () => {
    expect(historyCount(false, 3, 14)).toBe("14 items");
  });

  it("shows the visible count as soon as a filter is active", () => {
    // 14 total and a query matching none must read "0 items", not "14 items"
    // (CopyPaste-g27b.37).
    expect(historyCount(true, 0, 14)).toBe("0 items");
    expect(historyCount(true, 1, 14)).toBe("1 item");
  });

  it("falls back to what is on screen before the total arrives", () => {
    expect(historyCount(false, 2, undefined)).toBe("2 items");
  });
});

describe("clear all", () => {
  it("is offered when there is something to clear", async () => {
    listItems.mockResolvedValue(page(items(2)));
    withClient(<HistoryView />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /clear clipboard history/i }),
      ).toBeTruthy(),
    );
  });

  it("is not offered when there is nothing to clear", async () => {
    listItems.mockResolvedValue(page([]));
    withClient(<HistoryView />);
    await waitFor(() => expect(screen.getByText("Nothing copied yet")).toBeTruthy());
    expect(
      screen.queryByRole("button", { name: /clear clipboard history/i }),
    ).toBeNull();
  });

  it("asks first, and the prompt names what is lost and what is kept", async () => {
    // Destructive and un-undoable: 5j9x / kayk / fjvz / vcnv / w6xc are all
    // the same defect, a destructive action one misclick away.
    listItems.mockResolvedValue(page(items(2)));
    const { user } = withUser(<HistoryView />);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /clear clipboard history/i }),
      ).toBeTruthy(),
    );
    await user.click(
      screen.getByRole("button", { name: /clear clipboard history/i }),
    );

    const dialog = await screen.findByRole("alertdialog");
    expect(dialog.textContent).toContain("Clear all clipboard history?");
    expect(dialog.textContent).toContain("Pinned items are kept");
    expect(dialog.textContent).toContain("cannot be undone");
  });
});
