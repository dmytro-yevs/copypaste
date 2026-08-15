/**
 * The invariant: the pushed history entry exists exactly while the compact
 * subpage is on screen. An entry that outlives its level is not inert — the
 * next system Back is spent traversing it, so Back appears to do nothing
 * (DMY-169 review).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useSettingsLevel } from "@/components/settings/useSettingsLevel";

const close = vi.fn();
let pushState: ReturnType<typeof vi.spyOn>;
let back: ReturnType<typeof vi.spyOn>;

function pop() {
  act(() => {
    window.dispatchEvent(new PopStateEvent("popstate"));
  });
}

beforeEach(() => {
  close.mockReset();
  pushState = vi.spyOn(window.history, "pushState");
  // Stubbed, not merely watched: jsdom traverses asynchronously, and a real
  // traversal would deliver its `popstate` into whichever test ran next.
  back = vi.spyOn(window.history, "back").mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the compact settings level", () => {
  it("pushes one entry when the level opens", () => {
    const { rerender } = renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });
    expect(pushState).not.toHaveBeenCalled();

    rerender({ open: true });

    expect(pushState).toHaveBeenCalledTimes(1);
    expect(pushState.mock.calls[0]![1]).toBe("");
    expect(pushState.mock.calls[0]).toHaveLength(2);
  });

  it("closes the level when the system Back pops the entry", () => {
    const { rerender } = renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });
    rerender({ open: true });

    pop();

    expect(close).toHaveBeenCalledTimes(1);
    expect(back).not.toHaveBeenCalled();
  });

  it("spends the entry exactly once when the Back control is used", () => {
    const { result, rerender } = renderHook(
      ({ open }) => useSettingsLevel(open, close),
      { initialProps: { open: false } },
    );
    rerender({ open: true });

    act(() => result.current());
    // The traversal is what closes the level, so the caller has not yet.
    expect(back).toHaveBeenCalledTimes(1);
    pop();
    rerender({ open: false });

    expect(close).toHaveBeenCalledTimes(1);
    expect(back).toHaveBeenCalledTimes(1);
  });

  /**
   * A rotation across the size boundary, an unfold, or a desktop window resize.
   * No `popstate` fires on this path, so nothing spends the entry unless the
   * level itself does.
   */
  it("spends the entry when the size class takes the level away", () => {
    const { rerender } = renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });
    rerender({ open: true });

    rerender({ open: false });

    expect(back).toHaveBeenCalledTimes(1);
    close.mockClear();
    pop();
    expect(close).not.toHaveBeenCalled();
  });

  it("spends the entry when settings is left with a subpage open", () => {
    const { rerender, unmount } = renderHook(
      ({ open }) => useSettingsLevel(open, close),
      { initialProps: { open: false } },
    );
    rerender({ open: true });

    unmount();

    expect(back).toHaveBeenCalledTimes(1);
  });

  it("pushes a fresh entry when the level comes back", () => {
    const { rerender } = renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });
    rerender({ open: true });
    rerender({ open: false });

    rerender({ open: true });

    expect(pushState).toHaveBeenCalledTimes(2);
  });

  it("ignores a pop of an entry it never pushed", () => {
    renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });

    pop();

    expect(close).not.toHaveBeenCalled();
  });

  it("falls back to closing directly when no entry was pushed", () => {
    const { result } = renderHook(({ open }) => useSettingsLevel(open, close), {
      initialProps: { open: false },
    });

    act(() => result.current());

    expect(close).toHaveBeenCalledTimes(1);
    expect(back).not.toHaveBeenCalled();
  });
});
