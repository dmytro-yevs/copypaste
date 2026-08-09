import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import {
  getRuntimeLogEvents,
  type RuntimeLogPage,
  type RuntimeLogQuery,
} from "./runtimeLogs";

afterEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  invoke.mockReset();
});

function enableTauri(): void {
  (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
}

describe("runtime log service", () => {
  it("passes the typed page query as the Tauri command's query argument", async () => {
    enableTauri();
    const query: RuntimeLogQuery = {
      cursor: "50",
      level: "warn",
      process: "daemon",
      query: "capture",
      limit: 25,
    };
    const page: RuntimeLogPage = { events: [], next_cursor: "75" };
    invoke.mockResolvedValue(page);

    await expect(getRuntimeLogEvents(query)).resolves.toBe(page);

    expect(invoke).toHaveBeenCalledWith("runtime_log_events", { query });
  });

  it("preserves Rust-owned query defaults", async () => {
    enableTauri();
    invoke.mockResolvedValue({ events: [], next_cursor: null });

    await getRuntimeLogEvents({});

    expect(invoke).toHaveBeenCalledWith("runtime_log_events", { query: {} });
  });

  it("does not retain a path from a rejected invocation", async () => {
    enableTauri();
    vi.spyOn(console, "error").mockImplementation(() => {});
    invoke.mockRejectedValue({
      code: "internal",
      retryable: false,
      message: "open C:\\Users\\alice\\AppData\\Local\\CopyPaste\\runtime.log failed",
    });

    const failure = await getRuntimeLogEvents({}).catch((error: unknown) => error);
    const exposed = `${String(failure)} ${JSON.stringify(failure)}`;

    expect(exposed).toContain("internal");
    expect(exposed).not.toMatch(/alice|AppData|runtime\.log|C:\\/);
  });
});
