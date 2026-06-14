import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// Mock the Tauri core bridge so EVERY IPC call (and every Tauri-direct command)
// rejects, simulating the daemon being completely unreachable. This is the
// exact live-bug scenario: opening Settings while the daemon is down.
const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

beforeEach(() => {
  invoke.mockReset();
  // Reject everything — daemon offline / Tauri command failure.
  invoke.mockRejectedValue("daemon_offline:/tmp/copypaste.sock");
});

describe("SettingsView resilience (daemon down)", () => {
  it("renders without throwing when every IPC call rejects, and the tree stays mounted", async () => {
    // Wrap in the boundary exactly as the app does. If SettingsView threw on a
    // rejected IPC call, the boundary fallback would replace it; we assert the
    // OPPOSITE — the real Settings UI renders and the boundary never trips.
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // The daemon-unavailable state must be surfaced (not a blank screen).
    await waitFor(() => {
      expect(
        screen.getByText(/Daemon not running — clipboard sync paused/i),
      ).toBeInTheDocument();
    });

    // The boundary fallback must NOT be shown — the component handled the
    // failure itself rather than throwing.
    expect(screen.queryByText(/Something went wrong/i)).not.toBeInTheDocument();

    // A Retry action is available.
    expect(screen.getByRole("button", { name: /Retry/i })).toBeInTheDocument();

    // The Settings header still rendered — the tree is mounted, not blank.
    expect(
      screen.getByRole("heading", { name: /Settings/i }),
    ).toBeInTheDocument();
  });

  it("Retry re-attempts the load without crashing", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    const retry = await screen.findByRole("button", { name: /Retry/i });
    fireEvent.click(retry);

    // Still resilient after retry (daemon still down): banner persists, no throw.
    await waitFor(() => {
      expect(
        screen.getByText(/Daemon not running — clipboard sync paused/i),
      ).toBeInTheDocument();
    });
    expect(screen.queryByText(/Something went wrong/i)).not.toBeInTheDocument();
  });
});

