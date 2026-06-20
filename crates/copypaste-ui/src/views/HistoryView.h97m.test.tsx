/**
 * CopyPaste-h97m — HistoryView must refresh after a successful backup import.
 * The refresh is triggered via a "history-refresh" Tauri event emitted by
 * SettingsView when api.importItems() succeeds.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Tauri mocks — must be set up BEFORE importing HistoryView.
// ---------------------------------------------------------------------------
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

// Controllable listen/emit mock so we can fire events from tests.
const listenHandlers: Map<string, ((event: { payload: unknown }) => void)[]> = new Map();
const emitMock = vi.fn(async (eventName: string, payload: unknown) => {
  const handlers = listenHandlers.get(eventName) ?? [];
  for (const h of handlers) h({ payload });
});
const listenMock = vi.fn(async (eventName: string, handler: (event: { payload: unknown }) => void) => {
  if (!listenHandlers.has(eventName)) listenHandlers.set(eventName, []);
  listenHandlers.get(eventName)!.push(handler);
  return () => {
    const arr = listenHandlers.get(eventName) ?? [];
    const idx = arr.indexOf(handler);
    if (idx >= 0) arr.splice(idx, 1);
  };
});
vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: Parameters<typeof emitMock>) => emitMock(...args),
  listen: (...args: Parameters<typeof listenMock>) => listenMock(...args),
}));

import { HistoryView } from "./HistoryView";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

function ipcOk(data: unknown) {
  return { ok: true, data, error: null, error_code: null };
}

// ---------------------------------------------------------------------------
// Setup / teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  invoke.mockReset();
  listenHandlers.clear();
  emitMock.mockClear();
  listenMock.mockClear();
  useUI.setState((s) => ({ prefs: { ...s.prefs, skin: "classic" } }));
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// CopyPaste-h97m: HistoryView refreshes on "history-refresh" event
// ---------------------------------------------------------------------------

describe("CopyPaste-h97m: HistoryView refreshes after backup import", () => {
  it("registers a listener for 'history-refresh' event on mount", async () => {
    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        return Promise.resolve(ipcOk({ items: [makeEntry("x")], total: 1, own_device_id: "dev-1" }));
      }
      if (args?.method === "get_private_mode") {
        return Promise.resolve(ipcOk({ private_mode: false }));
      }
      if (args?.method === "status") {
        return Promise.resolve(ipcOk({ status: "running", ready: true, degraded: false }));
      }
      if (args?.method === "search") {
        return Promise.resolve(ipcOk({ items: [] }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item x")).toBeInTheDocument());

    // HistoryView must have registered a listener for "history-refresh".
    const registeredEvents = listenMock.mock.calls.map(([name]) => name as string);
    expect(registeredEvents).toContain("history-refresh");
  });

  it("re-fetches history_page when 'history-refresh' event fires", async () => {
    const firstItems = [makeEntry("first")];
    const secondItems = [makeEntry("first"), makeEntry("imported")];
    let callCount = 0;

    invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
      if (args?.method === "history_page") {
        callCount++;
        const returnItems = callCount <= 2 ? firstItems : secondItems;
        return Promise.resolve(ipcOk({ items: returnItems, total: returnItems.length, own_device_id: "dev-1" }));
      }
      if (args?.method === "get_private_mode") {
        return Promise.resolve(ipcOk({ private_mode: false }));
      }
      if (args?.method === "status") {
        return Promise.resolve(ipcOk({ status: "running", ready: true, degraded: false }));
      }
      if (args?.method === "search") {
        return Promise.resolve(ipcOk({ items: [] }));
      }
      return Promise.reject("daemon_offline:/tmp/x.sock");
    });

    render(<HistoryView />);

    await waitFor(() => expect(screen.getByText("Item first")).toBeInTheDocument());

    const historyCallsBefore = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "history_page",
    ).length;

    // Fire the history-refresh event to simulate SettingsView's post-import emit.
    await act(async () => {
      await emitMock("history-refresh", {});
    });

    // history_page should have been called at least once more after the event.
    await waitFor(() => {
      const historyCallsAfter = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "history_page",
      ).length;
      expect(historyCallsAfter).toBeGreaterThan(historyCallsBefore);
    });
  });
});
