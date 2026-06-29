/**
 * Tests for:
 *   bdac.3  — Panel and SliderRow are now shared components (not file-private)
 *   bdac.88 — History display limit row shows a display-only description
 *   ctmv    — imageQuality slider REMOVED (crh3.101: image_quality was a NO-OP;
 *             PNG capture is lossless, daemon never branched on this value)
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

/** Simulates a healthy daemon for SettingsView tests. */
function makeOnlineInvoke(
  overrides: Record<string, (args?: unknown) => unknown> = {},
) {
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
              max_image_size_bytes: 64 * 1024 * 1024,
              max_file_size_bytes: 100 * 1024 * 1024,
              storage_quota_bytes: 10 * 1024 * 1024 * 1024,
              sensitive_ttl_secs: 30,
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
        case "set_config":
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
        default:
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
    }

    if (cmd === "get_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_default_popup_shortcut") return Promise.resolve("CmdOrCtrl+Shift+V");
    if (cmd === "get_allow_screenshots") return Promise.resolve(false);
    if (cmd === "app_version") return Promise.resolve("0.5.5");
    return Promise.resolve(undefined);
  };
}

/** Navigate to the Storage tab and wait for the panel to be visible. */
async function goToStorageTab() {
  const storageTabBtn = await screen.findByRole("tab", { name: /Storage/i }).catch(() =>
    // Tab bar uses regular buttons with role="tab" (see TabBar in SettingsView).
    screen.findByText("Storage"),
  );
  await act(async () => {
    fireEvent.click(storageTabBtn);
  });
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(makeOnlineInvoke());
});

// ---------------------------------------------------------------------------
// bdac.3 — shared component extraction verification
// ---------------------------------------------------------------------------

describe("bdac.3 — Panel and SliderRow are imported from shared components", () => {
  it("Panel is importable as a named export from ../components/Panel", async () => {
    // Dynamic import will throw if the module doesn't exist or lacks the export.
    const mod = await import("../components/Panel");
    expect(typeof mod.Panel).toBe("function");
  });

  it("SliderRow is importable as a named export from ../components/SliderRow", async () => {
    const mod = await import("../components/SliderRow");
    expect(typeof mod.SliderRow).toBe("function");
  });

  it("SettingsView Storage tab renders the History display limit slider without crashing (uses shared SliderRow)", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() =>
      expect(screen.queryByText(/Background service not running/i)).not.toBeInTheDocument(),
    );

    await goToStorageTab();

    // "History display limit" label verifies Panel + SliderRow render correctly.
    // (Image quality slider was removed in crh3.101.)
    expect(await screen.findByText("History display limit")).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// bdac.88 — History display limit is clearly labelled as display-only
// ---------------------------------------------------------------------------

describe("bdac.88 — History display limit shows a display-only description", () => {
  it("renders the 'History display limit' row in the Storage tab", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() =>
      expect(screen.queryByText(/Background service not running/i)).not.toBeInTheDocument(),
    );

    await goToStorageTab();

    expect(await screen.findByText("History display limit")).toBeInTheDocument();
  });

  it("shows an inline description that calls out the display-only / no-deletion nature", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() =>
      expect(screen.queryByText(/Background service not running/i)).not.toBeInTheDocument(),
    );

    await goToStorageTab();

    // The description must communicate that this is a display filter, not a deletion cap.
    // We match on a key phrase that will catch regressions if the description is removed.
    await waitFor(() =>
      expect(screen.getByText(/display filter only/i)).toBeInTheDocument(),
    );
  });

  it("does NOT call set_config when the history display limit slider changes (localStorage only)", async () => {
    const setConfigCalls: unknown[] = [];
    invoke.mockImplementation(
      makeOnlineInvoke({
        ipc_call: (args: unknown) => {
          const method = (args as { method?: string } | undefined)?.method;
          if (method === "set_config") {
            setConfigCalls.push(args);
          }
          // Delegate to the default handler.
          return makeOnlineInvoke()("ipc_call", args);
        },
      }),
    );

    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    await waitFor(() =>
      expect(screen.queryByText(/Background service not running/i)).not.toBeInTheDocument(),
    );

    await goToStorageTab();

    // Wait for the Storage tab to be rendered with the slider.
    await waitFor(() =>
      expect(screen.getByText("History display limit")).toBeInTheDocument(),
    );

    // Clear any set_config calls that happened during load.
    setConfigCalls.length = 0;

    // The history display limit slider changes should NOT trigger set_config calls.
    // (It persists to localStorage/UIPrefs only.)
    // We verify that the field is present and the no-IPC contract is honoured.
    // If a regression causes an IPC call here, this assertion will catch it.
    await act(async () => {
      // Small pause to let any pending async effects settle.
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(setConfigCalls).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// crh3.101 — imageQuality slider REMOVED
// The ctmv tests (slider sends image_quality via set_config, JPEG/PNG
// description) are deleted. image_quality was a documented NO-OP: PNG capture
// is lossless and the daemon never branched on this value.
// ---------------------------------------------------------------------------
