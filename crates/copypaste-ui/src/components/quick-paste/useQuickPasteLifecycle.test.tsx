import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FocusEvent, ReactNode, RefObject } from "react";

import {
  QUICK_PASTE_QUERY_KEY,
  useQuickPasteLifecycle,
} from "@/components/quick-paste/useQuickPasteLifecycle";
import { DEFAULT_PREFS, STORAGE_KEY } from "@/store/prefs";
import { testClient } from "@/test/harness";

const hideWindow = vi.fn();
const setAllowScreenshots = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    hideWindow: () => hideWindow(),
    setAllowScreenshots: (...args: unknown[]) => setAllowScreenshots(...args),
  };
});

function renderLifecycle() {
  const client = testClient();
  const cancelQueries = vi.spyOn(client, "cancelQueries").mockResolvedValue(undefined);
  const refetchQueries = vi.spyOn(client, "refetchQueries").mockResolvedValue(undefined);
  const removeQueries = vi.spyOn(client, "removeQueries");
  const clearLocalState = vi.fn();
  const searchRef = {
    current: document.createElement("input"),
  } satisfies RefObject<HTMLInputElement>;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const rendered = renderHook(
    () => useQuickPasteLifecycle({ searchRef, clearLocalState }),
    { wrapper },
  );
  return {
    ...rendered,
    cancelQueries,
    clearLocalState,
    refetchQueries,
    removeQueries,
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  window.localStorage.clear();
  hideWindow.mockReset().mockResolvedValue(undefined);
  setAllowScreenshots.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useQuickPasteLifecycle", () => {
  it("re-reads preferences and refreshes the held query on focus", () => {
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const { result, cancelQueries, refetchQueries } = renderLifecycle();

    expect(result.current.previewLinesPopup).toBe(DEFAULT_PREFS.previewLinesPopup);
    expect(refetchQueries).not.toHaveBeenCalled();
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        state: { ...DEFAULT_PREFS, previewLines: 4, previewLinesPopup: 2 },
        version: 0,
      }),
    );

    act(() => window.dispatchEvent(new Event("focus")));

    expect(result.current.previewLinesPopup).toBe(2);
    expect(result.current.historyPreviewLines).toBe(4);
    expect(cancelQueries).toHaveBeenCalledWith({ queryKey: QUICK_PASTE_QUERY_KEY });
    expect(refetchQueries).toHaveBeenCalledWith({ queryKey: QUICK_PASTE_QUERY_KEY });
  });

  it("disables, cancels, and removes the hidden cache before re-enabling it", () => {
    const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const { result, cancelQueries, clearLocalState, refetchQueries, removeQueries } =
      renderLifecycle();
    const generation = result.current.currentCacheGeneration();

    act(() => window.__copypasteFreeMemory?.());

    expect(result.current.holding).toBe(false);
    expect(clearLocalState).toHaveBeenCalledTimes(1);
    expect(cancelQueries).toHaveBeenCalledWith({ queryKey: QUICK_PASTE_QUERY_KEY });
    expect(removeQueries).toHaveBeenCalledWith({ queryKey: QUICK_PASTE_QUERY_KEY });
    expect(result.current.isCacheGenerationCurrent(generation)).toBe(false);

    visibility.mockReturnValue("visible");
    act(() => document.dispatchEvent(new Event("visibilitychange")));
    expect(result.current.holding).toBe(true);
    expect(refetchQueries).not.toHaveBeenCalled();
  });

  it("dismisses only after focus leaves the popup and guards duplicate hides", () => {
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    const { result, clearLocalState } = renderLifecycle();
    const root = document.createElement("main");
    const inside = document.createElement("button");
    const toaster = document.createElement("div");
    const retry = document.createElement("button");
    root.append(inside);
    toaster.dataset.sonnerToaster = "";
    toaster.append(retry);
    document.body.append(root, toaster);

    const blurTo = (relatedTarget: EventTarget | null) =>
      result.current.dismissOnRootBlur({
        currentTarget: root,
        relatedTarget,
      } as FocusEvent<HTMLElement>);

    act(() => blurTo(inside));
    act(() => blurTo(retry));
    expect(hideWindow).not.toHaveBeenCalled();

    act(() => {
      blurTo(document.body);
      blurTo(document.body);
    });
    expect(hideWindow).toHaveBeenCalledTimes(1);
    expect(clearLocalState).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(100));
    act(() => blurTo(document.body));
    expect(hideWindow).toHaveBeenCalledTimes(2);

    root.remove();
    toaster.remove();
  });
});
