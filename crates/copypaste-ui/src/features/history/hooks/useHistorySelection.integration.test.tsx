import { StrictMode, type ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useBulkPin } from "./useHistoryMutations";
import { useHistorySelection } from "./useHistorySelection";
import type { Item } from "@/lib/ipc";
import { items, testClient } from "@/test/harness";

const setPinned = vi.hoisted(() => vi.fn());

vi.mock("sonner", () => ({
  toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}));

vi.mock("@/hooks/historyRefresh", () => ({
  invalidateHistoryQueries: vi.fn().mockResolvedValue(undefined),
  coalesceHistoryInvalidation: vi.fn().mockResolvedValue(undefined),
  STATUS_KEY: ["status"],
}));

vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  setPinned,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

function wrapper() {
  const client = testClient();
  return ({ children }: { children: ReactNode }) => (
    <StrictMode>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </StrictMode>
  );
}

function SelectionProbe({ shown }: { shown: readonly Item[] }) {
  const bulk = useHistorySelection(shown);
  return (
    <>
      <output aria-label="Bulk busy state">{String(bulk.busy)}</output>
      {shown.map((entry) => (
        <button
          key={entry.id}
          role="checkbox"
          aria-label={`Select ${entry.id}`}
          aria-checked={bulk.selection.selected.has(entry.id)}
          onClick={() => bulk.selection.toggle(entry.id)}
        />
      ))}
      {bulk.selection.active ? (
        <div
          role="toolbar"
          aria-label="Selection actions"
          data-busy={bulk.busy}
        >
          <button disabled={bulk.busy} onClick={bulk.togglePin}>
            Pin
          </button>
        </div>
      ) : null}
    </>
  );
}

function checkedIds(): string[] {
  return screen
    .getAllByRole("checkbox")
    .filter((box) => box.getAttribute("aria-checked") === "true")
    .map((box) => box.getAttribute("aria-label")!.replace("Select ", ""));
}

describe("the real bulk-pin selection composition", () => {
  beforeEach(() => setPinned.mockReset());

  it("reports an empty failed-id set after both acknowledgements", async () => {
    const selected = items(2);
    setPinned.mockImplementation(async (id: string) => ({
      ...selected.find((entry) => entry.id === id)!,
      pinned: true,
    }));
    const { result } = renderHook(() => useBulkPin(), {
      wrapper: wrapper(),
    });

    const outcome = await result.current.mutateAsync({
      items: selected,
      pinned: true,
    });

    expect(setPinned).toHaveBeenCalledTimes(2);
    expect(outcome).toEqual({ done: 2, failedIds: [] });
  });

  it("ends selection when canonical rows confirm commits before responses", async () => {
    const selected = items(2);
    const first = deferred<Item>();
    const second = deferred<Item>();
    setPinned
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const user = userEvent.setup();
    const view = render(<SelectionProbe shown={selected} />, {
      wrapper: wrapper(),
    });

    await user.click(screen.getByRole("checkbox", { name: "Select row-0" }));
    await user.click(screen.getByRole("checkbox", { name: "Select row-1" }));
    expect(checkedIds()).toEqual(["row-0", "row-1"]);
    await user.click(screen.getByRole("button", { name: "Pin" }));
    expect(
      screen
        .getByRole("toolbar", { name: "Selection actions" })
        .getAttribute("data-busy"),
    ).toBe("true");
    expect(screen.getByRole("status", { name: "Bulk busy state" }).textContent).toBe(
      "true",
    );

    view.rerender(
      <SelectionProbe
        shown={selected.map((entry, index) => ({
          ...entry,
          pinned: index === 0,
        }))}
      />,
    );
    await waitFor(() => expect(checkedIds()).toEqual(["row-1"]));

    await act(async () =>
      first.resolve({ ...selected[0]!, pinned: true }),
    );
    await waitFor(() => expect(setPinned).toHaveBeenCalledTimes(2));
    view.rerender(
      <SelectionProbe
        shown={selected.map((entry) => ({ ...entry, pinned: true }))}
      />,
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("toolbar", { name: "Selection actions" }),
      ).toBeNull(),
    );
    expect(checkedIds()).toEqual([]);

    await act(async () =>
      second.resolve({ ...selected[1]!, pinned: true }),
    );
    await waitFor(() => expect(checkedIds()).toEqual([]));
    await waitFor(() =>
      expect(
        screen.getByRole("status", { name: "Bulk busy state" }).textContent,
      ).toBe("false"),
    );
  });
});
