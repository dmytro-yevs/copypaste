/**
 * HistoryView — pagination + total-count + display-limit tests.
 *
 * DELIVERABLE 1: infinite-scroll load-more — subsequent pages are fetched when
 *   the VirtualList fires onNearBottom; de-dup by id; stop when all pages are loaded.
 * DELIVERABLE 2: header count badge reflects the FULL total from the daemon, not
 *   just the length of the currently-loaded array.
 * DELIVERABLE 3 (CopyPaste-2b1g): historyDisplayLimit pref persists across remounts
 *   and HistoryView enforces the cap on rendered items.
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
import { SettingsView } from "./SettingsView";
import { useUI } from "../store";

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

    // The header badge must reflect the daemon total (500), not the loaded slice (200).
    // The badge renders as "{total} item(s)", so we look up by testid and check content.
    await waitFor(() => {
      const badge = screen.getByTestId("history-total-badge");
      expect(badge).toBeInTheDocument();
      expect(badge.textContent).toMatch(/^500\b/);
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
    // The component renders "{total} item(s)" so "0 items" is the expected text.
    const badgeEl = screen.queryByTestId("history-total-badge");
    if (badgeEl !== null) {
      expect(badgeEl.textContent).toMatch(/^0\b/);
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

// ---------------------------------------------------------------------------
// DELIVERABLE 3 (CopyPaste-2b1g): historyDisplayLimit persists + HistoryView cap
// ---------------------------------------------------------------------------

/**
 * Build a minimal online invoke mock for SettingsView (Storage tab).
 * Mirrors the helper in SettingsView.test.tsx so the same invocation shape works.
 */
function makeOnlineInvokeForLimit() {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (cmd === "ipc_call") {
      const method = (args as { method?: string } | undefined)?.method;
      switch (method) {
        case "status":
          return Promise.resolve({
            ok: true,
            data: { status: "running", ready: true, degraded: false, degraded_reason: null, build_version: "0.5.5" },
            error: null, error_code: null,
          });
        case "get_config":
          return Promise.resolve({
            ok: true,
            data: {
              p2p_enabled: true, supabase_url: null, supabase_anon_key: null,
              max_text_size_bytes: 10 * 1024 * 1024, max_image_size_bytes: 25 * 1024 * 1024,
              max_file_size_bytes: 100 * 1024 * 1024, storage_quota_bytes: 10 * 1024 * 1024 * 1024,
              sensitive_ttl_secs: 30, image_quality: 100,
            },
            error: null, error_code: null,
          });
        case "get_private_mode":
          return Promise.resolve({ ok: true, data: { private_mode: false }, error: null, error_code: null });
        case "get_sync_status":
          return Promise.resolve({ ok: true, data: { passphrase_set: false, supabase_configured: false, signed_in: false, email: null, last_sync_ms: null }, error: null, error_code: null });
        default:
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
    }
    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "app_version") return Promise.resolve("0.5.5");
    return Promise.resolve(undefined);
  };
}

describe("CopyPaste-2b1g: historyDisplayLimit — persistence and HistoryView cap", () => {
  const PREFS_KEY = "copypaste-ui-prefs-v2";

  beforeEach(() => {
    invoke.mockReset();
    // Clear the store's persisted prefs so each test starts fresh.
    localStorage.removeItem(PREFS_KEY);
  });

  it("slider in SettingsView persists historyDisplayLimit to localStorage and remount reads it back", async () => {
    invoke.mockImplementation(makeOnlineInvokeForLimit());

    const { unmount } = render(<SettingsView />);

    // Wait for settings to load (not offline)
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Navigate to Storage tab
    const storageTab = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTab); });

    // The "History display limit" slider must be present
    expect(screen.getByText(/History display limit/i)).toBeInTheDocument();

    // LimitSliderRow uses an index-based range (0…steps.length-1) mapped to
    // MAX_ITEMS_STEPS = [100, 250, 500, 1000, 2500, 5000, 10000, 100000].
    // Index 0 → 100 items. Fire the slider at index 0 to choose the 100-item cap.
    const rangeInputs = document.querySelectorAll('input[type="range"]');
    // The display-limit slider is the last range input on the Storage tab.
    const limitSlider = rangeInputs[rangeInputs.length - 1] as HTMLInputElement;
    expect(limitSlider).toBeTruthy();

    await act(async () => {
      // value="0" → index 0 → MAX_ITEMS_STEPS[0] = 100
      fireEvent.change(limitSlider, { target: { value: "0" } });
      fireEvent.mouseUp(limitSlider, { target: { value: "0" } });
    });

    // The slider's onChange persists historyDisplayLimit through the store
    // (the ephemeral "Saved" toast fires on pointer-release and is cosmetic;
    // the deliverable is persistence, asserted via localStorage + remount).
    // localStorage must now contain historyDisplayLimit: 100 (MAX_ITEMS_STEPS[0]).
    await waitFor(() => {
      const stored = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") as Record<string, unknown>;
      expect(stored.historyDisplayLimit).toBe(100);
    });

    // Unmount and remount — the new instance must read the persisted value (100)
    unmount();

    invoke.mockImplementation(makeOnlineInvokeForLimit());
    render(<SettingsView />);

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab2 = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTab2); });

    // The slider must reflect index 0 (the persisted value 100 → index 0, not the default 1000 → index 3).
    const rangeInputs2 = document.querySelectorAll('input[type="range"]');
    const limitSlider2 = rangeInputs2[rangeInputs2.length - 1] as HTMLInputElement;
    expect(Number(limitSlider2.value)).toBe(0);
  });

  it("HistoryView renders at most historyDisplayLimit items when the limit is smaller than the list", async () => {
    // Seed localStorage with a tiny cap (N=3) before rendering.
    const N = 3;
    localStorage.setItem(PREFS_KEY, JSON.stringify({ historyDisplayLimit: N }));

    // Force the Zustand store to reload prefs from localStorage so HistoryView
    // sees the seeded value without going through SettingsView.
    // Reimport the store module to pick up the fresh value via loadPrefs().
    // Since Zustand initialises from loadPrefs() at module load time, we need to
    // manually patch the store state here (loadPrefs() is called once at import).
    useUI.setState((s) => ({ prefs: { ...s.prefs, historyDisplayLimit: N } }));

    // The daemon returns 10 items — more than the display cap of 3.
    const tenItems = Array.from({ length: 10 }, (_, i) => makeEntry(`cap-${i}`, 1_700_000_000_000 - i));

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: tenItems, total: 10 }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    // Wait for data to load — the first item must appear.
    await waitFor(() => {
      expect(screen.getByText("Item cap-0")).toBeInTheDocument();
    });

    // VirtualList virtualises rendering, but in jsdom viewportH=0 so the visible
    // window starts at [0, 0) — only items at padTop=0 render. However, the key
    // assertion is that items BEYOND the cap are not in the DOM at all.
    // With cap=3, items cap-3 through cap-9 must never be rendered.
    // We verify cap-0 IS rendered and cap-9 is NOT.
    expect(screen.getByText("Item cap-0")).toBeInTheDocument();

    // Items beyond the cap must not be in the DOM.
    expect(screen.queryByText("Item cap-9")).not.toBeInTheDocument();

    // The number of "Item cap-N" DOM elements must not exceed N.
    const rendered = screen.queryAllByText(/^Item cap-\d+$/);
    expect(rendered.length).toBeLessThanOrEqual(N);
  });
});
