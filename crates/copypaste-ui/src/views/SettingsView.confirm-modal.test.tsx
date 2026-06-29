/**
 * CopyPaste-w6xc — "Clear history" in Settings must use a proper modal.
 * CopyPaste-ju6b — "Enable sync" InfoPopover must not contain the stale
 *                  "requires daemon update" warning.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { SettingsView } from "./SettingsView";
import { ErrorBoundary } from "../components/ErrorBoundary";

// ---------------------------------------------------------------------------
// Helpers — minimal daemon stub so Settings renders in "ready" state.
// ---------------------------------------------------------------------------

function setupReadyDaemon() {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "status") {
      return Promise.resolve({
        ok: true,
        data: {
          ready: true,
          degraded: false,
          build_version: "0.7.5",
        },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "get_config") {
      return Promise.resolve({
        ok: true,
        data: {
          p2p_enabled: true,
          supabase_url: null,
          supabase_anon_key: null,
          relay_url: null,
          sync_enabled: true,
          sync_on_wifi_only: false,
          lan_visibility: true,
          auto_apply_synced_clip: true,
          collect_public_ip: false,
          paste_as_plain_text: false,
          excluded_apps: [],
          max_text_size_bytes: null,
          max_image_size_bytes: null,
          max_file_size_bytes: null,
          storage_quota_bytes: null,
          sensitive_ttl_secs: null,
        },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "get_private_mode") {
      return Promise.resolve({
        ok: true,
        data: { private_mode: false },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "get_sync_status") {
      return Promise.resolve({
        ok: true,
        data: { last_sync_ms: null, supabase_url: null },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "get_popup_shortcut") {
      return Promise.resolve({ ok: true, data: "CmdOrCtrl+Shift+V", error: null, error_code: null });
    }
    if (args?.method === "app_version") {
      return Promise.resolve({ ok: true, data: "0.7.5", error: null, error_code: null });
    }
    return Promise.reject("unknown_method");
  });
}

// ---------------------------------------------------------------------------
// CopyPaste-w6xc: "Clear history" must open a proper modal, not inline confirm
// ---------------------------------------------------------------------------

describe("CopyPaste-w6xc: Clear history uses a confirmation modal", () => {
  beforeEach(() => {
    invoke.mockReset();
    setupReadyDaemon();
  });

  it("opens a confirmation modal (not inline Yes/No) when 'Clear history…' is clicked", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Navigate to the Storage tab where Clear history lives.
    // The tab bar uses role="tab" (not button).
    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    // Find the "Clear history" button.
    const clearBtn = await screen.findByRole("button", { name: /clear history/i });
    fireEvent.click(clearBtn);

    // A dialog role must appear (proper modal), not just inline text.
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();

    // The modal must mention the destructive action.
    expect(dialog.textContent).toMatch(/delete|clear|history/i);
  });

  it("does NOT call delete_all when the user cancels the modal", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    const storageTab = await screen.findByRole("tab", { name: /storage/i });
    fireEvent.click(storageTab);

    const clearBtn = await screen.findByRole("button", { name: /clear history/i });
    fireEvent.click(clearBtn);

    await screen.findByRole("dialog");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    fireEvent.click(cancelBtn);

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const deleteAllCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "delete_all",
    );
    expect(deleteAllCalls).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// CopyPaste-ju6b: "Enable sync" InfoPopover must NOT say "requires daemon update"
// ---------------------------------------------------------------------------

describe("CopyPaste-ju6b: Enable sync has no stale 'requires daemon update' warning", () => {
  beforeEach(() => {
    invoke.mockReset();
    setupReadyDaemon();
  });

  it("the 'Enable sync' InfoPopover text does not contain stale daemon-update warning", async () => {
    render(
      <ErrorBoundary label="Settings">
        <SettingsView />
      </ErrorBoundary>,
    );

    // Wait for Settings to render — it shows the General tab by default.
    await screen.findByRole("heading", { name: /settings/i });

    // The stale warning text must not appear anywhere in the document.
    // It was previously in the InfoPopover tooltip for "Enable sync".
    const bodyText = document.body.textContent ?? "";
    expect(bodyText).not.toMatch(/requires a daemon update/i);
    expect(bodyText).not.toMatch(/requires daemon update/i);
  });
});
