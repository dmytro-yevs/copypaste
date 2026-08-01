/**
 * Every selector in `store/` must produce a snapshot that settles.
 *
 * zustand v5 reads the store through `useSyncExternalStore`, which compares
 * snapshots **by reference**. A selector that builds a fresh object or array
 * per call therefore never settles: React re-renders, re-selects, gets another
 * new object, and stops at "Maximum update depth exceeded" — which unmounts the
 * tree and leaves `#root` empty. The whole app renders nothing.
 *
 * That shipped. `App.tsx` called `usePrefs(selectAppearance)` unwrapped and the
 * page was blank in a real WebView while every jsdom test passed, because
 * nothing here rendered the component that did it.
 *
 * So this file asserts the property directly, over the selectors as a set,
 * rather than testing the one that broke.
 */
import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useShallow } from "zustand/react/shallow";

import { DEFAULT_PREFS, selectAppearance, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

describe("store snapshots settle", () => {
  /**
   * The direct statement of the rule. A selector whose result is `===` across
   * two calls on unchanged state cannot loop; one whose result is not needs
   * `useShallow` at every call site.
   */
  it.each([
    ["ui.view", () => useUi.getState().view],
    ["ui.query", () => useUi.getState().query],
    ["ui.activeId", () => useUi.getState().activeId],
    ["ui.dismissed", () => useUi.getState().dismissed],
    ["ui.setView", () => useUi.getState().setView],
    ["prefs.theme", () => usePrefs.getState().theme],
    ["prefs.previewLines", () => usePrefs.getState().previewLines],
    ["prefs.set", () => usePrefs.getState().set],
  ])("%s is reference-stable across calls", (_name, read) => {
    expect(read()).toBe(read());
  });

  /**
   * And the exception, stated as an exception. `selectAppearance` composes
   * three fields, so it cannot be reference-stable — which is exactly why it
   * must never be used bare.
   */
  it("selectAppearance is not stable on its own", () => {
    const state = usePrefs.getState();
    expect(selectAppearance(state)).not.toBe(selectAppearance(state));
  });

  it("selectAppearance under useShallow does not re-render on an unrelated change", async () => {
    usePrefs.setState({ ...DEFAULT_PREFS });
    let renders = 0;
    const { result } = renderHook(() => {
      renders += 1;
      return usePrefs(useShallow(selectAppearance));
    });

    const before = renders;
    // A field the selector does not read. Without `useShallow` this alone
    // would spin: any store notification hands back a new object.
    await act(async () => {
      usePrefs.setState({ previewLines: 4 });
    });
    expect(renders).toBe(before);
    expect(result.current.theme).toBe(DEFAULT_PREFS.theme);

    // A field it does read still gets through — the guard must not make the
    // subscription deaf.
    await act(async () => {
      usePrefs.setState({ accent: "teal" });
    });
    expect(renders).toBeGreaterThan(before);
    expect(result.current.accent).toBe("teal");

    await act(async () => {
      usePrefs.setState({ ...DEFAULT_PREFS });
    });
  });
});
