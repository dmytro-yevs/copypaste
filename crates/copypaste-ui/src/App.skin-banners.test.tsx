/**
 * Phase 4: App.tsx banners use fixed radius tokens (--r-card, --r-ctl).
 *
 * Updated from CopyPaste-kp6f: old skin tokens replaced by fixed design tokens.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import React from "react";

// ---------------------------------------------------------------------------
// Module mocks
// ---------------------------------------------------------------------------

vi.mock("./store", () => ({
  useUI: vi.fn((selector: (s: unknown) => unknown) =>
    selector({
      view: "history",
      setView: vi.fn(),
      prefs: {
        translucency: true,
        theme: "dark",
        accent: "indigo",
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

import App from "./App";
import * as ipc from "./lib/ipc";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("App banners — fixed radius tokens (Phase 4)", () => {
  let originalMatchMedia: typeof window.matchMedia;

  beforeEach(() => {
    originalMatchMedia = window.matchMedia;
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
  it("daemon-error banner container uses var(--r-card) not rounded-ide-lg class", async () => {
    vi.mocked(ipc.getDaemonError).mockResolvedValue("socket not found");

    const { findByText } = render(<App />);
    const label = await findByText("Background service error:", {}, { timeout: 2000 });

    const banner = label.closest("div") as HTMLElement | null;
    expect(banner).not.toBeNull();

    // Must NOT use the Tailwind rounded-ide-lg class
    expect(banner!.className).not.toMatch(/rounded-ide/);

    // Must use the fixed design token
    expect(banner!.style.borderRadius).toBe("var(--r-card)");
  });

  // -------------------------------------------------------------------------
  // Protocol-mismatch banner container
  // -------------------------------------------------------------------------
  it("protocol-mismatch banner container uses var(--r-card) not rounded-ide-lg class", async () => {
    vi.mocked(ipc.setProtocolMismatchHandler).mockImplementation(
      (handler: ((v: number) => void) | null) => {
        if (typeof handler === "function") {
          setTimeout(() => handler(2), 0);
        }
      }
    );

    render(<App />);

    const banner = await screen.findByTestId("protocol-mismatch-banner", {}, { timeout: 2000 });

    expect(banner.className).not.toMatch(/rounded-ide/);
    expect(banner.style.borderRadius).toBe("var(--r-card)");
  });

  // -------------------------------------------------------------------------
  // Protocol-mismatch banner dismiss button
  // -------------------------------------------------------------------------
  it("protocol-mismatch Dismiss button uses var(--r-ctl) not rounded-ide class", async () => {
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

    expect(dismissBtn!.className).not.toMatch(/rounded-ide/);
    expect(dismissBtn!.style.borderRadius).toBe("var(--r-ctl)");
  });
});
