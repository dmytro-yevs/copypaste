import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useHistorySelection } from "./useHistorySelection";
import { items } from "@/test/harness";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

const bulkPin = vi.hoisted(() => ({
  mutateAsync: vi.fn(),
}));
const bulkDelete = vi.hoisted(() => ({
  mutateAsync: vi.fn(),
}));

vi.mock("./useHistoryMutations", () => ({
  useBulkPin: () => bulkPin,
  useBulkDelete: () => bulkDelete,
}));

describe("useHistorySelection", () => {
  beforeEach(() => {
    bulkPin.mutateAsync.mockReset();
    bulkDelete.mutateAsync.mockReset();
  });

  it("clears a successful pin when the backend outcome arrives", async () => {
    const visible = items(2);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    bulkPin.mutateAsync.mockResolvedValue({ done: 2, failedIds: [] });
    act(() => result.current.togglePin());

    expect(bulkPin.mutateAsync).toHaveBeenCalledWith({
      items: visible,
      pinned: true,
    });
    expect(result.current.selection.active).toBe(true);
    await waitFor(() => expect(result.current.selection.active).toBe(false));
    expect(result.current.selection.selected).toEqual(new Set());
    expect(result.current.busy).toBe(false);
  });

  it("ends the selection only after the snapshotted pin run settles", async () => {
    const visible = items(2);
    const run = deferred<{ done: number; failedIds: readonly string[] }>();
    bulkPin.mutateAsync.mockReturnValue(run.promise);
    const { result, rerender } = renderHook(
      ({ shown }) => useHistorySelection(shown),
      { initialProps: { shown: visible } },
    );

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    act(() => result.current.togglePin());

    expect(result.current.busy).toBe(true);
    expect(result.current.selection.active).toBe(true);
    expect(bulkPin.mutateAsync).toHaveBeenCalledWith({
      items: visible,
      pinned: true,
    });

    rerender({
      shown: [...visible]
        .reverse()
        .map((entry) => ({ ...entry, pinned: true })),
    });
    expect(result.current.selection.active).toBe(false);
    expect(result.current.busy).toBe(true);

    await act(async () => run.resolve({ done: 2, failedIds: [] }));
    await waitFor(() => expect(result.current.selection.active).toBe(false));
    expect(result.current.busy).toBe(false);
  });

  it("releases busy state but keeps the selection when the pin run rejects", async () => {
    const visible = items(1);
    const run = deferred<{ done: number; failedIds: readonly string[] }>();
    bulkPin.mutateAsync.mockReturnValue(run.promise);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => result.current.selection.toggle(visible[0]!.id));
    act(() => result.current.togglePin());
    expect(result.current.busy).toBe(true);

    await act(async () => run.reject(new Error("service unavailable")));
    await waitFor(() => expect(result.current.busy).toBe(false));
    expect(result.current.selection.active).toBe(true);
  });

  it("ends an idle selection through the owner lifecycle", () => {
    const visible = items(1);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => result.current.selection.toggle(visible[0]!.id));
    expect(result.current.selection.active).toBe(true);
    act(() => result.current.end());

    expect(result.current.selection.active).toBe(false);
    expect(result.current.selection.selected).toEqual(new Set());
  });

  it("keeps only failed rows selected after a partial pin", async () => {
    const visible = items(2);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    bulkPin.mutateAsync.mockResolvedValue({
      done: 1,
      failedIds: [visible[1]!.id],
    });
    act(() => result.current.togglePin());

    await waitFor(() =>
      expect([...result.current.selection.selected]).toEqual([visible[1]!.id]),
    );
    expect(result.current.busy).toBe(false);
  });

  it("restores a reversed failure without dropping a new selection", async () => {
    const shown = items(3);
    const run = deferred<{ done: number; failedIds: readonly string[] }>();
    bulkPin.mutateAsync.mockReturnValue(run.promise);
    const { result, rerender } = renderHook(
      ({ visible }) => useHistorySelection(visible),
      { initialProps: { visible: shown } },
    );

    act(() => {
      result.current.selection.toggle(shown[0]!.id);
      result.current.selection.toggle(shown[1]!.id);
    });
    act(() => result.current.togglePin());
    rerender({
      visible: shown.map((entry, index) => ({
        ...entry,
        pinned: index < 2,
      })),
    });
    await waitFor(() => expect(result.current.selection.active).toBe(false));

    act(() => result.current.selection.toggle(shown[2]!.id));
    rerender({
      visible: shown.map((entry, index) => ({
        ...entry,
        pinned: index === 0,
      })),
    });
    await waitFor(() =>
      expect([...result.current.selection.selected]).toEqual([
        shown[2]!.id,
        shown[1]!.id,
      ]),
    );

    await act(async () =>
      run.resolve({ done: 1, failedIds: [shown[1]!.id] }),
    );
    await waitFor(() => expect(result.current.busy).toBe(false));
    expect(new Set(result.current.selection.selected)).toEqual(
      new Set([shown[1]!.id, shown[2]!.id]),
    );
  });

  it("keeps only failed rows selected after a partial delete", async () => {
    const visible = items(2);
    bulkDelete.mutateAsync.mockResolvedValue({
      done: 1,
      failedIds: [visible[1]!.id],
    });
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
      result.current.requestDelete();
    });
    act(() => result.current.confirmDelete());

    expect(bulkDelete.mutateAsync).toHaveBeenCalledWith(visible);
    expect(result.current.busy).toBe(true);
    await waitFor(() =>
      expect([...result.current.selection.selected]).toEqual([visible[1]!.id]),
    );
    expect(result.current.busy).toBe(false);
    expect(result.current.confirmingDelete).toBe(false);
  });
});