describe("ErrorBoundary", () => {
  it("renders a readable fallback with Retry instead of unmounting on a thrown child", () => {
    function Boom(): never {
      throw new Error("kaboom from render");
    }

    // Silence the expected React error log for this intentional throw.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary label="History">
        <Boom />
      </ErrorBoundary>,
    );
    spy.mockRestore();

    expect(screen.getByText(/Something went wrong in History/i)).toBeInTheDocument();
    expect(screen.getByText(/kaboom from render/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Retry/i })).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// P2P toggle restart behaviour
// ---------------------------------------------------------------------------

/**
 * Build an invoke mock that simulates a healthy daemon.
 * ipcCall wraps daemon replies as { ok, data, error, error_code }.
 */
function makeOnlineInvoke(overrides: Record<string, (args?: unknown) => unknown> = {}) {
  return (cmd: string, args?: unknown): Promise<unknown> => {
    if (overrides[cmd]) return Promise.resolve(overrides[cmd](args));

    if (cmd === "ipc_call") {
      const method = (args as { method?: string } | undefined)?.method;
      switch (method) {
        case "status":
          return Promise.resolve({
            ok: true,
            data: {
              status: "running",
              ready: true,
              degraded: false,
              degraded_reason: null,
              build_version: "0.5.5",
            },
            error: null,
            error_code: null,
          });
        case "get_config":
          return Promise.resolve({
            ok: true,
            data: {
              p2p_enabled: true,
              supabase_url: null,
              supabase_anon_key: null,
              max_text_size_bytes: 10 * 1024 * 1024,
              max_image_size_bytes: 25 * 1024 * 1024,
              max_file_size_bytes: 128 * 1024 * 1024,
              storage_quota_bytes: 5 * 1024 * 1024 * 1024,
              sensitive_ttl_secs: 300,
              image_quality: 85,
              sync_on_wifi_only: false,
              sound_on_copy: false,
              notify_on_copy: false,
            },
            error: null,
            error_code: null,
          });
        case "get_private_mode":
          return Promise.resolve({
            ok: true,
            data: { private_mode: false },
            error: null,
            error_code: null,
          });
        case "get_sync_status":
          return Promise.resolve({
            ok: true,
            data: {
              passphrase_set: false,
              supabase_configured: false,
              signed_in: false,
              email: null,
              last_sync_ms: null,
            },
            error: null,
            error_code: null,
          });
        case "get_limits":
          return Promise.resolve({
            ok: true,
            data: {
              max_text_size_bytes: 10 * 1024 * 1024,
              max_image_size_bytes: 25 * 1024 * 1024,
              max_file_size_bytes: 128 * 1024 * 1024,
              storage_quota_bytes: 5 * 1024 * 1024 * 1024,
              sensitive_ttl_secs: 300,
              image_quality: 85,
              sync_on_wifi_only: false,
              sound_on_copy: false,
              notify_on_copy: false,
            },
            error: null,
            error_code: null,
          });
        case "set_config":
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
        default:
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
    }

    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "restart_daemon") return Promise.resolve(undefined);
    if (cmd === "app_version") return Promise.resolve("0.5.5");
    return Promise.resolve(undefined);
  };
}

describe("P2P toggle triggers daemon restart", () => {
  it("calls restart_daemon after P2P toggle changes the value and set_config succeeds", async () => {
    invoke.mockImplementation(makeOnlineInvoke());

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for the component to reach "ready" state (tab bar becomes interactive).
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Navigate to the Sync tab. TabBar renders plain <button> elements (not role="tab").
    const syncTabBtn = await screen.findByText("Sync");
    await act(async () => {
      fireEvent.click(syncTabBtn);
    });

    // The P2P toggle should be visible (Toggle uses role="switch").
    const p2pToggle = await screen.findByRole("switch", { name: /P2P sync/i });
    expect(p2pToggle).toBeInTheDocument();

    // Reset call tracking after load; now only watch for the restart call.
    invoke.mockClear();
    invoke.mockImplementation(makeOnlineInvoke());

    // Click the toggle (currently checked=true → will toggle to false).
    await act(async () => {
      fireEvent.click(p2pToggle);
    });

    // After set_config resolves, restart_daemon must be called.
    await waitFor(() => {
      const restartCalls = invoke.mock.calls.filter(
        ([cmd]: [string]) => cmd === "restart_daemon",
      );
      expect(restartCalls.length).toBeGreaterThanOrEqual(1);
    });
  });

  it("shows 'Restarting sync service…' message after P2P toggle", async () => {
    // Use a stalling restart_daemon so the "Restarting…" message stays visible
    // long enough for the assertion (it would be replaced by "Sync service
    // restarted" the instant the promise resolves).
    let resolveRestart!: () => void;
    const restartPromise = new Promise<void>((res) => { resolveRestart = res; });

    invoke.mockImplementation(
      makeOnlineInvoke({
        restart_daemon: () => restartPromise,
      }),
    );

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const syncTabBtn = await screen.findByText("Sync");
    await act(async () => {
      fireEvent.click(syncTabBtn);
    });

    const p2pToggle = await screen.findByRole("switch", { name: /P2P sync/i });

    // Fire the toggle without awaiting the full async chain — the restart is
    // intentionally stalled so we can assert the in-flight message.
    fireEvent.click(p2pToggle);

    // The restart message should appear in the limitsMsg area for p2p_enabled.
    await waitFor(() => {
      expect(screen.getByText(/Restarting sync service/i)).toBeInTheDocument();
    });

    // Unblock the restart so the component can clean up properly.
    await act(async () => { resolveRestart(); });
  });

  // ---------------------------------------------------------------------------
  // §6 Liquid Glass Settings additions
  // ---------------------------------------------------------------------------

  it("§6.2 Display tab has a density segmented control as the first row", async () => {
    invoke.mockImplementation(makeOnlineInvoke());
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const displayTab = await screen.findByText("Display");
    await act(async () => { fireEvent.click(displayTab); });

    // "Row density" label must exist in the Display tab
    expect(screen.getByText(/Row density/i)).toBeInTheDocument();
    // Both density options must be present
    expect(screen.getByRole("button", { name: /comfortable/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /compact/i })).toBeInTheDocument();
  });

  it("§6.3 Storage tab has a Max history items slider row", async () => {
    invoke.mockImplementation(makeOnlineInvoke());
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTab); });

    // "Max history items" label must exist
    expect(screen.getByText(/Max history items/i)).toBeInTheDocument();
  });

  it("§6.5 SliderRow inputs have a datalist for tick marks", async () => {
    invoke.mockImplementation(makeOnlineInvoke());
    const { container } = render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTab = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTab); });

    // At least one range input should have a list attribute (datalist)
    const rangeInputs = container.querySelectorAll('input[type="range"][list]');
    expect(rangeInputs.length).toBeGreaterThan(0);
  });

  it("does NOT call restart_daemon when set_config fails", async () => {
    // Make set_config fail so the restart branch is not reached.
    invoke.mockImplementation(
      makeOnlineInvoke({
        ipc_call: (args?: unknown) => {
          const method = (args as { method?: string } | undefined)?.method;
          if (method === "set_config") {
            return { ok: false, data: null, error: "write failed", error_code: null };
          }
          // Delegate to default online mock for all other methods.
          return makeOnlineInvoke()("ipc_call", args);
        },
      }),
    );

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const syncTabBtn = await screen.findByText("Sync");
    await act(async () => {
      fireEvent.click(syncTabBtn);
    });

    const p2pToggle = await screen.findByRole("switch", { name: /P2P sync/i });

    invoke.mockClear();
    invoke.mockImplementation(
      makeOnlineInvoke({
        ipc_call: (args?: unknown) => {
          const method = (args as { method?: string } | undefined)?.method;
          if (method === "set_config") {
            return { ok: false, data: null, error: "write failed", error_code: null };
          }
          return makeOnlineInvoke()("ipc_call", args);
        },
      }),
    );

    await act(async () => {
      fireEvent.click(p2pToggle);
    });

    // Allow async handlers to settle.
    await new Promise((r) => setTimeout(r, 50));

    const restartCalls = invoke.mock.calls.filter(
      ([cmd]: [string]) => cmd === "restart_daemon",
    );
    expect(restartCalls.length).toBe(0);
  });
});
