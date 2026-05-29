import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

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
