/**
 * The render-cost gate for F-UI-3 and F-UI-4.
 *
 * Both findings were argued from identity flow — props, `memo`, React Query's
 * tracked properties — and neither had a measurement behind it, because the
 * repository had no way to count commits. This file is that way.
 *
 * F-UI-3: `counters.uptime_secs` ticks once a second inside the payload the 2 s
 * status poll returns, so `replaceEqualDeep` could never hand back the previous
 * object and every consumer of `status.data` re-rendered twice a second, window
 * merely open. The poll itself is a defect fix (`CopyPaste-f701`) and stays at
 * 2 s — what is asserted here is that a *ticking counter* no longer reaches the
 * rows.
 *
 * F-UI-4: `memo(HistoryRow)` compared six callback props that were re-created
 * on every `HistoryList` render, so it never held. The counter below wraps the
 * row in its own `memo` over the same props, so it increments exactly when the
 * real comparison would fail.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { memo, type ComponentProps } from "react";

import { HistoryView } from "@/components/history/HistoryView";
import { STATUS_POLL_MS } from "@/lib/layout";
import { items, page, status, withClient } from "@/test/harness";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...a: unknown[]) => listItems(...a),
    searchItems: (...a: unknown[]) => searchItems(...a),
    getStatus: () => getStatus(),
    getSourceAppIcon: async () => null,
    getImagePreview: async () => {
      throw new Error("no preview in this harness");
    },
  };
});

const rendered: string[] = [];

vi.mock("@/components/history/HistoryRow", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@/components/history/HistoryRow")>();
  const Real = actual.HistoryRow;
  const Counted = memo(function Counted(props: ComponentProps<typeof Real>) {
    rendered.push(props.item.id);
    return <Real {...props} />;
  });
  return { ...actual, HistoryRow: Counted };
});

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  rendered.length = 0;
  listItems.mockReset().mockResolvedValue(page(items(12)));
  searchItems.mockReset().mockResolvedValue(page([]));
  getStatus.mockReset();
  useUi.setState({ query: "", activeId: null, settingsTab: null });
  usePrefs.getState().reset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

/** A daemon whose uptime advances a second per status poll — the real one. */
function tickingStatus() {
  let uptime = 60;
  getStatus.mockImplementation(async () => {
    uptime += 2;
    return status({
      counters: {
        rejected_too_large: 0,
        lost_intermediates: 0,
        sensitive_swept: 0,
        index_purged: 0,
        uptime_secs: uptime,
      },
    });
  });
}

async function settle() {
  await waitFor(() => expect(screen.getAllByRole("listitem").length).toBeGreaterThan(0));
  await waitFor(() => expect(getStatus).toHaveBeenCalled());
  await act(async () => {
    await vi.advanceTimersByTimeAsync(50);
  });
  rendered.length = 0;
}

describe("an open window that nothing is happening in", () => {
  it("does not re-render a row for a status poll whose only change is the uptime", async () => {
    tickingStatus();
    withClient(<HistoryView />);
    await settle();

    const before = getStatus.mock.calls.length;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(STATUS_POLL_MS * 5);
    });

    // The 2s cadence is load-bearing (`CopyPaste-f701`): the polls did happen.
    expect(getStatus.mock.calls.length).toBeGreaterThanOrEqual(before + 4);
    // And not one of them reached a row.
    expect(rendered).toEqual([]);
  }, 30_000);

  it("re-renders only the two rows whose selection actually changed", async () => {
    getStatus.mockResolvedValue(status());
    withClient(<HistoryView />);
    await settle();

    await act(async () => {
      useUi.getState().setActiveId("row-3");
    });
    await waitFor(() => expect(rendered.length).toBeGreaterThan(0));
    expect(new Set(rendered)).toEqual(new Set(["row-3"]));

    rendered.length = 0;
    await act(async () => {
      useUi.getState().setActiveId("row-5");
    });
    await waitFor(() => expect(rendered.length).toBeGreaterThan(0));
    expect(new Set(rendered)).toEqual(new Set(["row-3", "row-5"]));
  }, 30_000);
});
