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
import { type ErrorKind, IpcFailure } from "@/lib/errors";
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

  it("shows the paused empty state when status reports private mode", async () => {
    listItems.mockResolvedValue(page([]));
    getStatus.mockResolvedValue(status({ private_mode: true }));
    withClient(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText("Private mode is on")).toBeTruthy(),
    );
    expect(
      screen.getByText(
        "Clipboard is not recorded while private mode is active.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Nothing copied yet")).toBeNull();
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

/**
 * The regression this file exists to stop reappearing: an unrecoverable state
 * rendered with the affordance of a transient one. Both of these used to land
 * on the generic `Failed to load history` screen and its **Try again**, which
 * retried forever against a condition retrying cannot fix (backlog B-4,
 * ui-parity finding 2).
 */
describe("states no retry can clear", () => {
  const failWith = (kind: ErrorKind) => {
    listItems.mockRejectedValue(new IpcFailure(kind));
    getStatus.mockRejectedValue(new IpcFailure(kind));
  };

  it("says a v0.4 history is a v0.4 history, and that it is untouched", async () => {
    failWith("legacy_database");
    withClient(<HistoryView />);

    const title = await screen.findByText(/CopyPaste 0\.4 clipboard history/i);
    expect(title).toBeTruthy();
    // The reassuring half is the half the user cannot work out alone.
    expect(document.body.textContent).toMatch(/still on this device/i);
    expect(screen.queryByText("Failed to load history")).toBeNull();
  });

  it("admits there is nothing to do about an unusable key", async () => {
    failWith("key_unusable");
    withClient(<HistoryView />);

    await screen.findByText(/can't be unlocked/i);
    expect(document.body.textContent).toMatch(/won't change that/i);
  });

  it.each(["legacy_database", "key_unusable"] as const)(
    "offers no retry for %s",
    async (kind) => {
      failWith(kind);
      withClient(<HistoryView />);

      await waitFor(() =>
        expect(screen.queryByText("Loading…")).toBeNull(),
      );
      for (const button of screen.queryAllByRole("button")) {
        expect(button.textContent ?? "").not.toMatch(/try again|retry|restart/i);
      }
    },
  );

  /** The gate must not swallow the ordinary case: a transient fault is still
   *  worth another go, and removing the retry from everything would be the
   *  same defect pointed the other way. */
  it("still offers a retry for an ordinary failure", async () => {
    failWith("internal");
    withClient(<HistoryView />);

    await screen.findByText("Failed to load history");
    expect(screen.getByRole("button", { name: /try again/i })).toBeTruthy();
  });

  /** The counterpart, and the reason the two key-store failures are separate
   *  codes: this one is worth asking again. */
  it("does offer a retry when the key store is merely locked", async () => {
    failWith("key_locked");
    withClient(<HistoryView />);

    await screen.findByText(/Waiting for the key store/i);
    expect(screen.getByRole("button", { name: /try again/i })).toBeTruthy();
  });

  it.each(["legacy_database", "key_unusable", "key_locked"] as const)(
    "names no path for %s",
    async (kind) => {
      failWith(kind);
      const { container } = withClient(<HistoryView />);
      await waitFor(() =>
        expect(screen.queryByText("Loading…")).toBeNull(),
      );
      expect(container.innerHTML).not.toMatch(
        /\/Users\/|\/home\/|\.sock|\.db\b|~\//,
      );
    },
  );
});
