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
        screen.getByText(/Background service not running — clipboard sync paused/i),
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
        screen.getByText(/Background service not running — clipboard sync paused/i),
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
    // P2-54h5: raw error detail is sanitized out of the DOM (logged to console
    // only) to avoid leaking filesystem paths / internal strings. The boundary
    // shows a generic fallback instead of the thrown message.
    expect(screen.queryByText(/kaboom from render/i)).not.toBeInTheDocument();
    expect(
      screen.getByText(/The background service may be unavailable, or this screen failed to load/i),
    ).toBeInTheDocument();
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

  it("§6.3 Storage tab has a History display limit slider row", async () => {
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

    // "History display limit" label must exist (UI-only pref, not daemon storage cap)
    expect(screen.getByText(/History display limit/i)).toBeInTheDocument();
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

  // ---------------------------------------------------------------------------
  // §hn5v Appearance section — palette / density / theme pickers
  // ---------------------------------------------------------------------------

  it("§hn5v Display tab has an Appearance section with palette picker", async () => {
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

    // "Appearance" subsection heading must be present (exact text, not "Popup appearance")
    expect(screen.getAllByText(/Appearance/i).some((el) => el.textContent === "Appearance")).toBe(true);

    // palette picker grid is data-testid="palette-picker"
    expect(document.querySelector('[data-testid="palette-picker"]')).not.toBeNull();

    // All 10 palettes rendered as swatch buttons (aria-label = palette name)
    expect(screen.getByRole("button", { name: /Graphite Mist/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Liquid Blue/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Aurora Violet/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Amber Night/i })).toBeInTheDocument();
  });

  it("§hn5v clicking a palette swatch updates the store (aria-pressed reflects active)", async () => {
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

    // Click "Liquid Blue" swatch — it should become pressed
    const liquidBlueBtn = screen.getByRole("button", { name: /Liquid Blue/i });
    await act(async () => { fireEvent.click(liquidBlueBtn); });

    // After click the button must be aria-pressed="true"
    expect(liquidBlueBtn.getAttribute("aria-pressed")).toBe("true");
  });

  it("§hn5v Display tab has density picker with compact/comfortable/spacious", async () => {
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

    // All three density options present
    expect(screen.getByRole("button", { name: /^compact$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^comfortable$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^spacious$/i })).toBeInTheDocument();
  });

  it("§hn5v Display tab has theme picker with dark/light/system", async () => {
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

    // Theme row in Appearance section
    expect(screen.getByText(/Color theme/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^light$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^dark$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^system$/i })).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------------
  // W-F4: Skin picker in the Appearance section of the Display tab
  // ---------------------------------------------------------------------------

  it("§W-F4 Display tab has a skin picker with Classic/Quiet/Vapor options", async () => {
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

    // "Visual style" skin picker must be in the Appearance section
    expect(screen.getByText(/Visual style/i)).toBeInTheDocument();

    // All three skin buttons must be present (data-testid="skin-picker")
    expect(document.querySelector('[data-testid="skin-picker"]')).not.toBeNull();
    expect(screen.getByRole("button", { name: /^Classic$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Quiet$/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Vapor$/i })).toBeInTheDocument();
  });

  it("§W-F4 clicking a skin button updates the store (aria-pressed reflects active)", async () => {
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

    // Click "Quiet" skin — it should become pressed
    const quietBtn = screen.getByRole("button", { name: /^Quiet$/i });
    await act(async () => { fireEvent.click(quietBtn); });

    // After click Quiet must be aria-pressed="true"
    expect(quietBtn.getAttribute("aria-pressed")).toBe("true");

    // Classic should no longer be pressed
    const classicBtn = screen.getByRole("button", { name: /^Classic$/i });
    expect(classicBtn.getAttribute("aria-pressed")).toBe("false");
  });

  it("§W-F4 existing appearance controls still present after adding skin picker", async () => {
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

    // All existing Appearance controls must still be present (zero-feature-loss)
    expect(document.querySelector('[data-testid="palette-picker"]')).not.toBeNull();
    expect(screen.getByText(/Row density/i)).toBeInTheDocument();
    expect(screen.getByText(/Color theme/i)).toBeInTheDocument();
    expect(screen.getByText(/Translucency/i)).toBeInTheDocument();
    expect(screen.getByText(/Reduce motion/i)).toBeInTheDocument();
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

// ---------------------------------------------------------------------------
// CopyPaste-3c72: BUG 2 — supabasePassword must be trimmed before saving
// Whitespace around the password causes silent auth failures. Email IS trimmed;
// password must match.
// ---------------------------------------------------------------------------

describe("CopyPaste-3c72: supabasePassword is trimmed before saving to config", () => {
  it("strips leading/trailing whitespace from supabasePassword before calling set_config", async () => {
    const setConfigCalls: unknown[] = [];
    invoke.mockImplementation((cmd: string, args?: unknown): Promise<unknown> => {
      if (cmd === "ipc_call") {
        const method = (args as { method?: string } | undefined)?.method;
        if (method === "set_config") {
          setConfigCalls.push(args);
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
        }
        // Delegate everything else to the online mock.
        return (makeOnlineInvoke())("ipc_call", args) as Promise<unknown>;
      }
      if (cmd === "restart_daemon") return Promise.resolve(undefined);
      if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "app_version") return Promise.resolve("0.5.5");
      return Promise.resolve(undefined);
    });

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for the component to be ready.
    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Navigate to the Sync tab to find the password field.
    const syncTabBtn = await screen.findByText("Sync");
    await act(async () => { fireEvent.click(syncTabBtn); });

    // Find the password input and type a value with surrounding whitespace.
    const passwordInput = await screen.findByPlaceholderText(/Password/i);
    await act(async () => {
      fireEvent.change(passwordInput, { target: { value: "  secretpass  " } });
    });

    // Find and click the Save button.
    const saveBtn = await screen.findByRole("button", { name: /Save/i });
    await act(async () => { fireEvent.click(saveBtn); });

    // Assert that set_config was called with the trimmed password (no whitespace).
    await waitFor(() => {
      expect(setConfigCalls.length).toBeGreaterThan(0);
    });

    const lastCall = setConfigCalls[setConfigCalls.length - 1] as {
      method: string;
      params: { supabase_password?: string };
    };
    // The saved password must be trimmed — no leading or trailing whitespace.
    expect(lastCall.params.supabase_password).toBe("secretpass");
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-tk2j: BUG 1 — non-daemon_offline IpcError must NOT show "offline"
// The early-return and catch block must probe status to distinguish real offline
// from a generic daemon error. Only "daemon_offline" transport errors warrant
// the offline banner.
// ---------------------------------------------------------------------------

describe("CopyPaste-tk2j: error handling — non-offline IpcError is NOT shown as offline", () => {
  it("shows an error banner (not 'Daemon not running') when get_config fails with a non-daemon_offline error while status is reachable", async () => {
    // Simulate: daemon is up (status returns ok) but get_config and get_private_mode
    // return error responses — NOT daemon_offline. The offline banner must NOT appear;
    // instead an error banner is shown ("Failed to load settings").
    invoke.mockImplementation((cmd: string, args?: unknown): Promise<unknown> => {
      if (cmd === "ipc_call") {
        const method = (args as { method?: string } | undefined)?.method;
        if (method === "get_config") {
          // Return a non-daemon_offline error response — daemon is up but cfg read failed.
          return Promise.resolve({
            ok: false,
            data: null,
            error: "database read error",
            error_code: "db_error",
          });
        }
        if (method === "get_private_mode") {
          // Also fail get_private_mode so both required fields are null.
          return Promise.resolve({
            ok: false,
            data: null,
            error: "pm read error",
            error_code: "db_error",
          });
        }
        if (method === "status") {
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
        }
        // All other IPC methods succeed generically.
        return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
      if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
      if (cmd === "app_version") return Promise.resolve("0.5.5");
      return Promise.resolve(undefined);
    });

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // The "error" state banner should appear — not the "offline" banner.
    await waitFor(() => {
      expect(screen.getByText(/Failed to load settings/i)).toBeInTheDocument();
    });

    // "Daemon not running" must NOT appear — daemon is reachable.
    expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
  });

  it("shows the offline banner only when the IpcError code is daemon_offline", async () => {
    // When the error is genuinely daemon_offline, the offline banner IS shown.
    invoke.mockRejectedValue("daemon_offline:/tmp/copypaste.sock");

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.getByText(/Background service not running — clipboard sync paused/i)).toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-wuek (NG-1): Parity — "Clear All" in canonical Settings location
// Canonical per PARITY-SPEC §8: destructive data operations belong in
// Settings (Storage tab → Data section), matching the Apple HIG model.
// Both platforms must expose "Clear clipboard history" in Settings.
// ---------------------------------------------------------------------------

describe("CopyPaste-wuek NG-1: clear-all in Settings Storage tab (canonical location)", () => {
  it("Storage tab has a 'Clear clipboard history' row in the Data section", async () => {
    invoke.mockImplementation(makeOnlineInvoke());
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    // Navigate to the Storage tab — canonical location for destructive data ops.
    const storageTabBtn = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTabBtn); });

    // The "Data" section heading must be present.
    expect(screen.getByText(/^DATA$/i)).toBeInTheDocument();

    // The "Clear clipboard history" row label must be present.
    expect(screen.getByText(/Clear clipboard history/i)).toBeInTheDocument();

    // The "Clear history…" button must be present and enabled when daemon is ready.
    const clearBtn = screen.getByRole("button", { name: /Clear history/i });
    expect(clearBtn).toBeInTheDocument();
    expect(clearBtn).not.toBeDisabled();
  });

  it("clicking 'Clear history…' shows a confirmation modal (w6xc: no longer inline Yes/No)", async () => {
    // w6xc: the inline Yes/No was replaced with a proper ConfirmModal.
    // Clicking "Clear history…" must open a dialog.
    invoke.mockImplementation(makeOnlineInvoke());
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTabBtn = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTabBtn); });

    // Click "Clear history…" → modal opens (role="dialog").
    const clearBtn = screen.getByRole("button", { name: /Clear history/i });
    await act(async () => { fireEvent.click(clearBtn); });

    // A proper dialog must appear.
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    // Modal title confirms the destructive action.
    expect(screen.getByText(/Clear all clipboard history/i)).toBeInTheDocument();
    // Modal has Cancel and Clear history buttons (not Yes/No).
    expect(screen.getByRole("button", { name: /cancel/i })).toBeInTheDocument();
    // The inline "Yes"/"No" must NOT be present — those were the old pattern.
    expect(screen.queryByRole("button", { name: /^Yes$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^No$/i })).not.toBeInTheDocument();
  });

  it("confirming in the modal calls the delete_all IPC", async () => {
    invoke.mockImplementation((cmd: string, args?: unknown): Promise<unknown> => {
      if (cmd === "ipc_call") {
        const method = (args as { method?: string } | undefined)?.method;
        if (method === "delete_all") {
          return Promise.resolve({ ok: true, data: { deleted: 5 }, error: null, error_code: null });
        }
      }
      return makeOnlineInvoke()(cmd, args);
    });

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() => {
      expect(screen.queryByText(/Daemon not running/i)).not.toBeInTheDocument();
    });

    const storageTabBtn = await screen.findByText("Storage");
    await act(async () => { fireEvent.click(storageTabBtn); });

    const clearBtn = screen.getByRole("button", { name: /Clear history/i });
    await act(async () => { fireEvent.click(clearBtn); });

    // Confirm in the modal — the confirm button has the same label as the action.
    const confirmBtn = screen.getByTestId("confirm-modal-confirm-btn");
    await act(async () => { fireEvent.click(confirmBtn); });

    // After confirming, delete_all IPC must have been called.
    await waitFor(() => {
      const deleteAllCalls = invoke.mock.calls.filter(
        ([cmd, args]: [string, unknown]) =>
          cmd === "ipc_call" &&
          (args as { method?: string } | undefined)?.method === "delete_all",
      );
      expect(deleteAllCalls.length).toBeGreaterThanOrEqual(1);
    });
  });
});
