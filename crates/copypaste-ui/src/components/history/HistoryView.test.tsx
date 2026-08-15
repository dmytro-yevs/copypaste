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
import { usePrefs } from "@/store/prefs";
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
  useUi.setState({ query: "", activeId: null, settingsTab: null });
  usePrefs.getState().reset();
});

afterEach(() => vi.restoreAllMocks());

describe("the service is not running", () => {
  it("explains that browser preview has no native service instead of showing empty history", async () => {
    listItems.mockRejectedValue(new IpcFailure("offline", true));
    withClient(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText("This is the web preview")).toBeTruthy(),
    );
    expect(screen.queryByRole("button", { name: /start the service/i })).toBeNull();
    expect(screen.queryByText("Nothing copied yet")).toBeNull();
  });

  it("never names a path in what it shows", async () => {
    listItems.mockRejectedValue(
      "connection refused on /Users/dmitriy/Library/CopyPaste/daemon.sock",
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { container } = withClient(<HistoryView />);

    await waitFor(() =>
      expect(screen.getByText(/clipboard service returned an error/i)).toBeTruthy(),
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
        "Clipboard items are not recorded while private mode is active.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("Nothing copied yet")).toBeNull();
  });

  it("turns an empty paused capture into an actionable capture state", async () => {
    listItems.mockResolvedValue(page([]));
    getStatus.mockResolvedValue(status({ capture_running: false }));
    const { user } = withUser(<HistoryView />);

    expect(await screen.findByText("Clipboard capture is paused")).toBeTruthy();
    expect(
      screen.getByText(
        "New clipboard items will appear here after capture is set up again.",
      ),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Set up capture" }));
    expect(useUi.getState().view).toBe("capture");
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

describe("binding search, device, and display preferences", () => {
  const from = (id: string, name: string, content: string) =>
    ({
      ...items(1)[0]!,
      id: `${id}-${content}`,
      content,
      origin_device_id: id,
      origin_device_name: name,
    }) as ReturnType<typeof items>[number];

  it("keeps fuzzy matches when service FTS fails", async () => {
    listItems.mockResolvedValue(page([from("mac", "Mac", "alpha clipboard")]));
    searchItems.mockRejectedValue(new Error("fts unavailable"));
    useUi.setState({ query: "alp clip" });
    withClient(<HistoryView />);

    expect(await screen.findByText("alpha clipboard")).toBeTruthy();
  });

  it("adds an FTS-only hit after client matches without duplicating either", async () => {
    listItems.mockResolvedValue(page([from("mac", "Mac", "client needle")]));
    searchItems.mockResolvedValue(
      page([
        from("mac", "Mac", "client needle"),
        from("phone", "Phone", "stemming-only result"),
      ]),
    );
    useUi.setState({ query: "needle" });
    withClient(<HistoryView />);

    expect(await screen.findByText("client needle")).toBeTruthy();
    expect(await screen.findByText("stemming-only result")).toBeTruthy();
    expect(screen.getAllByText("client needle")).toHaveLength(1);
  });

  it("filters by device and persists grouped headings from the toolbar", async () => {
    listItems.mockResolvedValue(
      page([
        from("mac-id", "Mac", "from mac"),
        from("phone-id", "Phone", "from phone"),
      ]),
    );
    const { user } = withUser(<HistoryView />);

    const filter = await screen.findByRole("combobox", {
      name: "Filter by device",
    });
    await user.selectOptions(filter, "phone-id");
    expect(screen.queryByText("from mac")).toBeNull();
    expect(screen.getByText("from phone")).toBeTruthy();

    await user.selectOptions(filter, "all");
    await user.click(
      screen.getByRole("button", {
        name: "Group clipboard items by device",
      }),
    );
    expect(usePrefs.getState().sortByDevice).toBe(true);
    expect(screen.getByRole("heading", { name: "Mac" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Phone" })).toBeTruthy();

    useUi.getState().setQuery("from");
    await waitFor(() =>
      expect(screen.queryByRole("heading", { name: "Mac" })).toBeNull(),
    );
    expect(usePrefs.getState().sortByDevice).toBe(true);
  });

  it("caps rendering after filtering and announces the hidden remainder", async () => {
    usePrefs.getState().set("historyDisplayLimit", 100);
    listItems.mockResolvedValue(page(items(120)));
    withClient(<HistoryView />);

    expect(
      await screen.findByText(
        "Showing first 100 of 120 results — adjust the display limit in Settings › Storage.",
      ),
    ).toBeTruthy();
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
    // 5j9x / kayk / fjvz / vcnv / w6xc are all the same defect, a destructive
    // action one misclick away. DMY-168 added an undo window behind this, but
    // the prompt stays: it is what names the blast radius before the misclick.
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
    expect(dialog.textContent).toContain("undo it");
  });
});

/**
 * The regression this file exists to stop reappearing: an unrecoverable state
 * explained the recovery poorly. Every failure now offers a recheck plus a
 * safe diagnostic report, so support does not have to reconstruct a failure
 * from a screenshot.
 */
describe("history failure recovery", () => {
  const failWith = (kind: ErrorKind, retryable = false) => {
    listItems.mockRejectedValue(new IpcFailure(kind, retryable));
    getStatus.mockRejectedValue(new IpcFailure(kind, retryable));
  };

  it("admits there is nothing to do about an unusable key", async () => {
    failWith("key_unusable");
    withClient(<HistoryView />);

    await screen.findByText(/can't be unlocked/i);
    expect(document.body.textContent).toMatch(/won't change that/i);
  });

  it("offers a recheck and diagnostics for an unusable key", async () => {
    failWith("key_unusable");
    withClient(<HistoryView />);

    expect(await screen.findByRole("button", { name: /try again/i })).toBeTruthy();
    expect(screen.getByRole("button", { name: /export support bundle/i })).toBeTruthy();
  });

  /** The gate must not swallow the ordinary case: a transient fault is still
   *  worth another go, and removing the retry from everything would be the
   *  same defect pointed the other way. */
  it("still offers a retry for an ordinary failure", async () => {
    failWith("internal", true);
    withClient(<HistoryView />);

    await screen.findByText("Failed to load history");
    expect(screen.getByRole("button", { name: /try again/i })).toBeTruthy();
  });

  /** The counterpart, and the reason the two key-store failures are separate
   *  codes: this one is worth asking again. */
  it("does offer a retry when the key store is merely locked", async () => {
    failWith("key_locked", true);
    withClient(<HistoryView />);

    await screen.findByText(/Waiting for the key store/i);
    expect(screen.getByRole("button", { name: /try again/i })).toBeTruthy();
  });

  it.each([
    ["key_unusable", false],
    ["key_locked", true],
  ] as const)(
    "names no path for %s",
    async (kind, retryable) => {
      failWith(kind, retryable);
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
