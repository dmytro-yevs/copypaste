import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BulkOutcome } from "./useHistoryMutations";
import { useHistorySelection } from "./useHistorySelection";
import { items } from "@/test/harness";

const bulkPin = vi.hoisted(() => ({
  isPending: false,
  mutateAsync: vi.fn(),
}));
const bulkDelete = vi.hoisted(() => ({
  isPending: false,
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
    let resolvePin!: (outcome: BulkOutcome) => void;
    bulkPin.mutateAsync.mockImplementation(
      () =>
        new Promise<BulkOutcome>((resolve) => {
          resolvePin = resolve;
        }),
    );
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    act(() => result.current.togglePin());

    expect(bulkPin.mutateAsync).toHaveBeenCalledWith({
      items: visible,
      pinned: true,
    });
    expect(result.current.selection.active).toBe(true);

    act(() => resolvePin({ done: 2, failedIds: [] }));
    await waitFor(() => expect(result.current.selection.active).toBe(false));
    expect(result.current.selection.selected).toEqual(new Set());
  });

  it("keeps only failed rows selected after a partial pin", async () => {
    const visible = items(2);
    bulkPin.mutateAsync.mockResolvedValue({
      done: 1,
      failedIds: [visible[1]!.id],
    });
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    act(() => result.current.togglePin());

    await waitFor(() =>
      expect([...result.current.selection.selected]).toEqual([visible[1]!.id]),
    );
  });
});
