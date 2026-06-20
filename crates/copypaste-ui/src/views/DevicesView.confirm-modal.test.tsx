/**
 * CopyPaste-uw45 — "Revoke all" must use a proper confirmation modal,
 *                  not a tiny inline Yes/No at 12px.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { DevicesView } from "./DevicesView";

// ---------------------------------------------------------------------------
// Helpers — minimal IPC stub that returns a ready state with one peer.
// ---------------------------------------------------------------------------

const stubPeer = {
  fingerprint: "fp-abc123",
  name: "My Mac",
  trusted: true,
  revoked_at: null,
  last_seen_ms: null,
  online: false,
  kind: "desktop",
};

function setupDaemonWithPeer() {
  invoke.mockImplementation((_cmd: string, args: { method?: string }) => {
    if (args?.method === "status") {
      return Promise.resolve({
        ok: true,
        data: { ready: true, degraded: false, build_version: "0.7.5" },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "list_peers") {
      return Promise.resolve({
        ok: true,
        data: { peers: [stubPeer] },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "list_discovered") {
      return Promise.resolve({
        ok: true,
        data: { devices: [] },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "get_device_info") {
      return Promise.resolve({
        ok: true,
        data: { device_id: "test-device-id", device_name: "Test Mac" },
        error: null,
        error_code: null,
      });
    }
    if (args?.method === "revoke_all_peers") {
      return Promise.resolve({
        ok: true,
        data: { revoked: 1 },
        error: null,
        error_code: null,
      });
    }
    return Promise.reject("daemon_offline:/tmp/x.sock");
  });
}

// ---------------------------------------------------------------------------
// CopyPaste-uw45: "Revoke all" must open a proper modal
// ---------------------------------------------------------------------------

describe("CopyPaste-uw45: Revoke all uses a confirmation modal", () => {
  beforeEach(() => {
    invoke.mockReset();
    setupDaemonWithPeer();
  });

  it("opens a modal (role=dialog) when 'Revoke all' is clicked", async () => {
    render(<DevicesView />);

    // Wait for the device list to load.
    const revokeAllBtn = await screen.findByRole("button", { name: /revoke all/i });
    fireEvent.click(revokeAllBtn);

    // A proper dialog must appear.
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();

    // The modal must warn about the action severity.
    expect(dialog.textContent).toMatch(/revoke|trust|device/i);
  });

  it("does NOT call revoke_all_peers when the user cancels", async () => {
    render(<DevicesView />);

    const revokeAllBtn = await screen.findByRole("button", { name: /revoke all/i });
    fireEvent.click(revokeAllBtn);

    await screen.findByRole("dialog");
    const cancelBtn = screen.getByRole("button", { name: /cancel/i });
    fireEvent.click(cancelBtn);

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    const revokeCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
      ([, args]) => args?.method === "revoke_all_peers",
    );
    expect(revokeCalls).toHaveLength(0);
  });

  it("calls revoke_all_peers when the user confirms in the modal", async () => {
    render(<DevicesView />);

    const revokeAllBtn = await screen.findByRole("button", { name: /revoke all/i });
    fireEvent.click(revokeAllBtn);

    await screen.findByRole("dialog");
    const confirmBtn = screen.getByTestId("confirm-modal-confirm-btn");
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      const revokeCalls = (invoke.mock.calls as Array<[string, { method?: string }]>).filter(
        ([, args]) => args?.method === "revoke_all_peers",
      );
      expect(revokeCalls.length).toBeGreaterThan(0);
    });
  });
});
