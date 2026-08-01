import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { useSelection } from "@/hooks/useSelection";
import { items } from "@/test/harness";

describe("useSelection", () => {
  it("keeps an explicitly opened mode, then exits when its last item leaves", async () => {
    const visible = items(2);
    const { result } = renderHook(() => useSelection(visible));

    act(() => result.current.begin());
    expect(result.current.selecting).toBe(true);

    act(() => result.current.toggle(visible[0]!.id));
    expect(result.current.selected.has(visible[0]!.id)).toBe(true);

    act(() => result.current.toggle(visible[0]!.id));
    await waitFor(() => expect(result.current.selecting).toBe(false));
    expect(result.current.selected.size).toBe(0);
  });

  it("exits when filtering prunes every selected id", async () => {
    const visible = items(2);
    const { result, rerender } = renderHook(
      ({ shown }) => useSelection(shown),
      { initialProps: { shown: visible } },
    );
    act(() => result.current.toggle(visible[0]!.id));

    rerender({ shown: [visible[1]!] });
    await waitFor(() => expect(result.current.selecting).toBe(false));
    expect(result.current.selected.size).toBe(0);
  });
});
