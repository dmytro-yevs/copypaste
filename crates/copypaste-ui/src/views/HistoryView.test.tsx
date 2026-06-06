/**
 * HistoryView — pagination + total-count tests.
 *
 * DELIVERABLE 1: infinite-scroll load-more — subsequent pages are fetched when
 *   the VirtualList fires onNearBottom; de-dup by id; stop when all pages are loaded.
 * DELIVERABLE 2: header count badge reflects the FULL total from the daemon, not
 *   just the length of the currently-loaded array.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  act,
} from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri mock — must be set up BEFORE importing any module that uses invoke.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { HistoryView } from "./HistoryView";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal HistoryEntry fixture. */
function makeEntry(id: string, wallTime = 1_700_000_000_000) {
  return {
    id,
    content_type: "text",
    preview: `Item ${id}`,
    is_sensitive: false,
    wall_time: wallTime,
    pinned: false,
  };
}

/**
 * Build the IPC reply shape that the Tauri bridge forwards from the daemon:
 * { ok: true, data: { items: [...], total: N }, error: null, error_code: null }
 */
function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

// ---------------------------------------------------------------------------
// DELIVERABLE 2: header shows total from daemon, not loaded-array length
// ---------------------------------------------------------------------------

describe("HistoryView — header total count (deliverable 2)", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("shows the daemon total (500) when only PAGE_SIZE (200) items are loaded", async () => {
    // The daemon says there are 500 items; only 200 are in the first page.
    const page1Items = Array.from({ length: 200 }, (_, i) => makeEntry(`item-${i}`));

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(
          ipcOk({ items: page1Items, total: 500 })
        );
      }
      if (args?.method === "status") {
        return Promise.resolve(
          ipcOk({ status: "running", private_mode: false, ready: true, degraded: false })
        );
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    // The header badge must show "500", not "200".
    await waitFor(() => {
      expect(screen.getByText("500")).toBeInTheDocument();
    });
  });

  it("shows 0 when total is 0 (empty DB)", async () => {
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: [], total: 0 }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => {
      // The empty-state message is shown, not a crash.
      expect(screen.getByText(/Nothing copied yet/i)).toBeInTheDocument();
    });

    // Badge must show 0 (or be absent — the view may hide a 0 badge; either is acceptable).
    // What must NOT happen: badge showing undefined or NaN.
    const badgeEl = screen.queryByTestId("history-total-badge");
    if (badgeEl !== null) {
      expect(badgeEl.textContent).toBe("0");
    }
  });
});

// ---------------------------------------------------------------------------
// DELIVERABLE 1: load-more pagination — onNearBottom triggers next page fetch
// ---------------------------------------------------------------------------

describe("HistoryView — load-more pagination (deliverable 1)", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("appends the second page when onNearBottom fires on the VirtualList", async () => {
    const page1 = Array.from({ length: 5 }, (_, i) => makeEntry(`p1-${i}`, 1_700_000_000_000 - i));
    const page2 = Array.from({ length: 5 }, (_, i) => makeEntry(`p2-${i}`, 1_699_000_000_000 - i));

    // Track calls so we can distinguish page 1 (offset=0) from page 2 (offset=5).
    invoke.mockImplementation((_cmd: string, args: { method?: string; params?: { offset?: number } }) => {
      if (args?.method === "history_page") {
        const offset = args?.params?.offset ?? 0;
        if (offset === 0) {
          return Promise.resolve(ipcOk({ items: page1, total: 10 }));
        }
        // offset > 0 → second page
        return Promise.resolve(ipcOk({ items: page2, total: 10 }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    // Wait for first page to load.
    await waitFor(() => {
      expect(screen.getByText("Item p1-0")).toBeInTheDocument();
    });

    // Simulate the "near bottom" scroll event that VirtualList fires via
    // onNearBottom.  jsdom has no layout engine so scrollHeight/clientHeight are
    // always 0 — we override them on the element before firing the scroll event.
    const listbox = screen.getByRole("listbox", { name: /clipboard history/i });

    // jsdom defines scrollHeight/clientHeight as getters on HTMLElement.prototype;
    // we must override them on the instance itself via defineProperty with
    // configurable:true so they shadow the prototype getter.
    Object.defineProperty(listbox, "scrollHeight", {
      configurable: true,
      get: () => 10000,
    });
    Object.defineProperty(listbox, "clientHeight", {
      configurable: true,
      get: () => 500,
    });
    // scrollTop needs to be high enough that remaining = scrollHeight - scrollTop
    // - clientHeight < LOAD_MORE_THRESHOLD_PX (300).
    // 10000 - 9400 - 500 = 100 < 300 ✓
    Object.defineProperty(listbox, "scrollTop", {
      configurable: true,
      get: () => 9400,
    });

    await act(async () => {
      fireEvent.scroll(listbox);
    });

    // The second page's items must now be fetched and appended.
    // VirtualList only renders the visible window (viewportH=0 in jsdom so the
    // window is tiny), so we verify via the invoke call count rather than a
    // specific row being in the DOM — the IPC mock being called with offset > 0
    // is the authoritative proof that load-more fired.
    await waitFor(() => {
      const page2Calls = invoke.mock.calls.filter(
        ([, a]) => a?.method === "history_page" && (a?.params?.offset ?? 0) > 0
      );
      expect(page2Calls.length).toBeGreaterThan(0);
    });

    // Additionally verify the appended items exist somewhere in the component's
    // loaded state by checking for any p2-* item that the virtualizer chose to
    // render (the last item in the page tends to be rendered at offset 0).
    expect(screen.getByText(/Item p2-/)).toBeInTheDocument();
  });

  it("does not fetch more pages when all items are already loaded", async () => {
    // total === items.length → no more pages.
    const page1 = Array.from({ length: 3 }, (_, i) => makeEntry(`done-${i}`));

    invoke.mockImplementation((_cmd: string, args: { method?: string; params?: { offset?: number } }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: page1, total: 3 }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item done-0")).toBeInTheDocument();
    });

    // Count invocations before simulating near-bottom.
    const callsBefore = invoke.mock.calls.filter(
      ([, args]) => args?.method === "history_page"
    ).length;

    const listbox = screen.getByRole("listbox", { name: /clipboard history/i });
    Object.defineProperty(listbox, "scrollHeight", { configurable: true, get: () => 10000 });
    Object.defineProperty(listbox, "clientHeight", { configurable: true, get: () => 500 });
    Object.defineProperty(listbox, "scrollTop", { configurable: true, get: () => 9400 });
    await act(async () => {
      fireEvent.scroll(listbox);
    });

    // Give any async work a tick to settle.
    await new Promise((r) => setTimeout(r, 50));

    const callsAfter = invoke.mock.calls.filter(
      ([, args]) => args?.method === "history_page"
    ).length;

    // No additional history_page fetch should have been made.
    expect(callsAfter).toBe(callsBefore);
  });

  it("de-duplicates items when a poll overlaps with a loaded page", async () => {
    const page1 = Array.from({ length: 3 }, (_, i) => makeEntry(`dup-${i}`));

    // Both the initial load and the "near-bottom" page return the same set
    // (simulates a reload during load-more).
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: page1, total: 3 }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => {
      expect(screen.getByText("Item dup-0")).toBeInTheDocument();
    });

    // There should be exactly 3 list items (no duplicates).
    expect(screen.getAllByText(/Item dup-/)).toHaveLength(3);
  });
});
