import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BulkOutcome } from "./useHistoryMutations";
import { useHistorySelection } from "./useHistorySelection";
import { items } from "@/test/harness";

const bulkPin = vi.hoisted(() => ({
  isPending: false,
  mutate: vi.fn(),
  applyOutcome: null as ((outcome: BulkOutcome) => void) | null,
}));
const bulkDelete = vi.hoisted(() => ({
  isPending: false,
  mutateAsync: vi.fn(),
}));

vi.mock("./useHistoryMutations", () => ({
  useBulkPin: (applyOutcome: (outcome: BulkOutcome) => void) => {
    bulkPin.applyOutcome = applyOutcome;
    return bulkPin;
  },
  useBulkDelete: () => bulkDelete,
}));

describe("useHistorySelection", () => {
  beforeEach(() => {
    bulkPin.mutate.mockReset();
    bulkPin.applyOutcome = null;
    bulkDelete.mutateAsync.mockReset();
  });

  it("clears a successful pin when the backend outcome arrives", async () => {
    const visible = items(2);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    act(() => result.current.togglePin());

    expect(bulkPin.mutate).toHaveBeenCalledWith({
      items: visible,
      pinned: true,
    });
    expect(result.current.selection.active).toBe(true);

    act(() => bulkPin.applyOutcome?.({ done: 2, failedIds: [] }));
    await waitFor(() => expect(result.current.selection.active).toBe(false));
    expect(result.current.selection.selected).toEqual(new Set());
  });

  it("keeps only failed rows selected after a partial pin", async () => {
    const visible = items(2);
    const { result } = renderHook(() => useHistorySelection(visible));

    act(() => {
      result.current.selection.toggle(visible[0]!.id);
      result.current.selection.toggle(visible[1]!.id);
    });
    act(() => result.current.togglePin());
    act(() =>
      bulkPin.applyOutcome?.({
        done: 1,
        failedIds: [visible[1]!.id],
      }),
    );

    await waitFor(() =>
      expect([...result.current.selection.selected]).toEqual([visible[1]!.id]),
    );
  });
});
