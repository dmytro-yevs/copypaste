/**
 * Tests for CopyPaste-kp6f (W5, App.tsx part): banner containers and buttons
 * must use inline style borderRadius with skin CSS variables instead of the
 * hard-coded Tailwind rounded-ide-lg / rounded-ide classes.
 *
 * Classic skin defines:
 *   --skin-r-card: 14px  (was rounded-ide-lg → 14px)
 *   --skin-r-ctl:  9px   (was rounded-ide     → 9px)
 *
 * The Tailwind classes are static and cannot be overridden per-skin; the CSS
 * variables are set by the html[data-skin] block in index.css and propagate
 * automatically when the user switches skins.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import React from "react";

// ---------------------------------------------------------------------------
// Module mocks — must NOT reference top-level variables inside factories
// (vi.mock is hoisted; top-level let/const are not yet initialised)
// ---------------------------------------------------------------------------

vi.mock("./store", () => ({
  useUI: vi.fn((selector: (s: unknown) => unknown) =>
    selector({
      view: "history",
      setView: vi.fn(),
      prefs: {
        translucency: true,
        theme: "dark",
        palette: "graphite-mist",
        skin: "classic",
        density: "compact",
        motionReduced: false,
      },
    })
  ),
}));

vi.mock("./components/Sidebar", () => ({
  Sidebar: () => <div data-testid="sidebar" />,
}));

vi.mock("./components/ErrorBoundary", () => ({
  ErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("./components/RestartDaemonButton", () => ({
  RestartDaemonButton: ({ onRestarted }: { onRestarted: () => void }) => (
    <button onClick={onRestarted}>Restart</button>
  ),
}));

vi.mock("./views/HistoryView", () => ({
  HistoryView: () => <div>history</div>,
}));
vi.mock("./views/DevicesView", () => ({
  DevicesView: () => <div>devices</div>,
}));
vi.mock("./views/SettingsView", () => ({
  SettingsView: () => <div>settings</div>,
}));
vi.mock("./views/AboutView", () => ({
  AboutView: () => <div>about</div>,
}));
vi.mock("./views/LogView", () => ({
  LogView: () => <div>logs</div>,
}));

vi.mock("./lib/peerPresence", () => ({
  startPeerPresencePolling: vi.fn(),
  stopPeerPresencePolling: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

// ipc mock: all functions are vi.fn() so individual tests can control return values.
vi.mock("./lib/ipc", () => ({
  appVersion: vi.fn().mockResolvedValue("0.7.5"),
  detectStaleDaemonFromStatus: vi.fn().mockReturnValue(null),
  api: { status: vi.fn().mockResolvedValue({ daemon_version: "0.7.5" }) },
  checkAccessibilityPermission: vi.fn().mockResolvedValue(true),
  requestAccessibilityPermission: vi.fn().mockResolvedValue(undefined),
  getDaemonError: vi.fn().mockResolvedValue(null),
  setProtocolMismatchHandler: vi.fn(),
  CURRENT_PROTOCOL_VERSION: 1,
}));

// Import after mocks are registered
import App from "./App";
// Also import the ipc module so we can access the mocked functions via vi.mocked
import * as ipc from "./lib/ipc";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("§kp6f  App banners — skin token borderRadius (CopyPaste-kp6f)", () => {
  let originalMatchMedia: typeof window.matchMedia;

  beforeEach(() => {
    originalMatchMedia = window.matchMedia;
    // Default: no reduced-motion
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));

    // Reset ipc mocks to safe defaults for each test
    vi.mocked(ipc.getDaemonError).mockResolvedValue(null);
    vi.mocked(ipc.checkAccessibilityPermission).mockResolvedValue(true);
    vi.mocked(ipc.appVersion).mockResolvedValue("0.7.5");
    vi.mocked(ipc.detectStaleDaemonFromStatus).mockReturnValue(null);
    vi.mocked(ipc.setProtocolMismatchHandler).mockReset();
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  // -------------------------------------------------------------------------
  // Daemon-error banner container
  // -------------------------------------------------------------------------
  it("daemon-error banner container uses var(--skin-r-card) not rounded-ide-lg class", async () => {
    vi.mocked(ipc.getDaemonError).mockResolvedValue("socket not found");

    const { findByText } = render(<App />);
    // Wait for async getDaemonError to resolve and banner to appear
    const label = await findByText("Background service error:", {}, { timeout: 2000 });

    // Walk up to the banner container div
    const banner = label.closest("div") as HTMLElement | null;
    expect(banner).not.toBeNull();

    // Must NOT use the Tailwind rounded-ide-lg class
    expect(banner!.className).not.toMatch(/rounded-ide/);

    // Must use inline style with the CSS skin variable
    expect(banner!.style.borderRadius).toBe("var(--skin-r-card)");
  });

  // -------------------------------------------------------------------------
  // Protocol-mismatch banner container
  // -------------------------------------------------------------------------
  it("protocol-mismatch banner container uses var(--skin-r-card) not rounded-ide-lg class", async () => {
    // When setProtocolMismatchHandler is called with a handler, invoke it
    // immediately to simulate a mismatch being detected on the wire.
    vi.mocked(ipc.setProtocolMismatchHandler).mockImplementation(
      (handler: ((v: number) => void) | null) => {
        if (typeof handler === "function") {
          // Call asynchronously so React state update happens after mount
          setTimeout(() => handler(2), 0);
        }
      }
    );

    render(<App />);

    const banner = await screen.findByTestId("protocol-mismatch-banner", {}, { timeout: 2000 });

    // Must NOT use Tailwind class
    expect(banner.className).not.toMatch(/rounded-ide/);

    // Must use inline style with the CSS skin variable
    expect(banner.style.borderRadius).toBe("var(--skin-r-card)");
  });

  // -------------------------------------------------------------------------
  // Protocol-mismatch banner dismiss button
  // -------------------------------------------------------------------------
  it("protocol-mismatch Dismiss button uses var(--skin-r-ctl) not rounded-ide class", async () => {
    vi.mocked(ipc.setProtocolMismatchHandler).mockImplementation(
      (handler: ((v: number) => void) | null) => {
        if (typeof handler === "function") {
          setTimeout(() => handler(2), 0);
        }
      }
    );

    render(<App />);

    const banner = await screen.findByTestId("protocol-mismatch-banner", {}, { timeout: 2000 });
    const dismissBtn = banner.querySelector("button[type='button']") as HTMLElement | null;
    expect(dismissBtn).not.toBeNull();

    // Must NOT use Tailwind class
    expect(dismissBtn!.className).not.toMatch(/rounded-ide/);

    // Must use inline style with the CSS skin variable
    expect(dismissBtn!.style.borderRadius).toBe("var(--skin-r-ctl)");
  });
});
