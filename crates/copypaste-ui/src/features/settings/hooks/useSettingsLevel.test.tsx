import { useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { useSettingsLevel } from "./useSettingsLevel";

function currentPath(): string[] | undefined {
  return window.history.state?.copypasteSettingsLevel?.path;
}

it("restores the section and nested view across actual Back and Forward traversal", async () => {
  const { result, unmount } = renderHook(() => {
    const [path, setPath] = useState<string[]>([]);
    const back = useSettingsLevel(path, (next) => setPath([...next]));
    return { path, setPath, back };
  });

  act(() => result.current.setPath(["diagnostics"]));
  expect(currentPath()).toEqual(["diagnostics"]);
  act(() => result.current.setPath(["diagnostics", "runtime-events"]));
  expect(currentPath()).toEqual(["diagnostics", "runtime-events"]);

  act(() => window.history.back());
  await waitFor(() => expect(result.current.path).toEqual(["diagnostics"]));
  act(() => window.history.forward());
  await waitFor(() => expect(result.current.path).toEqual(["diagnostics", "runtime-events"]));

  act(() => result.current.back());
  await waitFor(() => expect(result.current.path).toEqual(["diagnostics"]));
  act(() => window.history.back());
  await waitFor(() => expect(result.current.path).toEqual([]));
  act(() => window.history.forward());
  await waitFor(() => expect(result.current.path).toEqual(["diagnostics"]));

  act(() => window.dispatchEvent(new PopStateEvent("popstate", { state: { other: true } })));
  expect(result.current.path).toEqual(["diagnostics"]);
  unmount();
  await waitFor(() => expect(currentPath()).toBeUndefined());
});

it("waits for an async rotation collapse before reentering compact Settings", async () => {
  const { result, unmount } = renderHook(() => {
    const [path, setPath] = useState<string[]>(["diagnostics", "runtime-events"]);
    const [compact, setCompact] = useState(true);
    useSettingsLevel(compact ? path : [], (next) => setPath([...next]));
    return { path, setCompact };
  });
  expect(currentPath()).toEqual(["diagnostics", "runtime-events"]);

  const pop = vi.fn();
  window.addEventListener("popstate", pop);
  act(() => result.current.setCompact(false));
  act(() => result.current.setCompact(true));
  await waitFor(() => expect(pop).toHaveBeenCalled());
  await waitFor(() => expect(currentPath()).toEqual(["diagnostics", "runtime-events"]));
  act(() => window.history.back());
  await waitFor(() => expect(result.current.path).toEqual(["diagnostics"]));
  unmount();
  await waitFor(() => expect(currentPath()).toBeUndefined());
  window.removeEventListener("popstate", pop);
});

it("retires a departing screen before a rapid Settings remount owns history", async () => {
  const first = renderHook(() =>
    useSettingsLevel(["diagnostics", "runtime-events"], () => {}),
  );
  expect(currentPath()).toEqual(["diagnostics", "runtime-events"]);
  const firstOwner = window.history.state.copypasteSettingsLevel.owner;
  first.unmount();

  const second = renderHook(() => {
    const [path, setPath] = useState<string[]>(["diagnostics", "runtime-events"]);
    useSettingsLevel(path, (next) => setPath([...next]));
    return path;
  });
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.owner).not.toBe(firstOwner));
  await waitFor(() => expect(currentPath()).toEqual(["diagnostics", "runtime-events"]));
  act(() => window.history.back());
  await waitFor(() => expect(second.result.current).toEqual(["diagnostics"]));
  second.unmount();
  await waitFor(() => expect(currentPath()).toBeUndefined());
});

it("finishes an in-flight collapse before a new Settings screen takes ownership", async () => {
  const first = renderHook(() => {
    const [compact, setCompact] = useState(true);
    useSettingsLevel(compact ? ["diagnostics", "runtime-events"] : [], () => {});
    return setCompact;
  });
  const firstOwner = window.history.state.copypasteSettingsLevel.owner;
  act(() => first.result.current(false));
  first.unmount();

  const second = renderHook(() => {
    const [path, setPath] = useState<string[]>(["appearance"]);
    useSettingsLevel(path, (next) => setPath([...next]));
    return path;
  });
  await waitFor(() => expect(window.history.state?.copypasteSettingsLevel?.owner).not.toBe(firstOwner));
  await waitFor(() => expect(currentPath()).toEqual(["appearance"]));
  act(() => window.history.back());
  await waitFor(() => expect(second.result.current).toEqual([]));
  second.unmount();
  await waitFor(() => expect(currentPath()).toBeUndefined());
});

it("drops an obsolete Forward destination when a new section replaces a nested route", async () => {
  const { result, unmount } = renderHook(() => {
    const [path, setPath] = useState<string[]>(["diagnostics", "runtime-events"]);
    useSettingsLevel(path, (next) => setPath([...next]));
    return { path, setPath };
  });
  act(() => result.current.setPath(["appearance"]));
  await waitFor(() => expect(currentPath()).toEqual(["appearance"]));
  act(() => window.history.back());
  await waitFor(() => expect(result.current.path).toEqual([]));
  act(() => window.history.forward());
  await waitFor(() => expect(result.current.path).toEqual(["appearance"]));
  const pop = vi.fn();
  window.addEventListener("popstate", pop);
  act(() => window.history.forward());
  await new Promise((resolve) => window.setTimeout(resolve, 40));
  expect(pop).not.toHaveBeenCalled();
  expect(result.current.path).toEqual(["appearance"]);
  window.removeEventListener("popstate", pop);
  unmount();
  await waitFor(() => expect(currentPath()).toBeUndefined());
});
