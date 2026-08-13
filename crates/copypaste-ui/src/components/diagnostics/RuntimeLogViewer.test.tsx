import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

import { RuntimeLogViewer } from "@/components/diagnostics/RuntimeLogViewer";
import { en } from "@/i18n";
import { withUser } from "@/test/harness";

const getRuntimeLogEvents = vi.fn();
const copyText = vi.fn();

vi.mock("@/service/runtimeLogs", () => ({
  getRuntimeLogEvents: (...args: unknown[]) => getRuntimeLogEvents(...args),
}));

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, copyText: (...args: unknown[]) => copyText(...args) };
});

describe("RuntimeLogViewer", () => {
  beforeEach(() => {
    getRuntimeLogEvents.mockReset();
    copyText.mockReset();
  });

  it("renders only the redacted event fields and copies one entry", async () => {
    getRuntimeLogEvents.mockResolvedValue({
      events: [{
        timestamp_ms: 1_700_000_000_000,
        level: "warn",
        process: "daemon",
        target: "copypaste::capture",
        message: "capture paused",
      }],
      next_cursor: null,
    });
    copyText.mockResolvedValue(undefined);
    const { user } = withUser(<RuntimeLogViewer />);

    expect(await screen.findByText("capture paused")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Search runtime events" }).getAttribute("data-slot")).toBe("input");
    expect(screen.getByText("WARN")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Copy log entry" }));
    await waitFor(() => expect(copyText).toHaveBeenCalledWith(expect.stringContaining("capture paused")));
    await user.click(screen.getByRole("button", { name: "Copy loaded log events" }));
    await waitFor(() => expect(copyText).toHaveBeenLastCalledWith(expect.stringContaining("capture paused")));
  });

  it("loads older events automatically when the list reaches its end", async () => {
    // Keyed on the cursor, not on call order: the head query polls the newest
    // window alongside the paged reads, so a fixed sequence would hand the
    // wrong page to whichever call happened to land second.
    getRuntimeLogEvents.mockImplementation(
      async ({ cursor }: { cursor: string | null }) =>
        cursor === null
          ? {
              events: [{
                timestamp_ms: 1_700_000_000_000,
                level: "info",
                process: "app",
                target: "copypaste::app",
                message: "started",
              }],
              next_cursor: "50",
            }
          : { events: [], next_cursor: null },
    );
    withUser(<RuntimeLogViewer />);
    const list = await screen.findByRole("log", { name: "Runtime event list" });
    Object.defineProperties(list, {
      scrollHeight: { configurable: true, value: 500 },
      scrollTop: { configurable: true, value: 240, writable: true },
      clientHeight: { configurable: true, value: 100 },
    });
    fireEvent.scroll(list);

    await waitFor(() => expect(getRuntimeLogEvents).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: "50", limit: 50 })));
    expect(screen.queryByText("Load older events")).toBeNull();
  });

  /** The viewer authored its own English, including a second copy of the
   *  service's own failure sentence. */
  it("reads its filters and its failure state from the catalogue", async () => {
    getRuntimeLogEvents.mockRejectedValue(new Error("no log"));
    withUser(<RuntimeLogViewer />);

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      en.runtimeLog.loadFailed,
    );
    for (const label of Object.values(en.runtimeLog.level)) {
      if (label === en.runtimeLog.level.label) continue;
      expect(screen.getByRole("option", { name: label })).not.toBeNull();
    }
    expect(
      screen.getByRole("combobox", { name: en.runtimeLog.process.label }),
    ).not.toBeNull();
  });
});
