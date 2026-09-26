import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { useSettingsLevel } from "./useSettingsLevel";

afterEach(() => vi.restoreAllMocks());

it("consumes nested history one level at a time for system and visible Back", () => {
  const push = vi.spyOn(window.history, "pushState").mockImplementation(() => {});
  const back = vi.spyOn(window.history, "back").mockImplementation(() => {
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
  const onBack = vi.fn();
  const { result, rerender, unmount } = renderHook(
    ({ depth }) => useSettingsLevel(depth, onBack),
    { initialProps: { depth: 0 } },
  );

  rerender({ depth: 2 });
  expect(push).toHaveBeenCalledTimes(2);
  act(() => window.dispatchEvent(new PopStateEvent("popstate")));
  expect(onBack).toHaveBeenCalledTimes(1);
  rerender({ depth: 1 });
  act(() => result.current());
  expect(back).toHaveBeenCalledTimes(1);
  expect(onBack).toHaveBeenCalledTimes(2);
  rerender({ depth: 0 });
  unmount();
});

it("removes unspent entries on breakpoint change and on leaving Settings", () => {
  vi.spyOn(window.history, "pushState").mockImplementation(() => {});
  const go = vi.spyOn(window.history, "go").mockImplementation(() => {});
  const onBack = vi.fn();
  const { rerender, unmount } = renderHook(
    ({ depth }) => useSettingsLevel(depth, onBack),
    { initialProps: { depth: 2 } },
  );

  rerender({ depth: 0 });
  expect(go).toHaveBeenCalledWith(-2);
  act(() => window.dispatchEvent(new PopStateEvent("popstate")));
  expect(onBack).not.toHaveBeenCalled();
  rerender({ depth: 2 });
  unmount();
  expect(go).toHaveBeenLastCalledWith(-2);
  expect(go).toHaveBeenCalledTimes(2);
});
