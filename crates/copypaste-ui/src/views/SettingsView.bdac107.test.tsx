// SettingsView.bdac107.test.tsx
// CopyPaste-bdac.107: settings-row consistency
//   - "Color theme" row has description
//   - "Version" row has description
//   - "Restart" row renamed to "Restart service" with description
//   - "Private mode" → "Private Mode" (Title Case)
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

function makeOnlineInvoke() {
  return (cmd: string, args?: unknown): Promise<unknown> => {
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
              max_text_size_bytes: 10485760,
              max_image_size_bytes: 26214400,
              max_file_size_bytes: 134217728,
              storage_quota_bytes: 5368709120,
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
            data: {},
            error: null,
            error_code: null,
          });
        default:
          return Promise.resolve({ ok: true, data: null, error: null, error_code: null });
      }
    }
    // Tauri-direct commands (allow_screenshots, etc.)
    return Promise.resolve(null);
  };
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(makeOnlineInvoke());
});

async function renderSettingsOnline() {
  render(
    <ErrorBoundary label="Settings">
      <SettingsView />
    </ErrorBoundary>,
  );
  // Wait until offline banner is gone (daemon online)
  await waitFor(() => {
    expect(screen.queryByText(/Background service not running/i)).not.toBeInTheDocument();
  });
}

describe("bdac.107 — settings-row copy consistency", () => {
  // ── General tab ──────────────────────────────────────────────────────────

  it("'Private Mode' is Title Case (not 'Private mode')", async () => {
    await renderSettingsOnline();
    // The General tab is the default tab — no click needed.
    expect(screen.getByText("Private Mode")).toBeInTheDocument();
    expect(screen.queryByText("Private mode")).not.toBeInTheDocument();
  });

  it("'Version' row has a description", async () => {
    await renderSettingsOnline();
    expect(
      screen.getByText("Current daemon and app version."),
    ).toBeInTheDocument();
  });

  it("'Restart service' row exists (not bare 'Restart')", async () => {
    await renderSettingsOnline();
    expect(screen.getByText("Restart service")).toBeInTheDocument();
    // Bare "Restart" as a SettingsRow title must be gone
    // (RestartDaemonButton's own button label says "Restart background service", not "Restart")
    // Use queryAllByText to avoid false positive from the button label inside the row
    const restartRowTitles = screen.queryAllByText("Restart");
    // None of these should be a SettingsRow title-class element
    const titleSpans = restartRowTitles.filter(
      (el) => el.tagName === "SPAN" && el.classList.contains("text-\\[13px\\]"),
    );
    expect(titleSpans).toHaveLength(0);
  });

  it("'Restart service' row has a description", async () => {
    await renderSettingsOnline();
    expect(
      screen.getByText("Restart the background clipboard service."),
    ).toBeInTheDocument();
  });

  // ── Display tab ──────────────────────────────────────────────────────────

  it("'Theme' row exists in Display tab (renamed from 'Color theme' in Phase 4)", async () => {
    await renderSettingsOnline();
    const displayTab = await screen.findByText("Display");
    await act(async () => { fireEvent.click(displayTab); });

    // Phase 4: row renamed to "Theme"; no description popover (self-explanatory control)
    expect(screen.getByText(/^Theme$/i)).toBeInTheDocument();
    // "Color theme" label is gone
    expect(screen.queryByText(/^Color theme$/i)).toBeNull();
  });
});
